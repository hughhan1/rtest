//! Handles the execution of pytest with collected test nodes.

use crate::cache::{parse_junit_xml, TestOutcome};
use std::path::Path;
use std::process::Command;

pub struct PytestExecutionResult {
    pub exit_code: i32,
    pub outcomes: Vec<(String, TestOutcome)>,
}

/// Executes pytest with the given program, initial arguments, collected test nodes, and additional pytest arguments.
///
/// When `junit_xml` is set, passes `--junitxml=PATH` and parses structured outcomes from that file.
pub fn execute_tests(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    pytest_args: Vec<String>,
    working_dir: Option<&Path>,
    env_vars: &[(String, String)],
    junit_xml: Option<&Path>,
) -> PytestExecutionResult {
    let mut run_cmd = Command::new(program);
    run_cmd.args(initial_args);

    for (key, value) in env_vars {
        run_cmd.env(key, value);
    }

    if let Some(dir) = working_dir {
        run_cmd.current_dir(dir);
        run_cmd.arg("--rootdir");
        run_cmd.arg(dir);
    }

    run_cmd.args(&test_nodes);
    run_cmd.args(&pytest_args);

    if let Some(path) = junit_xml {
        run_cmd.arg(format!("--junitxml={}", path.display()));
    }

    let run_status = match run_cmd.status() {
        Ok(status) => status,
        Err(e) => {
            log::error!("Failed to execute pytest command: {e}");
            return PytestExecutionResult {
                exit_code: 1,
                outcomes: vec![],
            };
        }
    };

    let exit_code = run_status.code().unwrap_or_else(|| {
        log::warn!("Pytest process terminated by signal");
        1
    });

    let outcomes = if let Some(path) = junit_xml {
        parse_junit_xml(path).unwrap_or_else(|e| {
            log::warn!("Failed to parse junit xml {}: {e}", path.display());
            Vec::new()
        })
    } else {
        Vec::new()
    };

    PytestExecutionResult {
        exit_code,
        outcomes,
    }
}

/// Convenience wrapper when junit outcomes are not needed.
pub fn execute_tests_exit_code(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    pytest_args: Vec<String>,
    working_dir: Option<&Path>,
    env_vars: &[(String, String)],
) -> i32 {
    execute_tests(
        program,
        initial_args,
        test_nodes,
        pytest_args,
        working_dir,
        env_vars,
        None,
    )
    .exit_code
}
