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
/// Exits the process with the pytest exit code.
pub fn execute_tests(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    pytest_args: Vec<String>,
    working_dir: Option<&Path>,
) {
    let mut run_cmd = Command::new(program);
    run_cmd.args(initial_args);

    // Set working directory and constrain pytest's collection scope
    if let Some(dir) = working_dir {
        run_cmd.current_dir(dir);
        // Add --rootdir to prevent pytest from traversing up the directory tree during
        // its collection phase. Without this, pytest searches upward for config files
        // and can hit protected Windows system directories like "C:\Documents and Settings",
        // causing PermissionError even when we provide explicit test node IDs.
        run_cmd.arg("--rootdir");
        run_cmd.arg(dir);
    }

    // Add test nodes after rootdir
    run_cmd.args(test_nodes);
    run_cmd.args(pytest_args);

    let run_status = run_cmd.status().expect("Failed to execute run command");

    std::process::exit(run_status.code().unwrap_or(1));
}
