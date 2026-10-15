use crate::scheduler::DistributionMode;
use clap::{Parser, ValueEnum};
#[cfg(feature = "extension-module")]
use std::io::{self, Write};
use std::num::NonZeroUsize;

/// Exit codes for the CLI.
/// These follow standard conventions and match pytest's exit codes where applicable.
pub mod exit_codes {
    /// Successful execution.
    pub const OK: i32 = 0;
    /// Tests were collected and run but some failed.
    pub const TESTS_FAILED: i32 = 1;
    /// File or directory not found.
    pub const USAGE_ERROR: i32 = 4;
}

/// Test runner backend to use.
#[derive(Debug, Clone, Copy, Default, ValueEnum)]
pub enum Runner {
    /// No pytest required
    Native,
    /// Full pytest compatibility
    #[default]
    Pytest,
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// Environment variables to set for pytest (e.g., 'KEY=VALUE')
    #[arg(long, short)]
    pub env: Vec<String>,

    /// Worker count: a positive integer, auto, or logical
    #[arg(long, short = 'n', alias = "numprocesses")]
    pub numprocesses: Option<NumProcesses>,

    /// Maximum number of worker processes (must be positive)
    #[arg(long)]
    pub maxprocesses: Option<NonZeroUsize>,

    /// Distribution mode for parallel execution
    #[arg(long, default_value = "load")]
    pub dist: DistributionMode,

    /// Collect tests only, don't run them
    #[arg(long)]
    pub collect_only: bool,

    /// Where to write pytest fallback nodeids: stdout, stderr, none, or a file path
    #[arg(long, default_value = "stdout")]
    pub cannot_expand_report: String,

    /// Test runner backend
    #[arg(long, value_enum, default_value = "pytest")]
    pub runner: Runner,

    /// Test files or directories to run
    #[arg(help = "Test files or directories to run")]
    pub files: Vec<String>,
}

#[cfg(feature = "extension-module")]
impl Args {
    /// Open before collection so a failed/empty run cannot leave a stale report.
    pub(crate) fn report_writer(&self) -> io::Result<Box<dyn Write>> {
        match self.cannot_expand_report.as_str() {
            "stdout" => Ok(Box::new(io::stdout())),
            "stderr" => Ok(Box::new(io::stderr())),
            "none" => Ok(Box::new(io::sink())),
            path => Ok(Box::new(std::fs::File::create(path)?)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumProcesses {
    Auto,
    Logical,
    Count(NonZeroUsize),
}

impl std::str::FromStr for NumProcesses {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "auto" => Ok(Self::Auto),
            "logical" => Ok(Self::Logical),
            _ => value.parse().map(Self::Count).map_err(|_| {
                format!(
                    "Invalid worker count: {value}; expected a positive integer, auto, or logical"
                )
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_parsing_defaults() {
        let args = Args::parse_from(["rtest"]);

        assert!(args.env.is_empty());
        assert!(args.numprocesses.is_none());
        assert!(args.maxprocesses.is_none());
        assert_eq!(args.dist, DistributionMode::Load);
        assert!(!args.collect_only);
        assert!(args.files.is_empty());
        assert!(matches!(args.runner, Runner::Pytest));
    }

    #[test]
    fn test_cli_parsing_with_env_vars() {
        let args = Args::parse_from(["rtest", "--env", "DEBUG=1", "--env", "TEST=true"]);

        assert_eq!(args.env, vec!["DEBUG=1", "TEST=true"]);
    }

    #[test]
    fn test_cli_parsing_with_numprocesses() {
        let args = Args::parse_from(["rtest", "-n", "4"]);
        assert_eq!(
            args.numprocesses,
            Some(NumProcesses::Count(NonZeroUsize::new(4).expect("positive")))
        );

        let args = Args::parse_from(["rtest", "--numprocesses", "auto"]);
        assert_eq!(args.numprocesses, Some(NumProcesses::Auto));

        let args = Args::parse_from(["rtest", "-n", "logical"]);
        assert_eq!(args.numprocesses, Some(NumProcesses::Logical));
    }

    #[test]
    fn test_cli_parsing_with_maxprocesses() {
        let args = Args::parse_from(["rtest", "--maxprocesses", "8"]);
        assert_eq!(args.maxprocesses.map(NonZeroUsize::get), Some(8));
    }

    #[test]
    fn test_cli_parsing_with_dist() {
        let args = Args::parse_from(["rtest", "--dist", "loadscope"]);
        assert_eq!(args.dist, DistributionMode::LoadScope);
    }

    #[test]
    fn test_invalid_cli_configuration_rejected_at_parse_boundary() {
        for arguments in [
            ["rtest", "-n", "0"],
            ["rtest", "-n", "invalid"],
            ["rtest", "--maxprocesses", "0"],
            ["rtest", "--dist", "invalid"],
            ["rtest", "--dist", "loadgroup"],
        ] {
            assert!(Args::try_parse_from(arguments).is_err());
        }
    }

    #[test]
    fn test_cli_parsing_with_collect_only() {
        let args = Args::parse_from(["rtest", "--collect-only"]);
        assert!(args.collect_only);
    }

    #[test]
    fn test_cli_parsing_with_runner() {
        let args = Args::parse_from(["rtest", "--runner", "native"]);
        assert!(matches!(args.runner, Runner::Native));
    }
}
