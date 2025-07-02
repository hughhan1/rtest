pub struct PytestRunner {
    pub program: String,
    pub initial_args: Vec<String>,
}

impl PytestRunner {
    pub fn new(env_vars: Vec<String>) -> Self {
        let program = "python3".to_string();
        let initial_args = vec!["-m".to_string(), "pytest".to_string()];

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
        let runner = PytestRunner::new(vec![]);

        assert_eq!(runner.program, "python3");
        assert_eq!(runner.initial_args, vec!["-m", "pytest"]);
    }

    #[test]
    fn test_env_vars_acknowledged() {
        let env_vars = vec!["DEBUG=1".to_string(), "TEST_ENV=staging".to_string()];
        let runner = PytestRunner::new(env_vars);

        // The runner should be created successfully
        // (Environment variables are currently just acknowledged, not stored)
        assert_eq!(runner.program, "python3");
        assert_eq!(runner.initial_args, vec!["-m", "pytest"]);
    }
}
