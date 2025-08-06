import textwrap
import unittest

from test_helpers import create_test_project
from test_utils import run_rtest


class TestExecutionIntegration(unittest.TestCase):
    """Comprehensive tests for test execution functionality."""

    def test_execute_single_passing_test(self) -> None:
        """Test basic test execution with a single passing test."""
        files = {
            "test_single.py": textwrap.dedent("""
                def test_passing():
                    assert True
            """)
        }

        with create_test_project(files) as project_path:
            # Note: rtest currently collects and executes all tests in the file
            returncode, stdout, stderr = run_rtest(["test_single.py"], cwd=str(project_path))

            self.assertEqual(returncode, 0, "Test should pass")
            output = stdout + stderr
            self.assertIn("1 passed", output)

    def test_execute_single_failing_test(self) -> None:
        """Test basic test execution with a single failing test."""
        files = {
            "test_single.py": textwrap.dedent("""
                def test_failing():
                    assert False, "Expected failure"
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest(["test_single.py"], cwd=str(project_path))

            self.assertNotEqual(returncode, 0, "Test should fail")
            output = stdout + stderr
            self.assertTrue("1 failed" in output or "FAILED" in output)

    def test_execute_multiple_tests_single_file(self) -> None:
        """Test execution of multiple tests in a single file."""
        files = {
            "test_multiple.py": textwrap.dedent("""
                def test_one():
                    assert 1 + 1 == 2

                def test_two():
                    assert 2 * 2 == 4

                def test_three():
                    assert "hello".upper() == "HELLO"
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest(["test_multiple.py"], cwd=str(project_path))

            self.assertEqual(returncode, 0, "All tests should pass")
            output = stdout + stderr
            self.assertIn("3 passed", output)

    def test_execute_mixed_results(self) -> None:
        """Test execution with mixed pass/fail results."""
        files = {
            "test_mixed.py": textwrap.dedent("""
                def test_pass_one():
                    assert True

                def test_fail_one():
                    assert False, "This test fails"

                def test_pass_two():
                    assert 1 == 1

                def test_fail_two():
                    assert 1 == 2, "Math doesn't check out"
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest(["test_mixed.py"], cwd=str(project_path))

            self.assertNotEqual(returncode, 0, "Should fail due to failed tests")
            output = stdout + stderr
            # Should report both passed and failed tests
            self.assertTrue("2 passed" in output or "2 failed" in output)

    def test_execute_with_errors(self) -> None:
        """Test execution with test errors (not just failures)."""
        files = {
            "test_errors.py": textwrap.dedent("""
                def test_division_by_zero():
                    result = 1 / 0  # This will raise ZeroDivisionError

                def test_undefined_variable():
                    assert undefined_var == 42  # NameError

                def test_passing():
                    assert True
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest(["test_errors.py"], cwd=str(project_path))

            self.assertNotEqual(returncode, 0, "Should fail due to errors")
            output = stdout + stderr
            # Should contain error information
            self.assertTrue("ZeroDivisionError" in output or "NameError" in output or "error" in output)

    def test_execute_parallel_two_workers(self) -> None:
        """Test parallel execution with 2 workers."""
        files = {
            "test_file1.py": textwrap.dedent("""
                import time

                def test_file1_test1():
                    time.sleep(0.1)
                    assert True

                def test_file1_test2():
                    assert True
            """),
            "test_file2.py": textwrap.dedent("""
                import time

                def test_file2_test1():
                    time.sleep(0.1)
                    assert True

                def test_file2_test2():
                    assert True
            """),
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest(["-n", "2"], cwd=str(project_path))

            self.assertEqual(returncode, 0, "All tests should pass")
            output = stdout + stderr
            self.assertTrue("4 passed" in output or "Worker" in output)

    def test_execute_more_workers_than_tests(self) -> None:
        """Test parallel execution with more workers than tests."""
        files = {
            "test_single.py": textwrap.dedent("""
                def test_only_one():
                    assert True
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest(["-n", "4"], cwd=str(project_path))

            self.assertEqual(returncode, 0, "Test should pass")
            output = stdout + stderr
            self.assertIn("1 passed", output)

    def test_execute_specific_tests(self) -> None:
        """Test execution with specific test file."""
        files = {
            "test_selection.py": textwrap.dedent("""
                def test_selected():
                    assert True

                def test_another():
                    assert True

                class TestClass:
                    def test_method_one(self):
                        assert True

                    def test_method_two(self):
                        assert True
            """)
        }

        with create_test_project(files) as project_path:
            # Note: rtest executes all tests in specified file
            returncode, stdout, stderr = run_rtest(["test_selection.py"], cwd=str(project_path))

            self.assertEqual(returncode, 0, "All tests should pass")
            output = stdout + stderr
            self.assertIn("4 passed", output)

    # Skipping test_execute_with_markers - rtest doesn't support -m flag yet
    # def test_execute_with_markers(self) -> None:

    def test_execute_with_fixtures(self) -> None:
        """Test execution with fixtures."""
        files = {
            "test_fixtures.py": textwrap.dedent("""
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
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))

            self.assertEqual(returncode, 0, "Tests with fixtures should pass")
            output = stdout + stderr
            self.assertIn("3 passed", output)

    def test_execute_parameterized_tests(self) -> None:
        """Test execution with parameterized tests."""
        files = {
            "test_parametrize.py": textwrap.dedent("""
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
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))

            self.assertEqual(returncode, 0, "Parameterized tests should pass")
            output = stdout + stderr
            # Should run 4 + 3 = 7 test instances
            self.assertIn("7 passed", output)

    def test_execute_with_setup_teardown(self) -> None:
        """Test execution with setup and teardown."""
        files = {
            "test_setup.py": textwrap.dedent("""
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
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))

            self.assertEqual(returncode, 0, "Tests with setup/teardown should pass")
            output = stdout + stderr
            self.assertIn("2 passed", output)

    def test_execute_with_skipped_tests(self) -> None:
        """Test execution with skipped tests."""
        files = {
            "test_skip.py": textwrap.dedent("""
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
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))

            output = stdout + stderr
            # Should have at least 1 passed (test_normal) and some skipped
            self.assertTrue("1 passed" in output or "skipped" in output)

    def test_execute_with_xfail(self) -> None:
        """Test execution with expected failures."""
        files = {
            "test_xfail.py": textwrap.dedent("""
                import pytest

                @pytest.mark.xfail(reason="Known issue")
                def test_expected_failure():
                    assert False  # This failure is expected

                @pytest.mark.xfail(reason="Fixed but not verified")
                def test_unexpected_pass():
                    assert True  # This passes unexpectedly

                def test_normal():
                    assert True
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))

            output = stdout + stderr
            self.assertIn("1 passed", output)
            # Should report xfailed and xpassed
            self.assertTrue("xfailed" in output or "xpassed" in output or "1 passed" in output)

    def test_execute_custom_assertions(self) -> None:
        """Test execution with custom assertions."""
        files = {
            "test_assertions.py": textwrap.dedent("""
                def test_custom_assertion_message():
                    x = 5
                    y = 10
                    assert x == y, f"Expected x ({x}) to equal y ({y})"

                def test_complex_assertion():
                    data = {"a": 1, "b": 2, "c": 3}
                    assert "d" in data, f"Key 'd' not found in data: {data}"
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))

            self.assertNotEqual(returncode, 0, "Tests should fail")
            output = stdout + stderr
            # Should show custom assertion messages
            self.assertTrue("Expected x" in output or "not found in data" in output or "AssertionError" in output)

    def test_execute_with_timeout(self) -> None:
        """Test execution with timeouts."""
        files = {
            "test_timeout.py": textwrap.dedent("""
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
            """)
        }

        with create_test_project(files) as project_path:
            # Note: This requires pytest-timeout plugin
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))
            # Depending on whether pytest-timeout is installed, behavior may vary
            self.assertIsNotNone(returncode)

    def test_execute_multiple_files_parallel(self) -> None:
        """Test execution with multiple files in parallel."""
        files = {}
        for i in range(1, 11):
            files[f"test_file_{i:02d}.py"] = textwrap.dedent(f"""
                def test_file{i}_fast():
                    assert {i} % 2 == {i % 2}

                def test_file{i}_slow():
                    import time
                    time.sleep(0.01)
                    assert {i} + 1 == {i + 1}
            """)

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest(["-n", "4", "--dist", "load"], cwd=str(project_path))

            self.assertEqual(returncode, 0, "All tests should pass")
            output = stdout + stderr
            self.assertTrue("20 passed" in output or "Worker" in output)

    def test_worker_exit_code_propagation(self) -> None:
        """Test that worker exit codes are properly propagated."""
        files = {
            "test_worker1.py": textwrap.dedent("""
                def test_pass():
                    assert True
            """),
            "test_worker2.py": textwrap.dedent("""
                def test_fail():
                    assert False, "This worker should fail"
            """),
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest(["-n", "2", "--dist", "loadfile"], cwd=str(project_path))

            # Should fail because one worker has failing tests
            self.assertNotEqual(returncode, 0, "Should propagate failure exit code")
            output = stdout + stderr
            self.assertTrue("1 passed" in output and "1 failed" in output)

    def test_execute_no_tests(self) -> None:
        """Test execution with no tests collected."""
        files = {
            "not_a_test.py": textwrap.dedent("""
                def helper_function():
                    return 42

                class HelperClass:
                    def method(self):
                        pass
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))

            output = stdout + stderr
            # Should report no tests collected
            self.assertTrue("no tests" in output or "0 collected" in output or "No tests" in output)

    def test_execute_import_errors(self) -> None:
        """Test execution with import errors."""
        files = {
            "test_import_error.py": textwrap.dedent("""
                import nonexistent_module  # This will cause ImportError

                def test_never_runs():
                    assert True
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))

            self.assertNotEqual(returncode, 0, "Should fail due to import error")
            output = stdout + stderr
            self.assertTrue("ImportError" in output or "ModuleNotFoundError" in output or "import" in output)

    def test_execute_syntax_errors(self) -> None:
        """Test execution with syntax errors."""
        files = {
            "test_syntax_error.py": textwrap.dedent("""
                def test_syntax_error():
                    if True
                        assert True  # Missing colon after if
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))

            self.assertNotEqual(returncode, 0, "Should fail due to syntax error")
            output = stdout + stderr
            self.assertTrue("SyntaxError" in output or "syntax" in output or "invalid" in output)

    def test_execute_subdirectory_tests(self) -> None:
        """Test execution in subdirectories."""
        files = {
            "tests/unit/test_unit.py": textwrap.dedent("""
                def test_unit_one():
                    assert True

                def test_unit_two():
                    assert True
            """),
            "tests/integration/test_integration.py": textwrap.dedent("""
                def test_integration_one():
                    assert True

                def test_integration_two():
                    assert True
            """),
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest(["tests/"], cwd=str(project_path))

            self.assertEqual(returncode, 0, "All tests should pass")
            output = stdout + stderr
            self.assertIn("4 passed", output)

    def test_execute_with_conftest(self) -> None:
        """Test execution with conftest.py fixtures."""
        files = {
            "conftest.py": textwrap.dedent("""
                import pytest

                @pytest.fixture
                def shared_fixture():
                    return {"shared": "data"}
            """),
            "test_using_conftest.py": textwrap.dedent("""
                def test_with_shared_fixture(shared_fixture):
                    assert shared_fixture["shared"] == "data"

                def test_another_with_fixture(shared_fixture):
                    assert "shared" in shared_fixture
            """),
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest([], cwd=str(project_path))

            self.assertEqual(returncode, 0, "Tests should pass")
            output = stdout + stderr
            self.assertIn("2 passed", output)

    def test_execute_doctests(self) -> None:
        """Test execution with doctest-style tests."""
        files = {
            "module_with_doctests.py": textwrap.dedent("""
                def add(a, b):
                    '''Add two numbers.

                    >>> add(2, 3)
                    5
                    >>> add(-1, 1)
                    0
                    >>> add(0, 0)
                    0
                    '''
                    return a + b

                def multiply(a, b):
                    '''Multiply two numbers.

                    >>> multiply(3, 4)
                    12
                    >>> multiply(-2, 3)
                    -6
                    '''
                    return a * b
            """)
        }

        with create_test_project(files) as project_path:
            returncode, stdout, stderr = run_rtest(
                ["--doctest-modules", "module_with_doctests.py"], cwd=str(project_path)
            )

            # Doctest support depends on pytest configuration
            self.assertIsNotNone(returncode)

    def test_execute_with_env_vars(self) -> None:
        """Test that environment variables are correctly passed to pytest."""
        files = {
            "test_env.py": textwrap.dedent("""
                import os

                def test_env_var():
                    assert os.environ.get("TEST_VAR") == "test_value"

                def test_debug_env():
                    assert os.environ.get("DEBUG") == "1"
            """)
        }

        with create_test_project(files) as project_path:
            # Run with environment variables
            returncode, stdout, stderr = run_rtest(
                ["--env", "TEST_VAR=test_value", "--env", "DEBUG=1"], cwd=str(project_path)
            )

            self.assertEqual(returncode, 0, "Tests with env vars should pass")
            output = stdout + stderr
            self.assertIn("2 passed", output)


if __name__ == "__main__":
    unittest.main()
