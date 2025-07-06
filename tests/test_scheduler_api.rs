//! Integration tests for the new generic Scheduler API

use rtest_core::{
    scheduler::{create_scheduler_function, create_scheduler_string, DistributionMode, LoadGroupScheduler, Scheduler},
    collection::{Function, Location},
};
use std::path::PathBuf;
use std::sync::Arc;

#[test]
fn test_string_scheduler_basic() {
    let tests = vec![
        "test_module.py::test_one".to_string(),
        "test_module.py::test_two".to_string(),
        "test_module.py::test_three".to_string(),
        "test_module.py::test_four".to_string(),
    ];

    let scheduler = create_scheduler_string(DistributionMode::Load);
    let result = scheduler.distribute(tests.clone(), 2).unwrap();
    
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].len() + result[1].len(), 4);
    
    // Verify all tests are distributed
    let mut all_tests: Vec<String> = result.into_iter().flatten().collect();
    all_tests.sort();
    let mut expected = tests;
    expected.sort();
    assert_eq!(all_tests, expected);
}

#[test]
fn test_function_scheduler_with_groups() {
    let functions = vec![
        Function {
            name: Arc::from("test_db_1"),
            nodeid: Arc::from("tests/test_db.py::test_db_1"),
            location: Location { 
                path: PathBuf::from("tests/test_db.py"), 
                line: Some(10), 
                name: "test_db_1".to_string() 
            },
            xdist_group: Some(Arc::from("database")),
        },
        Function {
            name: Arc::from("test_db_2"),
            nodeid: Arc::from("tests/test_db.py::test_db_2"),
            location: Location { 
                path: PathBuf::from("tests/test_db.py"), 
                line: Some(20), 
                name: "test_db_2".to_string() 
            },
            xdist_group: Some(Arc::from("database")),
        },
        Function {
            name: Arc::from("test_ui_1"),
            nodeid: Arc::from("tests/test_ui.py::test_ui_1"),
            location: Location { 
                path: PathBuf::from("tests/test_ui.py"), 
                line: Some(5), 
                name: "test_ui_1".to_string() 
            },
            xdist_group: Some(Arc::from("ui")),
        },
        Function {
            name: Arc::from("test_api"),
            nodeid: Arc::from("tests/test_api.py::test_api"),
            location: Location { 
                path: PathBuf::from("tests/test_api.py"), 
                line: Some(15), 
                name: "test_api".to_string() 
            },
            xdist_group: None,
        },
    ];

    let scheduler = LoadGroupScheduler;
    let result = scheduler.distribute(functions, 3).unwrap();
    
    // Should have 3 groups: database, ui, and ungrouped
    assert_eq!(result.len(), 3);
    
    // Verify that tests with same group are together
    for worker_functions in &result {
        if worker_functions.len() > 1 {
            // If multiple functions, they should have the same group
            let first_group = &worker_functions[0].xdist_group;
            for func in worker_functions.iter().skip(1) {
                assert_eq!(&func.xdist_group, first_group);
            }
        }
    }
}

#[test]
fn test_scheduler_error_handling() {
    let scheduler = create_scheduler_string(DistributionMode::Load);
    
    // Test with 0 workers
    let result = scheduler.distribute(vec!["test".to_string()], 0);
    assert!(result.is_err());
    
    // Test with empty input - should succeed with empty result
    let result = scheduler.distribute(vec![], 5).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_all_distribution_modes() {
    let tests = vec![
        "test_a.py::test_1".to_string(),
        "test_a.py::test_2".to_string(),
        "test_b.py::test_3".to_string(),
        "test_b.py::test_4".to_string(),
    ];

    for mode in &[
        DistributionMode::Load,
        DistributionMode::LoadScope,
        DistributionMode::LoadFile,
        DistributionMode::LoadGroup,
        DistributionMode::WorkSteal,
        DistributionMode::No,
    ] {
        let scheduler = create_scheduler_string(*mode);
        let result = scheduler.distribute(tests.clone(), 2);
        
        assert!(result.is_ok(), "Mode {:?} failed", mode);
        let distributed = result.unwrap();
        
        if *mode == DistributionMode::No {
            assert_eq!(distributed.len(), 1, "No mode should return single group");
        } else {
            assert!(!distributed.is_empty(), "Mode {:?} returned empty", mode);
        }
        
        // Verify all tests are distributed
        let total: usize = distributed.iter().map(|w| w.len()).sum();
        assert_eq!(total, tests.len(), "Mode {:?} lost tests", mode);
    }
}

#[test]
fn test_string_interning_in_functions() {
    use rtest_core::string_interner::intern;
    
    // Test that interned strings work correctly with Functions
    let group_name = intern("shared_group");
    
    let func1 = Function {
        name: intern("test_1"),
        nodeid: intern("test.py::test_1"),
        location: Location { 
            path: PathBuf::from("test.py"), 
            line: Some(1), 
            name: "test_1".to_string() 
        },
        xdist_group: Some(Arc::clone(&group_name)),
    };
    
    let func2 = Function {
        name: intern("test_2"),
        nodeid: intern("test.py::test_2"),
        location: Location { 
            path: PathBuf::from("test.py"), 
            line: Some(10), 
            name: "test_2".to_string() 
        },
        xdist_group: Some(Arc::clone(&group_name)),
    };
    
    // Verify that the Arc pointers are the same
    assert!(Arc::ptr_eq(&func1.xdist_group.as_ref().unwrap(), &func2.xdist_group.as_ref().unwrap()));
}