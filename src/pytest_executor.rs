//! Handles the execution of pytest with collected test nodes.

use std::path::Path;
use std::process::Command;

pub(crate) fn pytest_command(
    program: &str,
    initial_args: &[String],
    test_nodes: &[String],
    pytest_args: &[String],
    working_dir: Option<&Path>,
    env_vars: &[(String, String)],
) -> Command {
    let mut command = Command::new(program);
    command
        .args(initial_args)
        .envs(env_vars.iter().map(|(key, value)| (key, value)));

    // Keep pytest from searching parent directories for configuration, including
    // protected system directories on Windows.
    if let Some(dir) = working_dir {
        command.current_dir(dir).arg("--rootdir").arg(dir);
    }

    command.args(test_nodes).args(pytest_args);
    command
}

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
    let run_status = match pytest_command(
        program,
        initial_args,
        &test_nodes,
        &pytest_args,
        working_dir,
        env_vars,
    )
    .status()
    {
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
