//! Handles the execution of pytest with collected test nodes.

use std::path::Path;
use std::process::Command;

/// Executes pytest with the given program, initial arguments, collected test nodes, and additional pytest arguments.
///
/// # Arguments
///
/// * `program` - The pytest executable or package manager command.
/// * `initial_args` - Initial arguments to pass to the program (e.g., `run` for `uv`).
/// * `test_nodes` - A `Vec<String>` of test node IDs to execute.
/// * `pytest_args` - Additional arguments to pass directly to pytest.
/// * `working_dir` - Optional working directory for pytest execution.
/// * `env_vars` - Environment variables to set for pytest execution.
///
/// Returns the exit code from pytest.
pub fn execute_tests(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    pytest_args: Vec<String>,
    working_dir: Option<&Path>,
    env_vars: &[(String, String)],
) -> i32 {
    let mut run_cmd = Command::new(program);
    run_cmd.args(initial_args);

    // Set environment variables
    for (key, value) in env_vars {
        run_cmd.env(key, value);
    }

    if let Some(dir) = working_dir {
        run_cmd.current_dir(dir);
        run_cmd.arg("--rootdir");
        run_cmd.arg(dir);
    }

    run_cmd.args(test_nodes);
    run_cmd.args(pytest_args);

    let run_status = match run_cmd.status() {
        Ok(status) => status,
        Err(e) => {
            log::error!("Failed to execute pytest command: {e}");
            return 1;
        }
    };

    run_status.code().unwrap_or_else(|| {
        log::warn!("Pytest process terminated by signal");
        1
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Returns a (program, initial_args) pair that exits with the given code
    /// on the current platform.
    fn exit_with(code: i32) -> (&'static str, Vec<String>) {
        if cfg!(windows) {
            ("cmd", vec!["/C".into(), "exit".into(), code.to_string()])
        } else {
            ("sh", vec!["-c".into(), format!("exit {code}"), "sh".into()])
        }
    }

    #[test]
    fn test_execute_tests_success_exit_code() {
        let (program, initial_args) = exit_with(0);
        let code = execute_tests(program, &initial_args, vec![], vec![], None, &[]);
        assert_eq!(code, 0);
    }

    #[test]
    fn test_execute_tests_failure_exit_code() {
        let (program, initial_args) = exit_with(3);
        let code = execute_tests(program, &initial_args, vec![], vec![], None, &[]);
        assert_eq!(code, 3);
    }

    #[test]
    fn test_execute_tests_nonexistent_program_returns_one() {
        let code = execute_tests(
            "rtest_nonexistent_program_xyz",
            &[],
            vec![],
            vec![],
            None,
            &[],
        );
        assert_eq!(code, 1);
    }

    #[test]
    fn test_execute_tests_passes_env_vars() {
        // The child exits 0 only when the env var was propagated correctly.
        let (program, args): (&str, Vec<String>) = if cfg!(windows) {
            (
                "cmd",
                vec![
                    "/C".into(),
                    "if \"%RTEST_ENV_CHECK%\"==\"present\" (exit 0) else (exit 1)".into(),
                ],
            )
        } else {
            (
                "sh",
                vec![
                    "-c".into(),
                    "[ \"$RTEST_ENV_CHECK\" = present ]".into(),
                    "sh".into(),
                ],
            )
        };
        let env = vec![("RTEST_ENV_CHECK".to_string(), "present".to_string())];
        let code = execute_tests(program, &args, vec![], vec![], None, &env);
        assert_eq!(
            code, 0,
            "environment variable should reach the child process"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_tests_sets_working_dir() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonicalize");
        // `--rootdir <dir>` is appended by execute_tests; `true` ignores extra
        // args and exits 0, confirming the working-dir branch is exercised.
        let code = execute_tests("true", &[], vec![], vec![], Some(&root), &[]);
        assert_eq!(code, 0);
    }
}
