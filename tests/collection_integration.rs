//! Integration tests for test collection functionality.

use rustic::collection_integration::{collect_tests_rust, display_collection_results};

mod common;

/// Test that Rust-based collection finds all expected tests
#[test]
fn test_rust_collection_finds_all_tests() {
    let (_temp_dir, project_path) = common::create_test_project();

    // Collect tests from the temporary project
    let test_nodes = collect_tests_rust(project_path, &[]);

    // Should find tests from both test_sample.py and test_math.py
    assert!(!test_nodes.is_empty(), "Should find some tests");

    // Check for specific test nodes we expect
    let expected_patterns = [
        "test_sample.py::test_simple_function",
        "test_sample.py::test_another_function",
        "test_sample.py::TestExampleClass::test_method_one",
        "test_sample.py::TestExampleClass::test_method_two",
        "test_math.py::test_math_operations",
        "test_math.py::TestCalculator::test_addition",
        "test_math.py::TestCalculator::test_subtraction",
    ];

    for pattern in &expected_patterns {
        assert!(
            test_nodes.iter().any(|node| node.contains(pattern)),
            "Should find test matching pattern: {pattern}. Found: {test_nodes:#?}"
        );
    }

    // Should NOT find the test in utils.py (non-test file)
    assert!(
        !test_nodes.iter().any(|node| node.contains("utils.py")),
        "Should not find tests in non-test files"
    );

    // Should NOT find helper methods or non-test functions
    assert!(
        !test_nodes.iter().any(|node| node.contains("helper_method")),
        "Should not find helper methods"
    );
    assert!(
        !test_nodes.iter().any(|node| node.contains("not_a_test")),
        "Should not find non-test functions"
    );
}

/// Test collection with no test files
#[test]
fn test_collection_with_no_tests() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().to_path_buf();

    // Create a non-test Python file
    let content = r#"
def regular_function():
    return "hello"

class RegularClass:
    def method(self):
        pass
"#;

    let file_path = project_path.join("regular.py");
    fs::write(&file_path, content).expect("Failed to write file");

    let test_nodes = collect_tests_rust(project_path, &[]);

    assert!(
        test_nodes.is_empty(),
        "Should find no tests in regular Python files"
    );
}

/// Test that display_collection_results doesn't panic
#[test]
fn test_display_collection_results() {
    let test_nodes = vec![
        "test_file.py::test_function".to_string(),
        "test_file.py::TestClass::test_method".to_string(),
    ];

    // This should not panic
    display_collection_results(&test_nodes);

    // Test with empty list
    display_collection_results(&[]);
}

/// Test collection with malformed Python files
#[test]
fn test_collection_with_syntax_errors() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().to_path_buf();

    // Create a Python file with syntax errors
    let malformed_content = r#"
def test_function(
    # Missing closing parenthesis and body
"#;

    let file_path = project_path.join("test_malformed.py");
    fs::write(&file_path, malformed_content).expect("Failed to write file");

    // Collection should handle parse errors gracefully
    let test_nodes = collect_tests_rust(project_path, &[]);

    // Should not crash, but also shouldn't find any tests
    assert!(
        test_nodes.is_empty(),
        "Should gracefully handle malformed Python files"
    );
}
