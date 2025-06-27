//! Handles the execution of pytest with collected test nodes.

use std::process::Command;

/// Executes pytest with the given program, initial arguments, collected test nodes, and additional pytest arguments.
///
/// # Arguments
///
/// * `program` - The pytest executable or package manager command.
/// * `initial_args` - Initial arguments to pass to the program (e.g., `run` for `uv`).
/// * `test_nodes` - A `Vec<String>` of test node IDs to execute.
/// * `pytest_args` - Additional arguments to pass directly to pytest.
///
/// Exits the process with the pytest exit code.
pub fn execute_tests(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    pytest_args: Vec<String>,
) {
    let mut run_cmd = Command::new(program);
    run_cmd.args(initial_args);
    run_cmd.args(test_nodes);
    run_cmd.args(pytest_args);

    let run_status = run_cmd.status().expect("Failed to execute run command");

    std::process::exit(run_status.code().unwrap_or(1));
}
