// Tests to verify that examples in documentation work correctly

use std::process::Command;

#[test]
fn test_basic_usage_examples() {
    // Test basic sequential usage (should work even without pytest)
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute help command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: rtest"));
}

#[test]
fn test_parallel_usage_examples() {
    // These are the basic usage patterns that should be documented

    let test_cases = vec![
        // Basic parallel execution
        vec!["-n", "2"],
        // Auto CPU detection
        vec!["-n", "auto"],
        // Logical CPU detection
        vec!["-n", "logical"],
        // With max processes limit
        vec!["-n", "4", "--maxprocesses", "2"],
        // Explicit load distribution
        vec!["--dist", "load"],
        // Combined options
        vec!["-n", "3", "--dist", "load", "--maxprocesses", "4"],
    ];

    for args in test_cases {
        let mut cmd_args = vec!["run", "--"];
        cmd_args.extend(args.iter());

        let output = Command::new("cargo")
            .args(cmd_args)
            .output()
            .expect("Failed to execute command");

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Should not have argument parsing errors
        assert!(
            !stderr.contains("error: invalid value"),
            "Command failed with args {args:?}: {stderr}"
        );
        assert!(
            !stderr.contains("error: unexpected argument"),
            "Command failed with args {args:?}: {stderr}"
        );
    }
}

#[test]
fn test_error_cases_examples() {
    // Test that documented error cases work as expected

    // Invalid distribution mode
    let output = Command::new("cargo")
        .args(["run", "--", "--dist", "invalid"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("not yet implemented") || stderr.contains("Only 'load' is supported"));
}

#[test]
fn test_version_and_help_accessibility() {
    // Ensure basic documentation commands work

    let version_output = Command::new("cargo")
        .args(["run", "--", "--version"])
        .output()
        .expect("Failed to execute version command");

    assert!(version_output.status.success());

    let help_output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute help command");

    assert!(help_output.status.success());
    let help_text = String::from_utf8_lossy(&help_output.stdout);

    // Should contain key usage information
    assert!(help_text.contains("numprocesses"));
    assert!(help_text.contains("maxprocesses"));
    assert!(help_text.contains("dist"));
}

#[test]
fn test_pytest_passthrough_args() {
    // Test that pytest arguments are properly passed through
    let output = Command::new("cargo")
        .args(["run", "--", "-n", "2", "--", "-v", "--tb=short"])
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should not have argument parsing errors for pytest args
    assert!(!stderr.contains("error: invalid value"));
    assert!(!stderr.contains("error: unexpected argument"));
}

#[test]
fn test_package_manager_integration() {
    // Test that parallel execution works with different package managers
    let package_managers = ["python", "uv", "pipenv", "poetry"];

    for pm in package_managers {
        let output = Command::new("cargo")
            .args(["run", "--", "--package-manager", pm, "-n", "2"])
            .output()
            .expect("Failed to execute command");

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Should accept the package manager argument
        assert!(!stderr.contains("error: invalid value"));
    }
}
