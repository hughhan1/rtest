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
///
/// Returns the exit code from pytest.
pub fn execute_tests(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    pytest_args: Vec<String>,
    working_dir: Option<&Path>,
) -> i32 {
    let mut run_cmd = Command::new(program);
    run_cmd.args(initial_args);

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
            eprintln!("Failed to execute pytest command: {e}");
            return 1;
        }
    };

    run_status.code().unwrap_or(1)
}
