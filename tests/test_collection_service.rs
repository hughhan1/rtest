//! Integration tests for TestCollectionService

use rtest_core::{
    collection_service::{TestCollectionService, CollectionConfig},
    collection::CollectionResult,
};
use std::path::PathBuf;
use tempfile::TempDir;
use std::fs;

#[test]
fn test_collection_service_basic() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_sample.py");
    
    fs::write(&test_file, r#"
def test_one():
    assert True

def test_two():
    assert True

class TestClass:
    def test_method(self):
        assert True
"#).unwrap();

    let config = CollectionConfig {
        paths: vec![test_file.clone()],
        ..Default::default()
    };

    let service = TestCollectionService::new(temp_dir.path().to_path_buf(), config);
    let result = service.collect().unwrap();
    
    assert_eq!(result.tests.len(), 3);
    assert_eq!(result.stats.total_files, 1);
    assert_eq!(result.stats.total_tests, 3);
    assert_eq!(result.stats.tests_with_groups, 0);
}

#[test]
fn test_collection_service_with_groups() {
    let temp_dir = TempDir::new().unwrap();
    let test_file = temp_dir.path().join("test_groups.py");
    
    fs::write(&test_file, r#"
import pytest

@pytest.mark.xdist_group("database")
def test_db_one():
    pass

@pytest.mark.xdist_group("database")
def test_db_two():
    pass

@pytest.mark.xdist_group("ui")
def test_ui():
    pass

def test_no_group():
    pass
"#).unwrap();

    let config = CollectionConfig {
        paths: vec![test_file.clone()],
        ..Default::default()
    };

    let service = TestCollectionService::new(temp_dir.path().to_path_buf(), config);
    let result = service.collect().unwrap();
    
    assert_eq!(result.tests.len(), 4);
    assert_eq!(result.stats.total_tests, 4);
    assert_eq!(result.stats.tests_with_groups, 3);
    
    // Verify group assignments
    let db_tests: Vec<_> = result.tests.iter()
        .filter(|t| t.function.xdist_group.as_ref().map(|g| g.as_ref()) == Some("database"))
        .collect();
    assert_eq!(db_tests.len(), 2);
    
    let ui_tests: Vec<_> = result.tests.iter()
        .filter(|t| t.function.xdist_group.as_ref().map(|g| g.as_ref()) == Some("ui"))
        .collect();
    assert_eq!(ui_tests.len(), 1);
}

#[test]
fn test_collection_service_multiple_files() {
    let temp_dir = TempDir::new().unwrap();
    
    // Create multiple test files
    let test_files = vec![
        ("test_a.py", "def test_a1(): pass\ndef test_a2(): pass"),
        ("test_b.py", "def test_b1(): pass\nclass TestB:\n    def test_b2(self): pass"),
        ("test_c.py", "import pytest\n@pytest.mark.xdist_group('special')\ndef test_c(): pass"),
    ];
    
    let mut paths = Vec::new();
    for (name, content) in &test_files {
        let path = temp_dir.path().join(name);
        fs::write(&path, content).unwrap();
        paths.push(path);
    }

    let config = CollectionConfig {
        paths: paths.clone(),
        ..Default::default()
    };

    let service = TestCollectionService::new(temp_dir.path().to_path_buf(), config);
    let result = service.collect().unwrap();
    
    assert_eq!(result.stats.total_files, 3);
    assert_eq!(result.stats.total_tests, 5);
    assert_eq!(result.stats.tests_with_groups, 1);
    
    // Verify all tests are collected
    let test_names: Vec<_> = result.tests.iter()
        .map(|t| t.function.name.as_ref())
        .collect();
    
    assert!(test_names.contains(&"test_a1"));
    assert!(test_names.contains(&"test_a2"));
    assert!(test_names.contains(&"test_b1"));
    assert!(test_names.contains(&"test_b2"));
    assert!(test_names.contains(&"test_c"));
}

#[test]
fn test_collection_service_error_handling() {
    let temp_dir = TempDir::new().unwrap();
    let invalid_file = temp_dir.path().join("test_invalid.py");
    
    // Write invalid Python syntax
    fs::write(&invalid_file, "def test_invalid(\n    pass").unwrap();

    let config = CollectionConfig {
        paths: vec![invalid_file],
        ..Default::default()
    };

    let service = TestCollectionService::new(temp_dir.path().to_path_buf(), config);
    let result = service.collect();
    
    // Should handle parse errors gracefully
    assert!(result.is_err());
}

#[test]
fn test_collection_service_empty_directory() {
    let temp_dir = TempDir::new().unwrap();
    
    let config = CollectionConfig {
        paths: vec![temp_dir.path().to_path_buf()],
        ..Default::default()
    };

    let service = TestCollectionService::new(temp_dir.path().to_path_buf(), config);
    let result = service.collect().unwrap();
    
    assert_eq!(result.tests.len(), 0);
    assert_eq!(result.stats.total_files, 0);
    assert_eq!(result.stats.total_tests, 0);
}