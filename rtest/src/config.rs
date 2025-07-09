//! Configuration parsing for pytest settings from pyproject.toml

use log::debug;
use std::path::{Path, PathBuf};
use toml::Value;

/// Pytest configuration from pyproject.toml
#[derive(Debug, Clone, Default)]
pub struct PytestConfig {
    /// Test paths to search for tests
    pub testpaths: Vec<PathBuf>,
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

    if let Some(testpaths) = toml_value
        .get("tool")
        .and_then(|t| t.get("pytest"))
        .and_then(|p| p.get("ini_options"))
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

    config
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_read_pytest_config_with_testpaths() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

        let content = r#"
[tool.pytest.ini_options]
testpaths = ["tests", "test"]
"#;

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
    }

    #[test]
    fn test_read_pytest_config_no_testpaths() {
        let temp_dir = TempDir::new().unwrap();
        let pyproject_path = temp_dir.path().join("pyproject.toml");

        let content = r#"
[tool.pytest.ini_options]
filterwarnings = ["error"]
"#;

        fs::write(&pyproject_path, content).unwrap();

        let config = read_pytest_config(temp_dir.path());
        assert!(config.testpaths.is_empty());
    }
}
