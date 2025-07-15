//! Common test utilities and helpers.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tempfile::TempDir;

/// Creates a temporary directory with Python test files for testing.
///
/// Used by multiple integration test modules (cli_integration, cli_parallel_options, etc.)
#[allow(dead_code)]
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

/// Helper function to get the path to the rtest binary.
///
/// Used by multiple integration test modules.
#[allow(dead_code)]
pub fn get_rtest_binary() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("rtest");
    path.push("target");
    path.push("debug");
    path.push("rtest");

    // Add .exe extension on Windows
    if cfg!(target_os = "windows") {
        path.set_extension("exe");
    }

    // Ensure the binary is built
    if !path.exists() {
        let output = Command::new("cargo")
            .args(["build", "--bin", "rtest"])
            .current_dir(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("rtest"))
            .output()
            .expect("Failed to build rtest binary");

        if !output.status.success() {
            panic!(
                "Failed to build rtest binary: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    path
}

/// Creates a temporary directory with specified Python test files for testing.
///
/// This is a more flexible version that accepts a HashMap of file paths and contents,
/// similar to the Python test helper.
///
/// # Arguments
///
/// * `files` - A HashMap where keys are file paths (relative to project root) and values are file contents
///
/// # Returns
///
/// A tuple of (TempDir, PathBuf) where TempDir is the temporary directory handle
/// and PathBuf is the path to the project directory inside it.
#[allow(dead_code)]
pub fn create_test_project_with_files(files: HashMap<&str, &str>) -> (TempDir, PathBuf) {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().join("test_project");
    std::fs::create_dir_all(&project_path).expect("Failed to create project directory");

    for (file_path, content) in files {
        let full_path = project_path.join(file_path);

        // Create parent directories if they don't exist
        if let Some(parent) = full_path.parent() {
            std::fs::create_dir_all(parent).expect("Failed to create parent directories");
        }

        let mut file = fs::File::create(&full_path)
            .unwrap_or_else(|e| panic!("Failed to create file {file_path}: {e}"));
        file.write_all(content.as_bytes())
            .unwrap_or_else(|e| panic!("Failed to write file {file_path}: {e}"));
    }

    (temp_dir, project_path)
}

/// Creates a test project with a single Python file
///
/// Convenience function for simple test cases that only need one file.
#[allow(dead_code)]
pub fn create_test_file(filename: &str, content: &str) -> (TempDir, PathBuf) {
    let mut files = HashMap::new();
    files.insert(filename, content);
    create_test_project_with_files(files)
}
