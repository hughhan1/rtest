//! Configuration parsing for pytest settings from pyproject.toml

use log::debug;
use std::path::{Path, PathBuf};
use toml::Value;

/// Pytest configuration from pyproject.toml
#[derive(Debug, Clone, Default)]
pub struct PytestConfig {
    /// Test paths to search for tests
    pub testpaths: Vec<PathBuf>,
    /// Glob patterns for test file names (e.g., "test_*.py", "*_test.py")
    pub python_files: Vec<String>,
    /// Patterns for test class names (e.g., "Test*")
    pub python_classes: Vec<String>,
    /// Patterns for test function/method names (e.g., "test*")
    pub python_functions: Vec<String>,
}

/// Read pytest configuration from pyproject.toml
pub fn read_pytest_config(root_path: &Path) -> PytestConfig {
    let pyproject_path = root_path.join("pyproject.toml");

    if !pyproject_path.exists() {
        debug!("No pyproject.toml found at {pyproject_path:?}");
        return PytestConfig::default();
    }

    let content = match std::fs::read_to_string(&pyproject_path) {
        Ok(content) => content,
        Err(e) => {
            debug!("Failed to read pyproject.toml: {e}");
            return PytestConfig::default();
        }
    };

    let toml_value: Value = match toml::from_str(&content) {
        Ok(value) => value,
        Err(e) => {
            debug!("Failed to parse pyproject.toml: {e}");
            return PytestConfig::default();
        }
    };

    let mut config = PytestConfig::default();

    let ini_options = toml_value
        .get("tool")
        .and_then(|t| t.get("pytest"))
        .and_then(|p| p.get("ini_options"));

    if let Some(values) = read_string_array(ini_options, "testpaths") {
        config.testpaths = values.map(PathBuf::from).collect();
        debug!("Found testpaths in pyproject.toml: {:?}", config.testpaths);
    }

    for (name, patterns) in [
        ("python_files", &mut config.python_files),
        ("python_classes", &mut config.python_classes),
        ("python_functions", &mut config.python_functions),
    ] {
        if let Some(values) = read_string_array(ini_options, name) {
            *patterns = values.map(String::from).collect();
            debug!("Found {name} in pyproject.toml: {patterns:?}");
        }
    }

    config
}

/// Read a TOML array of strings under the given `key`, yielding an iterator of
/// its string elements. Returns `None` when the key is absent or not an array.
fn read_string_array<'a>(
    ini_options: Option<&'a Value>,
    key: &str,
) -> Option<impl Iterator<Item = &'a str>> {
    ini_options
        .and_then(|i| i.get(key))
        .and_then(|t| t.as_array())
        .map(|array| array.iter().filter_map(|v| v.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_read_pytest_config_no_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = read_pytest_config(temp_dir.path());
        assert!(config.testpaths.is_empty());
        assert!(config.python_files.is_empty());
        assert!(config.python_classes.is_empty());
        assert!(config.python_functions.is_empty());
    }

    #[test]
    fn test_read_pytest_config_no_testpaths() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

        let content = indoc! {r#"
            [tool.pytest.ini_options]
            filterwarnings = ["error"]
        "#};

        fs::write(&pyproject_path, content).unwrap();

        let config = read_pytest_config(temp_dir.path());
        assert!(config.testpaths.is_empty());
        assert!(config.python_files.is_empty());
        assert!(config.python_classes.is_empty());
        assert!(config.python_functions.is_empty());
    }

    #[test]
    fn reads_all_configured_paths_and_patterns() {
        let temp_dir = TempDir::new().expect("temporary project");
        fs::write(
            temp_dir.path().join("pyproject.toml"),
            indoc! {r#"
            [tool.pytest.ini_options]
            testpaths = ["tests", "test"]
            python_files = ["test_*.py", "*_test.py", "check_*.py"]
            python_classes = ["Test*", "Check*", "*Suite"]
            python_functions = ["test*", "check_*", "*_test"]
        "#},
        )
        .expect("write configuration");

        let config = read_pytest_config(temp_dir.path());
        assert_eq!(
            config.testpaths,
            [PathBuf::from("tests"), PathBuf::from("test")]
        );
        assert_eq!(
            config.python_files,
            ["test_*.py", "*_test.py", "check_*.py"]
        );
        assert_eq!(config.python_classes, ["Test*", "Check*", "*Suite"]);
        assert_eq!(config.python_functions, ["test*", "check_*", "*_test"]);
    }
}
