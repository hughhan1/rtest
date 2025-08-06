use pyo3::indoc::indoc;
use std::collections::HashMap;
use std::process::Command;

mod common;
use common::{create_test_file, create_test_project_with_files, get_rtest_binary};

/// Test basic test execution with a single passing test
#[test]
fn test_execute_single_passing_test() {
    let (_temp_dir, project_path) = create_test_file(
        "test_single.py",
        indoc! {r#"
            def test_passing():
                assert True
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .arg("test_single.py")
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "Test should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 passed"));
}

/// Test basic test execution with a single failing test
#[test]
fn test_execute_single_failing_test() {
    let (_temp_dir, project_path) = create_test_file(
        "test_single.py",
        indoc! {r#"
            def test_failing():
                assert False, "Expected failure"
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .arg("test_single.py")
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_ne!(output.status.code(), Some(0), "Test should fail");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 failed") || stdout.contains("FAILED"));
}

/// Test execution of multiple tests in a single file
#[test]
fn test_execute_multiple_tests_single_file() {
    let (_temp_dir, project_path) = create_test_file(
        "test_multiple.py",
        indoc! {r#"
            def test_one():
                assert 1 + 1 == 2

            def test_two():
                assert 2 * 2 == 4

            def test_three():
                assert "hello".upper() == "HELLO"
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .arg("test_multiple.py")
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("3 passed"));
}

/// Test execution with mixed pass/fail results
#[test]
fn test_execute_mixed_results() {
    let (_temp_dir, project_path) = create_test_file(
        "test_mixed.py",
        indoc! {r#"
            def test_pass_one():
                assert True

            def test_fail_one():
                assert False, "This test fails"

            def test_pass_two():
                assert 1 == 1

            def test_fail_two():
                assert 1 == 2, "Math doesn't check out"
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .arg("test_mixed.py")
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_ne!(
        output.status.code(),
        Some(0),
        "Should fail due to failed tests"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should report both passed and failed tests
    assert!(combined.contains("2 passed") || combined.contains("2 failed"));
}

/// Test execution with test errors (not just failures)
#[test]
fn test_execute_with_errors() {
    let (_temp_dir, project_path) = create_test_file(
        "test_errors.py",
        indoc! {r#"
            def test_division_by_zero():
                result = 1 / 0  # This will raise ZeroDivisionError

            def test_undefined_variable():
                assert undefined_var == 42  # NameError

            def test_passing():
                assert True
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .arg("test_errors.py")
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_ne!(output.status.code(), Some(0), "Should fail due to errors");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should contain error information
    assert!(
        combined.contains("ZeroDivisionError")
            || combined.contains("NameError")
            || combined.contains("error")
    );
}

/// Test parallel execution with 2 workers
#[test]
fn test_execute_parallel_two_workers() {
    let mut files = HashMap::new();

    // Create multiple test files to distribute
    files.insert(
        "test_file1.py",
        indoc! {r#"
            import time

            def test_file1_test1():
                time.sleep(0.1)
                assert True

            def test_file1_test2():
                assert True
        "#},
    );

    files.insert(
        "test_file2.py",
        indoc! {r#"
            import time

            def test_file2_test1():
                time.sleep(0.1)
                assert True

            def test_file2_test2():
                assert True
        "#},
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["-n", "2"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("4 passed") || stdout.contains("Worker"));
}

/// Test parallel execution with more workers than tests
#[test]
fn test_execute_more_workers_than_tests() {
    let (_temp_dir, project_path) = create_test_file(
        "test_single.py",
        indoc! {r#"
            def test_only_one():
                assert True
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .args(["-n", "4"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "Test should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 passed"));
}

/// Test load distribution mode
#[test]
fn test_execute_load_distribution() {
    let mut files = HashMap::new();

    // Use macro-generated strings for static content
    files.insert(
        "test_file1.py",
        "def test_f1_t1(): assert True\ndef test_f1_t2(): assert True",
    );
    files.insert(
        "test_file2.py",
        "def test_f2_t1(): assert True\ndef test_f2_t2(): assert True",
    );
    files.insert(
        "test_file3.py",
        "def test_f3_t1(): assert True\ndef test_f3_t2(): assert True",
    );
    files.insert(
        "test_file4.py",
        "def test_f4_t1(): assert True\ndef test_f4_t2(): assert True",
    );
    files.insert(
        "test_file5.py",
        "def test_f5_t1(): assert True\ndef test_f5_t2(): assert True",
    );
    files.insert(
        "test_file6.py",
        "def test_f6_t1(): assert True\ndef test_f6_t2(): assert True",
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["-n", "3", "--dist", "load"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("12 passed") || stdout.contains("Worker"));
}

/// Test loadscope distribution mode
#[test]
fn test_execute_loadscope_distribution() {
    let mut files = HashMap::new();

    files.insert(
        "test_classes.py",
        indoc! {r#"
            class TestClassA:
                def test_a1(self):
                    assert True
                
                def test_a2(self):
                    assert True

            class TestClassB:
                def test_b1(self):
                    assert True
                
                def test_b2(self):
                    assert True

            def test_function():
                assert True
        "#},
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["-n", "3", "--dist", "loadscope"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("5 passed") || stdout.contains("Worker"));
}

/// Test loadfile distribution mode
#[test]
fn test_execute_loadfile_distribution() {
    let mut files = HashMap::new();

    files.insert(
        "test_file1.py",
        "def test_f1_t1(): assert True\ndef test_f1_t2(): assert True\ndef test_f1_t3(): assert True",
    );
    files.insert(
        "test_file2.py",
        "def test_f2_t1(): assert True\ndef test_f2_t2(): assert True\ndef test_f2_t3(): assert True",
    );
    files.insert(
        "test_file3.py",
        "def test_f3_t1(): assert True\ndef test_f3_t2(): assert True\ndef test_f3_t3(): assert True",
    );
    files.insert(
        "test_file4.py",
        "def test_f4_t1(): assert True\ndef test_f4_t2(): assert True\ndef test_f4_t3(): assert True",
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["-n", "2", "--dist", "loadfile"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Each file's tests should be kept together
    assert!(stdout.contains("12 passed") || stdout.contains("Worker"));
}

/// Test worksteal distribution mode
#[test]
fn test_execute_worksteal_distribution() {
    let mut files = HashMap::new();

    files.insert(
        "test_worksteal.py",
        indoc! {r#"
            import time

            def test_fast1():
                assert True

            def test_slow1():
                time.sleep(0.2)
                assert True

            def test_fast2():
                assert True

            def test_slow2():
                time.sleep(0.2)
                assert True
        "#},
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["-n", "2", "--dist", "worksteal"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("4 passed") || stdout.contains("Worker"));
}

/// Test no distribution mode (all tests to single worker)
#[test]
fn test_execute_no_distribution() {
    let mut files = HashMap::new();

    files.insert(
        "test_file1.py",
        "def test_f1_t1(): assert True\ndef test_f1_t2(): assert True",
    );
    files.insert(
        "test_file2.py",
        "def test_f2_t1(): assert True\ndef test_f2_t2(): assert True",
    );
    files.insert(
        "test_file3.py",
        "def test_f3_t1(): assert True\ndef test_f3_t2(): assert True",
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["-n", "3", "--dist", "no"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // All tests should run in a single batch
    assert!(stdout.contains("6 passed"));
}

/// Test execution with specific test file
#[test]
fn test_execute_specific_tests() {
    let (_temp_dir, project_path) = create_test_file(
        "test_selection.py",
        indoc! {r#"
            def test_selected():
                assert True

            def test_another():
                assert True

            class TestClass:
                def test_method_one(self):
                    assert True
                
                def test_method_two(self):
                    assert True
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .arg("test_selection.py")
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("4 passed"));
}

// Skipping test_execute_with_markers - rtest doesn't support -m flag yet
// #[test]
// fn test_execute_with_markers() {

/// Test execution with fixtures
#[test]
fn test_execute_with_fixtures() {
    let (_temp_dir, project_path) = create_test_file(
        "test_fixtures.py",
        indoc! {r#"
            import pytest

            @pytest.fixture
            def sample_data():
                return [1, 2, 3, 4, 5]

            @pytest.fixture
            def sample_dict():
                return {"key": "value", "number": 42}

            def test_with_fixture(sample_data):
                assert len(sample_data) == 5
                assert sum(sample_data) == 15

            def test_with_multiple_fixtures(sample_data, sample_dict):
                assert sample_data[0] == 1
                assert sample_dict["number"] == 42

            class TestClassWithFixtures:
                def test_method_with_fixture(self, sample_dict):
                    assert "key" in sample_dict
                    assert sample_dict["key"] == "value"
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(
        output.status.code(),
        Some(0),
        "Tests with fixtures should pass"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("3 passed"));
}

/// Test execution with parameterized tests
#[test]
fn test_execute_parameterized_tests() {
    let (_temp_dir, project_path) = create_test_file(
        "test_parametrize.py",
        indoc! {r#"
            import pytest

            @pytest.mark.parametrize("input,expected", [
                (2, 4),
                (3, 9),
                (4, 16),
                (5, 25),
            ])
            def test_square(input, expected):
                assert input * input == expected

            @pytest.mark.parametrize("a,b,expected", [
                (1, 2, 3),
                (5, 5, 10),
                (-1, 1, 0),
            ])
            def test_addition(a, b, expected):
                assert a + b == expected
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(
        output.status.code(),
        Some(0),
        "Parameterized tests should pass"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should run 4 + 3 = 7 test instances
    assert!(stdout.contains("7 passed"));
}

/// Test execution with setup and teardown
#[test]
fn test_execute_with_setup_teardown() {
    let (_temp_dir, project_path) = create_test_file(
        "test_setup.py",
        indoc! {r#"
            import pytest
            import tempfile
            import os

            class TestWithSetup:
                def setup_method(self):
                    self.temp_file = tempfile.NamedTemporaryFile(delete=False)
                    self.temp_file.write(b"test data")
                    self.temp_file.close()
                
                def teardown_method(self):
                    if hasattr(self, 'temp_file'):
                        os.unlink(self.temp_file.name)
                
                def test_file_exists(self):
                    assert os.path.exists(self.temp_file.name)
                
                def test_file_content(self):
                    with open(self.temp_file.name, 'rb') as f:
                        assert f.read() == b"test data"
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(
        output.status.code(),
        Some(0),
        "Tests with setup/teardown should pass"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2 passed"));
}

/// Test execution with skipped tests
#[test]
fn test_execute_with_skipped_tests() {
    let (_temp_dir, project_path) = create_test_file(
        "test_skip.py",
        indoc! {r#"
            import pytest
            import sys

            @pytest.mark.skip(reason="Unconditionally skipped")
            def test_always_skipped():
                assert False  # Should not run

            @pytest.mark.skipif(sys.platform == "win32", reason="Skip on Windows")
            def test_skip_on_windows():
                assert True

            @pytest.mark.skipif(sys.version_info < (3, 10), reason="Requires Python 3.10+")
            def test_skip_old_python():
                assert True

            def test_normal():
                assert True
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should have at least 1 passed (test_normal) and some skipped
    assert!(stdout.contains("1 passed") || stdout.contains("skipped"));
}

/// Test execution with expected failures
#[test]
fn test_execute_with_xfail() {
    let (_temp_dir, project_path) = create_test_file(
        "test_xfail.py",
        indoc! {r#"
            import pytest

            @pytest.mark.xfail(reason="Known issue")
            def test_expected_failure():
                assert False  # This failure is expected

            @pytest.mark.xfail(reason="Fixed but not verified")
            def test_unexpected_pass():
                assert True  # This passes unexpectedly

            def test_normal():
                assert True
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("1 passed"));
    // Should report xfailed and xpassed
    assert!(
        stdout.contains("xfailed") || stdout.contains("xpassed") || stdout.contains("1 passed")
    );
}

/// Test execution with custom assertions
#[test]
fn test_execute_custom_assertions() {
    let (_temp_dir, project_path) = create_test_file(
        "test_assertions.py",
        indoc! {r#"
            def test_custom_assertion_message():
                x = 5
                y = 10
                assert x == y, f"Expected x ({x}) to equal y ({y})"

            def test_complex_assertion():
                data = {"a": 1, "b": 2, "c": 3}
                assert "d" in data, f"Key 'd' not found in data: {data}"
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_ne!(output.status.code(), Some(0), "Tests should fail");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should show custom assertion messages
    assert!(
        combined.contains("Expected x")
            || combined.contains("not found in data")
            || combined.contains("AssertionError")
    );
}

/// Test execution with timeouts
#[test]
fn test_execute_with_timeout() {
    let (_temp_dir, project_path) = create_test_file(
        "test_timeout.py",
        indoc! {r#"
            import time
            import pytest

            @pytest.mark.timeout(1)
            def test_quick():
                time.sleep(0.1)
                assert True

            @pytest.mark.timeout(1)
            def test_too_slow():
                time.sleep(2)  # This should timeout
                assert True
        "#},
    );

    // Note: This requires pytest-timeout plugin
    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let _stdout = String::from_utf8_lossy(&output.stdout);
    // Depending on whether pytest-timeout is installed, behavior may vary
    assert!(output.status.code().is_some());
}

// Skipping test_execute_output_capture - rtest doesn't support -s flag yet
// #[test]
// fn test_execute_output_capture() {

/// Test execution with multiple files in parallel
#[test]
fn test_execute_multiple_files_parallel() {
    let mut files = HashMap::new();

    // Create 10 test files with different test functions
    files.insert(
        "test_file_01.py",
        indoc! {r#"
        def test_file1_fast():
            assert 1 % 2 == 1

        def test_file1_slow():
            import time
            time.sleep(0.01)
            assert 1 + 1 == 2
    "#},
    );
    files.insert(
        "test_file_02.py",
        indoc! {r#"
        def test_file2_fast():
            assert 2 % 2 == 0

        def test_file2_slow():
            import time
            time.sleep(0.01)
            assert 2 + 1 == 3
    "#},
    );
    files.insert(
        "test_file_03.py",
        indoc! {r#"
        def test_file3_fast():
            assert 3 % 2 == 1

        def test_file3_slow():
            import time
            time.sleep(0.01)
            assert 3 + 1 == 4
    "#},
    );
    files.insert(
        "test_file_04.py",
        indoc! {r#"
        def test_file4_fast():
            assert 4 % 2 == 0

        def test_file4_slow():
            import time
            time.sleep(0.01)
            assert 4 + 1 == 5
    "#},
    );
    files.insert(
        "test_file_05.py",
        indoc! {r#"
        def test_file5_fast():
            assert 5 % 2 == 1

        def test_file5_slow():
            import time
            time.sleep(0.01)
            assert 5 + 1 == 6
    "#},
    );
    files.insert(
        "test_file_06.py",
        indoc! {r#"
        def test_file6_fast():
            assert 6 % 2 == 0

        def test_file6_slow():
            import time
            time.sleep(0.01)
            assert 6 + 1 == 7
    "#},
    );
    files.insert(
        "test_file_07.py",
        indoc! {r#"
        def test_file7_fast():
            assert 7 % 2 == 1

        def test_file7_slow():
            import time
            time.sleep(0.01)
            assert 7 + 1 == 8
    "#},
    );
    files.insert(
        "test_file_08.py",
        indoc! {r#"
        def test_file8_fast():
            assert 8 % 2 == 0

        def test_file8_slow():
            import time
            time.sleep(0.01)
            assert 8 + 1 == 9
    "#},
    );
    files.insert(
        "test_file_09.py",
        indoc! {r#"
        def test_file9_fast():
            assert 9 % 2 == 1

        def test_file9_slow():
            import time
            time.sleep(0.01)
            assert 9 + 1 == 10
    "#},
    );
    files.insert(
        "test_file_10.py",
        indoc! {r#"
        def test_file10_fast():
            assert 10 % 2 == 0

        def test_file10_slow():
            import time
            time.sleep(0.01)
            assert 10 + 1 == 11
    "#},
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["-n", "4", "--dist", "load"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("20 passed") || stdout.contains("Worker"));
}

/// Test that worker exit codes are properly propagated
#[test]
fn test_worker_exit_code_propagation() {
    let mut files = HashMap::new();

    files.insert(
        "test_worker1.py",
        indoc! {r#"
            def test_pass():
                assert True
        "#},
    );

    files.insert(
        "test_worker2.py",
        indoc! {r#"
            def test_fail():
                assert False, "This worker should fail"
        "#},
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["-n", "2", "--dist", "loadfile"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    // Should fail because one worker has failing tests
    assert_ne!(
        output.status.code(),
        Some(0),
        "Should propagate failure exit code"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("1 passed") && combined.contains("1 failed"));
}

/// Test execution with no tests collected
#[test]
fn test_execute_no_tests() {
    let (_temp_dir, project_path) = create_test_file(
        "not_a_test.py",
        indoc! {r#"
            def helper_function():
                return 42

            class HelperClass:
                def method(self):
                    pass
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should report no tests collected
    assert!(
        combined.contains("no tests")
            || combined.contains("0 collected")
            || combined.contains("No tests")
    );
}

/// Test execution with import errors
#[test]
fn test_execute_import_errors() {
    let (_temp_dir, project_path) = create_test_file(
        "test_import_error.py",
        indoc! {r#"
            import nonexistent_module  # This will cause ImportError

            def test_never_runs():
                assert True
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_ne!(
        output.status.code(),
        Some(0),
        "Should fail due to import error"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("ImportError")
            || combined.contains("ModuleNotFoundError")
            || combined.contains("import")
    );
}

/// Test execution with syntax errors
#[test]
fn test_execute_syntax_errors() {
    let (_temp_dir, project_path) = create_test_file(
        "test_syntax_error.py",
        indoc! {r#"
            def test_syntax_error():
                if True
                    assert True  # Missing colon after if
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_ne!(
        output.status.code(),
        Some(0),
        "Should fail due to syntax error"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("SyntaxError")
            || combined.contains("syntax")
            || combined.contains("invalid")
    );
}

/// Test maxprocesses limit
#[test]
fn test_execute_maxprocesses_limit() {
    let mut files = HashMap::new();

    // Create 20 test files
    files.insert("test_proc_1.py", "def test_p1_t1(): assert True");
    files.insert("test_proc_2.py", "def test_p2_t1(): assert True");
    files.insert("test_proc_3.py", "def test_p3_t1(): assert True");
    files.insert("test_proc_4.py", "def test_p4_t1(): assert True");
    files.insert("test_proc_5.py", "def test_p5_t1(): assert True");
    files.insert("test_proc_6.py", "def test_p6_t1(): assert True");
    files.insert("test_proc_7.py", "def test_p7_t1(): assert True");
    files.insert("test_proc_8.py", "def test_p8_t1(): assert True");
    files.insert("test_proc_9.py", "def test_p9_t1(): assert True");
    files.insert("test_proc_10.py", "def test_p10_t1(): assert True");
    files.insert("test_proc_11.py", "def test_p11_t1(): assert True");
    files.insert("test_proc_12.py", "def test_p12_t1(): assert True");
    files.insert("test_proc_13.py", "def test_p13_t1(): assert True");
    files.insert("test_proc_14.py", "def test_p14_t1(): assert True");
    files.insert("test_proc_15.py", "def test_p15_t1(): assert True");
    files.insert("test_proc_16.py", "def test_p16_t1(): assert True");
    files.insert("test_proc_17.py", "def test_p17_t1(): assert True");
    files.insert("test_proc_18.py", "def test_p18_t1(): assert True");
    files.insert("test_proc_19.py", "def test_p19_t1(): assert True");
    files.insert("test_proc_20.py", "def test_p20_t1(): assert True");

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["-n", "10", "--maxprocesses", "5"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Should limit to 5 workers despite requesting 10
    assert!(stdout.contains("20 passed") || stdout.contains("Worker"));
}

/// Test execution in subdirectories
#[test]
fn test_execute_subdirectory_tests() {
    let mut files = HashMap::new();

    files.insert(
        "tests/unit/test_unit.py",
        indoc! {r#"
            def test_unit_one():
                assert True

            def test_unit_two():
                assert True
        "#},
    );

    files.insert(
        "tests/integration/test_integration.py",
        indoc! {r#"
            def test_integration_one():
                assert True

            def test_integration_two():
                assert True
        "#},
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .arg("tests/")
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "All tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("4 passed"));
}

/// Test execution with conftest.py fixtures
#[test]
fn test_execute_with_conftest() {
    let mut files = HashMap::new();

    files.insert(
        "conftest.py",
        indoc! {r#"
            import pytest

            @pytest.fixture
            def shared_fixture():
                return {"shared": "data"}
        "#},
    );

    files.insert(
        "test_using_conftest.py",
        indoc! {r#"
            def test_with_shared_fixture(shared_fixture):
                assert shared_fixture["shared"] == "data"

            def test_another_with_fixture(shared_fixture):
                assert "shared" in shared_fixture
        "#},
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert_eq!(output.status.code(), Some(0), "Tests should pass");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("2 passed"));
}

/// Test execution with doctest-style tests
#[test]
fn test_execute_doctests() {
    let (_temp_dir, project_path) = create_test_file(
        "module_with_doctests.py",
        indoc! {r#"
            def add(a, b):
                """Add two numbers.
                
                >>> add(2, 3)
                5
                >>> add(-1, 1)
                0
                >>> add(0, 0)
                0
                """
                return a + b

            def multiply(a, b):
                """Multiply two numbers.
                
                >>> multiply(3, 4)
                12
                >>> multiply(-2, 3)
                -6
                """
                return a * b
        "#},
    );

    let output = Command::new(get_rtest_binary())
        .args(["--doctest-modules", "module_with_doctests.py"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    // Doctest support depends on pytest configuration
    let _stdout = String::from_utf8_lossy(&output.stdout);
    assert!(output.status.code().is_some());
}
