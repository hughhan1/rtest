//! Integration tests for test collection functionality.

use rustic::collection::CollectionError;
use rustic::collection_integration::{collect_tests_rust, display_collection_results};

mod common;

/// Test that Rust-based collection finds all expected tests
#[test]
fn test_rust_collection_finds_all_tests() {
    let (_temp_dir, project_path) = common::create_test_project();

    // Collect tests from the temporary project
    let test_nodes = collect_tests_rust(project_path, &[]).expect("Collection should succeed");

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

    let test_nodes = collect_tests_rust(project_path, &[]).expect("Collection should succeed");

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
fn test_collection_with_syntax_errors_returns_error() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().to_path_buf();

    let malformed_content = r#"def test_function():
    if True  # Missing colon
        pass"#;

    let file_path = project_path.join("test_malformed.py");
    fs::write(&file_path, malformed_content).expect("Failed to write file");

    let result = collect_tests_rust(project_path, &[file_path.to_str().unwrap().to_string()]);
    assert!(
        result.is_err(),
        "Should return error for malformed Python files"
    );

    let error = result.unwrap_err();
    assert!(
        matches!(error, CollectionError::ParseError(_)),
        "Should be a parse error"
    );
    assert!(
        error.to_string().contains("Parse error:"),
        "Error message should mention parse error"
    );
}

/// Test collection with missing colon syntax error
#[test]
fn test_collection_missing_colon_error() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().to_path_buf();

    let content = r#"
def test_broken():
    if True
        assert False  # Missing colon after if
"#;

    let file_path = project_path.join("test_syntax_error.py");
    fs::write(&file_path, content).expect("Failed to write file");

    let result = collect_tests_rust(project_path, &[file_path.to_str().unwrap().to_string()]);
    assert!(result.is_err(), "Should return error for syntax error");

    let error = result.unwrap_err();
    assert!(
        matches!(error, CollectionError::ParseError(_)),
        "Should be a parse error"
    );
    assert!(
        error.to_string().contains("ExpectedToken"),
        "Error should mention expected token"
    );
}

/// Test collection with while statement missing condition
#[test]
fn test_collection_while_stmt_missing_condition() {
    use std::fs;
    use tempfile::TempDir;

    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().to_path_buf();

    let content = r#"while : ..."#;

    let file_path = project_path.join("test_while_error.py");
    fs::write(&file_path, content).expect("Failed to write file");

    let result = collect_tests_rust(project_path, &[file_path.to_str().unwrap().to_string()]);
    assert!(
        result.is_err(),
        "Should return error for while statement syntax error"
    );

    let error = result.unwrap_err();
    assert!(
        matches!(error, CollectionError::ParseError(_)),
        "Should be a parse error"
    );
    assert!(
        error.to_string().contains("Parse error:"),
        "Error message should mention parse error"
    );
}
