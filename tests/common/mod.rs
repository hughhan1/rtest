//! Common test utilities and helpers.

use indoc::indoc;
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
    let test_file_content = indoc! {r#"
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
    "#};

    let test_file_path = project_path.join("test_sample.py");
    let mut file = fs::File::create(&test_file_path).expect("Failed to create test file");
    file.write_all(test_file_content.as_bytes())
        .expect("Failed to write test file");

    // Create another test file
    let another_test_content = indoc! {r#"
        def test_math_operations():
            assert 5 + 3 == 8

        class TestCalculator:
            def test_addition(self):
                assert 10 + 5 == 15
            
            def test_subtraction(self):
                assert 10 - 5 == 5
    "#};

    let another_test_path = project_path.join("test_math.py");
    let mut file = fs::File::create(&another_test_path).expect("Failed to create second test file");
    file.write_all(another_test_content.as_bytes())
        .expect("Failed to write second test file");

    // Create a non-test file
    let regular_file_content = indoc! {r#"
        def helper_function():
            return "helper"

        def test_in_regular_file():
            # This should be ignored since file doesn't start with test_
            pass
    "#};

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

/// Runs pytest --collect-only and returns the output
#[allow(dead_code)]
pub fn run_pytest_collection(project_path: &PathBuf, files: &[&str]) -> Result<String, String> {
    // Get the rtest project root (where pyproject.toml is)
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Get the Python executable from the venv - platform-aware
    let python_exe = if cfg!(target_os = "windows") {
        manifest_dir.join(".venv/Scripts/python.exe")
    } else {
        manifest_dir.join(".venv/bin/python")
    };

    if !python_exe.exists() {
        return Err(
            "Python venv not found. Run 'uv sync --dev' to set up the environment.".to_string(),
        );
    }

    // Create both pytest.ini AND setup.py to mark this as the root
    let pytest_ini_path = project_path.join("pytest.ini");
    let pytest_ini_content = "[pytest]\n\
        testpaths = .\n\
        python_files = test_*.py *_test.py\n\
        python_classes = Test*\n\
        python_functions = test_*\n";
    fs::write(&pytest_ini_path, pytest_ini_content)
        .map_err(|e| format!("Failed to create pytest.ini: {}", e))?;

    // Create a setup.py to stop pytest from walking up further
    let setup_py_path = project_path.join("setup.py");
    fs::write(&setup_py_path, "# Marker file\n")
        .map_err(|e| format!("Failed to create setup.py: {}", e))?;

    // Use the venv python directly
    let mut cmd = Command::new(&python_exe);
    cmd.arg("-m")
        .arg("pytest")
        .arg("--collect-only")
        .arg("-q") // Quiet mode for cleaner output
        .arg("-p")
        .arg("no:cacheprovider") // Disable pytest cache
        .current_dir(project_path); // Run pytest in the temp test directory

    // Always specify what to collect - either files or "."
    if files.is_empty() {
        cmd.arg(".");
    } else {
        for file in files {
            cmd.arg(file);
        }
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute pytest: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !output.status.success() && stdout.is_empty() && stderr.contains("No module named pytest") {
        return Err(
            "pytest not found. Run 'uv sync --dev' to install development dependencies."
                .to_string(),
        );
    }

    Ok(format!("{}{}", stdout, stderr))
}

/// Runs rtest --collect-only and returns the output
#[allow(dead_code)]
pub fn run_rtest_collection(project_path: &PathBuf, files: &[&str]) -> Result<String, String> {
    let rtest_binary = get_rtest_binary();

    let mut cmd = Command::new(&rtest_binary);
    cmd.arg("--collect-only").current_dir(project_path);

    // Add specific files if provided, otherwise collect all
    for file in files {
        cmd.arg(file);
    }

    let output = cmd
        .output()
        .map_err(|e| format!("Failed to execute rtest: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    Ok(format!("{}{}", stdout, stderr))
}

/// Extracts test node IDs from collection output
/// Returns a sorted vector of test identifiers
#[allow(dead_code)]
pub fn parse_collected_tests(output: &str) -> Vec<String> {
    let mut tests = Vec::new();

    for line in output.lines() {
        let trimmed = line.trim();

        // Look for lines containing test identifiers (with ::)
        if trimmed.contains("::") && trimmed.contains("test_") {
            // Extract the test identifier
            // Handle both pytest format: "  <Module test_file.py>"
            // and rtest format: "test_file.py::test_function"

            // Try to find the test identifier pattern
            if let Some(test_id) = extract_test_id(trimmed) {
                tests.push(test_id);
            }
        }
    }

    // Sort for consistent comparison
    tests.sort();
    tests.dedup(); // Remove duplicates
    tests
}

/// Helper to extract test ID from a line
fn extract_test_id(line: &str) -> Option<String> {
    // Handle format like: "test_file.py::test_function"
    // or "test_file.py::TestClass::test_method"

    // Find the part that looks like a test identifier
    let parts: Vec<&str> = line.split_whitespace().collect();

    for part in parts {
        if part.contains("::") && part.contains(".py") {
            // Normalize path separators to forward slashes
            let normalized = part.replace('\\', "/");
            return Some(normalized);
        }
    }

    None
}

/// Compares two sets of collected tests and returns the differences
#[allow(dead_code)]
pub fn compare_test_collections(
    rtest_tests: &[String],
    pytest_tests: &[String],
) -> (Vec<String>, Vec<String>) {
    use std::collections::HashSet;

    let rtest_set: HashSet<_> = rtest_tests.iter().collect();
    let pytest_set: HashSet<_> = pytest_tests.iter().collect();

    // Tests in rtest but not in pytest
    let only_rtest: Vec<String> = rtest_set
        .difference(&pytest_set)
        .map(|s| (*s).clone())
        .collect();

    // Tests in pytest but not in rtest
    let only_pytest: Vec<String> = pytest_set
        .difference(&rtest_set)
        .map(|s| (*s).clone())
        .collect();

    (only_rtest, only_pytest)
}

/// Formats the diff between rtest and pytest collection results
#[allow(dead_code)]
pub fn format_diff(rtest_tests: &[String], pytest_tests: &[String]) -> String {
    let (only_rtest, only_pytest) = compare_test_collections(rtest_tests, pytest_tests);

    let mut diff = String::new();

    diff.push_str(&format!(
        "\n=== Collection Comparison ===\n\
         rtest collected: {} tests\n\
         pytest collected: {} tests\n\n",
        rtest_tests.len(),
        pytest_tests.len()
    ));

    if !only_rtest.is_empty() {
        diff.push_str(&format!(
            "Tests found by rtest but NOT by pytest ({}):\n",
            only_rtest.len()
        ));
        for test in &only_rtest {
            diff.push_str(&format!("  + {}\n", test));
        }
        diff.push('\n');
    }

    if !only_pytest.is_empty() {
        diff.push_str(&format!(
            "Tests found by pytest but NOT by rtest ({}):\n",
            only_pytest.len()
        ));
        for test in &only_pytest {
            diff.push_str(&format!("  - {}\n", test));
        }
        diff.push('\n');
    }

    if only_rtest.is_empty() && only_pytest.is_empty() {
        diff.push_str("✓ Collections match perfectly!\n");
    }

    diff
}

/// Asserts that rtest and pytest collections match, with detailed diff on failure
#[allow(dead_code)]
pub fn assert_collections_match(rtest_tests: &[String], pytest_tests: &[String], context: &str) {
    let (only_rtest, only_pytest) = compare_test_collections(rtest_tests, pytest_tests);

    if !only_rtest.is_empty() || !only_pytest.is_empty() {
        let diff = format_diff(rtest_tests, pytest_tests);
        panic!("Collection mismatch for {}: {}", context, diff);
    }
}
