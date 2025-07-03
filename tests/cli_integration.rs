//! Integration tests for CLI functionality.

use std::process::Command;

mod common;

/// Test that the CLI shows help when requested
#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: rtest"));
    assert!(stdout.contains("--env"));
    assert!(stdout.contains("--numprocesses"));
    assert!(stdout.contains("--dist"));
}

/// Test that the CLI shows version when requested
#[test]
fn test_cli_version() {
    let output = Command::new("cargo")
        .args(["run", "--", "--version"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rtest"));
}

/// Test CLI argument parsing for distribution modes
#[test]
fn test_distribution_args() {
    // Test with default (load)
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[default: load]"));
}

/// Test that invalid arguments are rejected
#[test]
fn test_invalid_args() {
    let output = Command::new("cargo")
        .args(["run", "--", "--invalid-flag"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error") || stderr.contains("unrecognized"));
}

/// Test collection functionality with temporary test files
/// Note: This test will fail until the python executable issue is fixed,
/// but it validates the collection phase works correctly.
#[test]
fn test_collection_phase() {
    let (_temp_dir, project_path) = common::create_test_project();

    let rustic_binary = std::env::current_exe()
        .unwrap()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("rtest");

    let output = Command::new(&rustic_binary)
        .args(["--", "test_sample.py"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    // The collection should work (finding tests), but execution will fail
    // due to python executable not found
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Check that collection found the expected tests
    assert!(
        stdout.contains("test_sample.py::test_simple_function")
            || stderr.contains("test_sample.py::test_simple_function")
    );
    assert!(
        stdout.contains("test_sample.py::test_another_function")
            || stderr.contains("test_sample.py::test_another_function")
    );
    assert!(
        stdout.contains("test_sample.py::TestExampleClass::test_method_one")
            || stderr.contains("test_sample.py::TestExampleClass::test_method_one")
    );
    assert!(
        stdout.contains("test_sample.py::TestExampleClass::test_method_two")
            || stderr.contains("test_sample.py::TestExampleClass::test_method_two")
    );
}
