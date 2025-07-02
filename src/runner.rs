use log::debug;
use pyo3::prelude::*;

pub struct PytestRunner {
    pub program: String,
    pub initial_args: Vec<String>,
}

impl PytestRunner {
    /// Used by Python bindings in [`crate::run_tests`]
    #[allow(dead_code)]
    pub fn from_current_python(py: Python) -> Self {
        // Use the same Python executable that's running this program
        let python_path = py
            .import_bound("sys")
            .and_then(|sys| sys.getattr("executable"))
            .and_then(|exe| exe.extract::<String>())
            .unwrap_or_else(|_| "python3".to_string());

        let initial_args = vec!["-m".to_string(), "pytest".to_string()];
        debug!("Running {} {}", python_path, initial_args.join(" "));

        Self {
            program: python_path,
            initial_args,
        }
    }

    pub fn new(package_manager: String, env_vars: Vec<String>) -> Self {
        let mut program = "python3".to_string();
        let mut initial_args = vec!["-m".to_string(), "pytest".to_string()];

        match package_manager.as_str() {
            "uv" => {
                program = "uv".to_string();
                initial_args = vec![
                    "run".to_string(),
                    "python".to_string(),
                    "-m".to_string(),
                    "pytest".to_string(),
                ];
            }
            "pipenv" => {
                program = "pipenv".to_string();
                initial_args = vec![
                    "run".to_string(),
                    "python".to_string(),
                    "-m".to_string(),
                    "pytest".to_string(),
                ];
            }
            "poetry" => {
                program = "poetry".to_string();
                initial_args = vec![
                    "run".to_string(),
                    "python".to_string(),
                    "-m".to_string(),
                    "pytest".to_string(),
                ];
            }
            _ => {}
        }

        // Apply environment variables (though this is typically done before command execution)
        // For now, we'll just acknowledge them, but a real implementation would set them
        // on the Command object before spawning.
        for env_var in env_vars {
            println!("Note: Environment variable '{env_var}' would be set for pytest.");
        }

        println!("Pytest command: {} {}", program, initial_args.join(" "));

        PytestRunner {
            program,
            initial_args,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_python_runner() {
        let runner = PytestRunner::new("python".to_string(), vec![]);

        assert_eq!(runner.program, "python3");
        assert_eq!(runner.initial_args, vec!["-m", "pytest"]);
    }

    #[test]
    fn test_uv_runner() {
        let runner = PytestRunner::new("uv".to_string(), vec![]);

        assert_eq!(runner.program, "uv");
        assert_eq!(runner.initial_args, vec!["run", "python", "-m", "pytest"]);
    }

    #[test]
    fn test_pipenv_runner() {
        let runner = PytestRunner::new("pipenv".to_string(), vec![]);

        assert_eq!(runner.program, "pipenv");
        assert_eq!(runner.initial_args, vec!["run", "python", "-m", "pytest"]);
    }

    #[test]
    fn test_poetry_runner() {
        let runner = PytestRunner::new("poetry".to_string(), vec![]);

        assert_eq!(runner.program, "poetry");
        assert_eq!(runner.initial_args, vec!["run", "python", "-m", "pytest"]);
    }

    #[test]
    fn test_unknown_package_manager() {
        let runner = PytestRunner::new("unknown".to_string(), vec![]);

        // Should default to python3
        assert_eq!(runner.program, "python3");
        assert_eq!(runner.initial_args, vec!["-m", "pytest"]);
    }

    #[test]
    fn test_env_vars_acknowledged() {
        let env_vars = vec!["DEBUG=1".to_string(), "TEST_ENV=staging".to_string()];
        let runner = PytestRunner::new("python".to_string(), env_vars);

        // The runner should be created successfully
        // (Environment variables are currently just acknowledged, not stored)
        assert_eq!(runner.program, "python3");
    }
}
