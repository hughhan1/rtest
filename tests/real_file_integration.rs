//! Integration test using a real Python test file to ensure collection works
//! on actual pytest files (not just generated test content).

use rustic::collection_integration::collect_tests_rust;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_collection_on_real_pytest_file() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().join("real_project");
    fs::create_dir_all(&project_path).expect("Failed to create project directory");

    // Create a realistic pytest file with comprehensive test patterns
    let real_pytest_content = r#"def test_simple_assertion():
    assert 1 + 1 == 2

def test_string_operations():
    text = "hello world"
    assert text.upper() == "HELLO WORLD"

def test_list_operations():
    numbers = [1, 2, 3, 4, 5]
    assert len(numbers) == 5

def helper_function():
    return "helper"

class TestMathOperations:
    def test_addition(self):
        assert 10 + 5 == 15
    
    def test_subtraction(self):
        assert 10 - 5 == 5
    
    def setup_method(self):
        pass

class TestStringMethods:
    def test_capitalize(self):
        assert "hello".capitalize() == "Hello"
    
    def test_split(self):
        result = "a,b,c".split(",")
        assert result == ["a", "b", "c"]

class UtilityClass:
    def test_method_should_be_ignored(self):
        pass
    
    def utility_method(self):
        return True

def process_data(data):
    return sum(data)
"#;

    let test_file_path = project_path.join("test_comprehensive.py");
    fs::write(&test_file_path, real_pytest_content).expect("Failed to write test file");

    // Collect tests
    let (test_nodes, _errors) =
        collect_tests_rust(project_path, &[]).expect("Collection should succeed");

    println!("Found {} test nodes:", test_nodes.len());
    for node in &test_nodes {
        println!("  - {node}");
    }

    // Verify we found the expected tests
    assert!(
        !test_nodes.is_empty(),
        "Should find tests in real pytest file"
    );

    // Expected test functions
    let expected_functions = [
        "test_comprehensive.py::test_simple_assertion",
        "test_comprehensive.py::test_string_operations",
        "test_comprehensive.py::test_list_operations",
    ];

    // Expected test class methods
    let expected_class_methods = [
        "test_comprehensive.py::TestMathOperations::test_addition",
        "test_comprehensive.py::TestMathOperations::test_subtraction",
        "test_comprehensive.py::TestStringMethods::test_capitalize",
        "test_comprehensive.py::TestStringMethods::test_split",
    ];

    // Verify all expected tests are found
    for expected in &expected_functions {
        assert!(
            test_nodes.iter().any(|node| node == expected),
            "Should find test function: {expected}. Found: {test_nodes:#?}"
        );
    }

    for expected in &expected_class_methods {
        assert!(
            test_nodes.iter().any(|node| node == expected),
            "Should find test method: {expected}. Found: {test_nodes:#?}"
        );
    }

    // Verify we don't collect non-test items
    let should_not_collect = [
        "helper_function",
        "setup_method",
        "teardown_method",
        "UtilityClass",
        "test_method_should_be_ignored",
        "utility_method",
        "process_data",
    ];

    for item in &should_not_collect {
        assert!(
            !test_nodes.iter().any(|node| node.contains(item)),
            "Should not collect non-test item: {item}"
        );
    }

    // Should find exactly 7 tests (3 functions + 4 class methods)
    assert_eq!(
        test_nodes.len(),
        7,
        "Should find exactly 7 tests, found {}: {:#?}",
        test_nodes.len(),
        test_nodes
    );
}
