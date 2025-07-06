//! Integration tests for error handling improvements

use rtest_core::{
    error::{RtestError, CollectionError, SchedulerError, WorkerError},
    scheduler::{Scheduler, LoadScheduler},
    downcast::CollectorDowncast,
    collection::{Collector, Function, Location},
};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn test_scheduler_error_types() {
    let scheduler = LoadScheduler;
    
    // Test invalid worker count
    let result = scheduler.distribute(vec!["test".to_string()], 0);
    match result {
        Err(SchedulerError::InvalidWorkerCount { requested, min, max }) => {
            assert_eq!(requested, 0);
            assert_eq!(min, 1);
            assert_eq!(max, usize::MAX);
        }
        _ => panic!("Expected InvalidWorkerCount error"),
    }
    
    // Test empty input - should not error
    let result = scheduler.distribute(Vec::<String>::new(), 4);
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_error_conversion() {
    // Test that different error types convert to RtestError
    let collection_err = CollectionError::ParseError("test parse error".to_string());
    let rtest_err: RtestError = collection_err.into();
    
    match rtest_err {
        RtestError::Collection(CollectionError::ParseError(msg)) => {
            assert_eq!(msg, "test parse error");
        }
        _ => panic!("Wrong error type"),
    }
    
    let scheduler_err = SchedulerError::EmptyInput;
    let rtest_err: RtestError = scheduler_err.into();
    
    match rtest_err {
        RtestError::Scheduler(SchedulerError::EmptyInput) => {
            // Success
        }
        _ => panic!("Wrong error type"),
    }
}

#[test]
fn test_safe_downcasting() {
    struct TestCollector;
    
    impl Collector for TestCollector {
        fn name(&self) -> &str { "test" }
        fn location(&self) -> &Location { unimplemented!() }
        fn as_any(&self) -> &dyn std::any::Any { self }
    }
    
    let collector: Box<dyn Collector> = Box::new(TestCollector);
    
    // Should fail to downcast to Function
    let result = collector.as_function();
    assert!(result.is_err());
    
    match result {
        Err(RtestError::Collection(CollectionError::InvalidDowncast { from, to })) => {
            assert_eq!(from, "test");
            assert_eq!(to, "Function");
        }
        _ => panic!("Expected InvalidDowncast error"),
    }
    
    // Try_as_function should return None
    assert!(collector.try_as_function().is_none());
}

#[test]
fn test_function_downcast_success() {
    let function = Function {
        name: Arc::from("test_func"),
        nodeid: Arc::from("test.py::test_func"),
        location: Location {
            path: PathBuf::from("test.py"),
            line: Some(10),
            name: "test_func".to_string(),
        },
        xdist_group: None,
    };
    
    let collector: Box<dyn Collector> = Box::new(function.clone());
    
    // Should successfully downcast to Function
    let result = collector.as_function();
    assert!(result.is_ok());
    
    let downcasted = result.unwrap();
    assert_eq!(downcasted.name, function.name);
    assert_eq!(downcasted.nodeid, function.nodeid);
    
    // Try_as_function should return Some
    assert!(collector.try_as_function().is_some());
}

#[test]
fn test_error_display() {
    // Test error messages are properly formatted
    let errors = vec![
        RtestError::Collection(CollectionError::FileNotFound("test.py".into())),
        RtestError::Scheduler(SchedulerError::InvalidWorkerCount { 
            requested: 0, 
            min: 1, 
            max: 10 
        }),
        RtestError::Worker(WorkerError::SpawnFailed { 
            worker_id: 5, 
            reason: "resource limit".into() 
        }),
    ];
    
    for error in errors {
        let msg = error.to_string();
        assert!(!msg.is_empty(), "Error message should not be empty");
        
        // Verify error messages contain relevant information
        match &error {
            RtestError::Collection(CollectionError::FileNotFound(path)) => {
                assert!(msg.contains(path));
            }
            RtestError::Scheduler(SchedulerError::InvalidWorkerCount { requested, .. }) => {
                assert!(msg.contains(&requested.to_string()));
            }
            RtestError::Worker(WorkerError::SpawnFailed { worker_id, .. }) => {
                assert!(msg.contains(&worker_id.to_string()));
            }
            _ => {}
        }
    }
}

#[test]
fn test_collection_error_context() {
    use std::fs;
    use tempfile::TempDir;
    
    let temp_dir = TempDir::new().unwrap();
    let bad_file = temp_dir.path().join("bad_syntax.py");
    
    // Write invalid Python
    fs::write(&bad_file, "def test_bad(\n    pass").unwrap();
    
    // Attempt to parse - should get meaningful error
    let content = fs::read_to_string(&bad_file).unwrap();
    let result = rtest_core::python_discovery::discover_tests(
        &bad_file,
        &content,
        &Default::default()
    );
    
    assert!(result.is_err());
    match result {
        Err(CollectionError::ParseError(msg)) => {
            assert!(msg.contains("bad_syntax.py"));
            assert!(msg.contains("parse"));
        }
        _ => panic!("Expected ParseError"),
    }
}