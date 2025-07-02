use std::process::Command;

#[test]
fn test_cli_help_includes_parallel_options() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should include new parallel execution options
    assert!(stdout.contains("numprocesses"));
    assert!(stdout.contains("maxprocesses"));
    assert!(stdout.contains("dist"));
    assert!(stdout.contains("Number of processes to run tests in parallel"));
    assert!(stdout.contains("Maximum number of worker processes"));
    assert!(stdout.contains("Distribution mode for parallel execution"));
}

#[test]
fn test_invalid_distribution_mode_error() {
    let output = Command::new("cargo")
        .args(["run", "--", "--dist", "loadfile"])
        .output()
        .expect("Failed to execute command");

    // Should fail with proper error message
    assert!(!output.status.success());

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should have proper error message about unsupported distribution mode
    assert!(
        stderr.contains("Distribution mode 'loadfile' is not yet implemented")
            || stderr.contains("Only 'load' is supported")
    );
}

#[test]
fn test_valid_distribution_mode_load() {
    // Test that load mode is accepted (even if pytest fails)
    let output = Command::new("cargo")
        .args(["run", "--", "--dist", "load"])
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should NOT have distribution mode error
    assert!(!stderr.contains("Distribution mode 'load' is not yet implemented"));
}

#[test]
fn test_numprocesses_argument_parsing() {
    // Test various numprocesses values are accepted
    let test_cases = ["1", "2", "4", "auto", "logical"];

    for &num_processes in &test_cases {
        let output = Command::new("cargo")
            .args(["run", "--", "-n", num_processes])
            .output()
            .expect("Failed to execute command");

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Should not have argument parsing errors
        assert!(!stderr.contains("error: invalid value"));
        assert!(!stderr.contains("argument"));
    }
}

#[test]
fn test_maxprocesses_argument_parsing() {
    let output = Command::new("cargo")
        .args(["run", "--", "--maxprocesses", "4"])
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should not have argument parsing errors
    assert!(!stderr.contains("error: invalid value"));
    assert!(!stderr.contains("argument"));
}

#[test]
fn test_combined_parallel_arguments() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "-n",
            "4",
            "--maxprocesses",
            "2",
            "--dist",
            "load",
        ])
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should not have argument parsing errors
    assert!(!stderr.contains("error: invalid value"));
    assert!(!stderr.contains("argument"));
    // Should not have distribution mode errors
    assert!(!stderr.contains("Distribution mode"));
}

#[test]
fn test_cli_version_still_works() {
    let output = Command::new("cargo")
        .args(["run", "--", "--version"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show version information
    assert!(stdout.contains("rtest") || stdout.contains("0.1.0"));
}
