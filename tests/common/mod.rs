//! Common test utilities and helpers.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;

/// Creates a temporary directory with Python test files for testing
pub fn create_test_project() -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    // Create a subdirectory that doesn't start with a dot to avoid being ignored
    let project_path = temp_dir.path().join("test_project");
    std::fs::create_dir_all(&project_path).expect("Failed to create project directory");

    // Create a simple test file
    let test_file_content = r#"
def test_simple_function():
    assert 1 + 1 == 2

def test_another_function():
    assert "hello".upper() == "HELLO"

def not_a_test():
    pass

class TestExampleClass:
    def test_method_one(self):
        assert True
    
    def test_method_two(self):
        assert 2 * 2 == 4
    
    def helper_method(self):
        pass

class NotATestClass:
    def test_ignored(self):
        pass
"#;

    let test_file_path = project_path.join("test_sample.py");
    let mut file = fs::File::create(&test_file_path).expect("Failed to create test file");
    file.write_all(test_file_content.as_bytes())
        .expect("Failed to write test file");

    // Create another test file
    let another_test_content = r#"
def test_math_operations():
    assert 5 + 3 == 8

class TestCalculator:
    def test_addition(self):
        assert 10 + 5 == 15
    
    def test_subtraction(self):
        assert 10 - 5 == 5
"#;

    let another_test_path = project_path.join("test_math.py");
    let mut file = fs::File::create(&another_test_path).expect("Failed to create second test file");
    file.write_all(another_test_content.as_bytes())
        .expect("Failed to write second test file");

    // Create a non-test file
    let regular_file_content = r#"
def helper_function():
    return "helper"

def test_in_regular_file():
    # This should be ignored since file doesn't start with test_
    pass
"#;

    let regular_file_path = project_path.join("utils.py");
    let mut file = fs::File::create(&regular_file_path).expect("Failed to create regular file");
    file.write_all(regular_file_content.as_bytes())
        .expect("Failed to write regular file");

    (temp_dir, project_path)
}
