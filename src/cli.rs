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

    /// Arguments to pass directly to pytest
    #[arg(last = true)]
    pub pytest_args: Vec<String>,
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
}
