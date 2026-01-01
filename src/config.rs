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

    if let Some(testpaths) = ini_options
        .and_then(|i| i.get("testpaths"))
        .and_then(|t| t.as_array())
    {
        config.testpaths = testpaths
            .iter()
            .filter_map(|v| v.as_str())
            .map(PathBuf::from)
            .collect();
        debug!("Found testpaths in pyproject.toml: {:?}", config.testpaths);
    }

    if let Some(python_files) = ini_options
        .and_then(|i| i.get("python_files"))
        .and_then(|t| t.as_array())
    {
        config.python_files = python_files
            .iter()
            .filter_map(|v| v.as_str())
            .map(String::from)
            .collect();
        debug!(
            "Found python_files in pyproject.toml: {:?}",
            config.python_files
        );
    }

    if let Some(python_classes) = ini_options
        .and_then(|i| i.get("python_classes"))
        .and_then(|t| t.as_array())
    {
        config.python_classes = python_classes
            .iter()
            .filter_map(|v| v.as_str())
            .map(String::from)
            .collect();
        debug!(
            "Found python_classes in pyproject.toml: {:?}",
            config.python_classes
        );
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_read_pytest_config_with_testpaths() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

        let content = indoc! {r#"
            [tool.pytest.ini_options]
            testpaths = ["tests", "test"]
        "#};

        fs::write(&pyproject_path, content).unwrap();

        let config = read_pytest_config(temp_dir.path());
        assert_eq!(config.testpaths.len(), 2);
        assert_eq!(config.testpaths[0], PathBuf::from("tests"));
        assert_eq!(config.testpaths[1], PathBuf::from("test"));
    }

    #[test]
    fn test_read_pytest_config_no_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = read_pytest_config(temp_dir.path());
        assert!(config.testpaths.is_empty());
        assert!(config.python_files.is_empty());
        assert!(config.python_classes.is_empty());
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
    }

    #[test]
    fn test_read_pytest_config_with_python_files() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

        let content = indoc! {r#"
            [tool.pytest.ini_options]
            python_files = ["test_*.py", "*_test.py", "check_*.py"]
        "#};

        fs::write(&pyproject_path, content).unwrap();

        let config = read_pytest_config(temp_dir.path());
        assert_eq!(config.python_files.len(), 3);
        assert_eq!(config.python_files[0], "test_*.py");
        assert_eq!(config.python_files[1], "*_test.py");
        assert_eq!(config.python_files[2], "check_*.py");
    }

    #[test]
    fn test_read_pytest_config_with_python_classes() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

        let content = indoc! {r#"
            [tool.pytest.ini_options]
            python_classes = ["Test*", "Check*", "*Suite"]
        "#};

        fs::write(&pyproject_path, content).unwrap();

        let config = read_pytest_config(temp_dir.path());
        assert_eq!(config.python_classes.len(), 3);
        assert_eq!(config.python_classes[0], "Test*");
        assert_eq!(config.python_classes[1], "Check*");
        assert_eq!(config.python_classes[2], "*Suite");
    }
}
