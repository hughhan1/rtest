use clap::Parser;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Package manager to use (e.g., 'uv', 'pipenv', 'poetry')
    #[arg(long, default_value = "python")]
    pub package_manager: String,

    /// Environment variables to set for pytest (e.g., 'KEY=VALUE')
    #[arg(long, short, num_args = 0..)]
    pub env: Vec<String>,

    /// Number of processes to run tests in parallel
    #[arg(long, short = 'n', alias = "numprocesses")]
    pub numprocesses: Option<String>,

    /// Maximum number of worker processes
    #[arg(long)]
    pub maxprocesses: Option<usize>,

    /// Distribution mode for parallel execution
    #[arg(long, default_value = "load")]
    pub dist: String,

    /// Arguments to pass directly to pytest
    #[arg(last = true)]
    pub pytest_args: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum NumProcesses {
    Auto,
    Logical,
    Count(usize),
}

impl Args {
    pub fn get_num_processes(&self) -> Option<NumProcesses> {
        self.numprocesses.as_ref().map(|s| match s.as_str() {
            "auto" => NumProcesses::Auto,
            "logical" => NumProcesses::Logical,
            _ => NumProcesses::Count(s.parse().unwrap_or(1)),
        })
    }

    pub fn validate_dist(&self) -> Result<(), String> {
        match self.dist.as_str() {
            "load" => Ok(()),
            other => Err(format!(
                "Distribution mode '{other}' is not yet implemented. Only 'load' is supported."
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn test_cli_parsing_defaults() {
        let args = Args::parse_from(["rustic"]);

        assert_eq!(args.package_manager, "python");
        assert!(args.env.is_empty());
        assert!(args.pytest_args.is_empty());
        assert!(args.numprocesses.is_none());
        assert!(args.maxprocesses.is_none());
        assert_eq!(args.dist, "load");
    }

    #[test]
    fn test_cli_parsing_with_package_manager() {
        let args = Args::parse_from(["rustic", "--package-manager", "uv"]);

        assert_eq!(args.package_manager, "uv");
    }

    #[test]
    fn test_cli_parsing_with_env_vars() {
        let args = Args::parse_from(["rustic", "--env", "DEBUG=1", "--env", "TEST=true"]);

        assert_eq!(args.env, vec!["DEBUG=1", "TEST=true"]);
    }

    #[test]
    fn test_cli_parsing_with_pytest_args() {
        let args = Args::parse_from(["rustic", "--", "-v", "--tb=short", "test_file.py"]);

        assert_eq!(args.pytest_args, vec!["-v", "--tb=short", "test_file.py"]);
    }

    #[test]
    fn test_cli_parsing_all_options() {
        let args = Args::parse_from([
            "rustic",
            "--package-manager",
            "poetry",
            "--env",
            "DEBUG=1",
            "--env",
            "ENV=test",
            "--",
            "-v",
            "test_file.py",
        ]);

        assert_eq!(args.package_manager, "poetry");
        assert_eq!(args.env, vec!["DEBUG=1", "ENV=test"]);
        assert_eq!(args.pytest_args, vec!["-v", "test_file.py"]);
    }

    #[test]
    fn test_cli_help_generation() {
        let mut cmd = Args::command();
        let help = cmd.render_help();

        assert!(help.to_string().contains("package-manager"));
        assert!(help.to_string().contains("env"));
        assert!(help.to_string().contains("PYTEST_ARGS"));
    }

    #[test]
    fn test_cli_parsing_with_numprocesses() {
        let args = Args::parse_from(["rustic", "-n", "4"]);
        assert_eq!(args.numprocesses, Some("4".to_string()));

        let args = Args::parse_from(["rustic", "--numprocesses", "auto"]);
        assert_eq!(args.numprocesses, Some("auto".to_string()));
    }

    #[test]
    fn test_cli_parsing_with_maxprocesses() {
        let args = Args::parse_from(["rustic", "--maxprocesses", "8"]);
        assert_eq!(args.maxprocesses, Some(8));
    }

    #[test]
    fn test_cli_parsing_with_dist() {
        let args = Args::parse_from(["rustic", "--dist", "load"]);
        assert_eq!(args.dist, "load");
    }

    #[test]
    fn test_get_num_processes() {
        let args = Args::parse_from(["rustic", "-n", "auto"]);
        assert!(matches!(args.get_num_processes(), Some(NumProcesses::Auto)));

        let args = Args::parse_from(["rustic", "-n", "logical"]);
        assert!(matches!(
            args.get_num_processes(),
            Some(NumProcesses::Logical)
        ));

        let args = Args::parse_from(["rustic", "-n", "4"]);
        assert!(matches!(
            args.get_num_processes(),
            Some(NumProcesses::Count(4))
        ));

        let args = Args::parse_from(["rustic"]);
        assert!(args.get_num_processes().is_none());
    }

    #[test]
    fn test_validate_dist() {
        let args = Args::parse_from(["rustic", "--dist", "load"]);
        assert!(args.validate_dist().is_ok());

        let args = Args::parse_from(["rustic", "--dist", "loadfile"]);
        assert!(args.validate_dist().is_err());
    }
}
