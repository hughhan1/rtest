"""Integration tests for test collection functionality."""

import tempfile
import textwrap
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from test_helpers import (
    assert_patterns_not_found,
    assert_tests_found,
    create_test_project,
    run_collection,
)

from rtest._rtest import run_tests


class TestCollectionIntegration(unittest.TestCase):
    """Test that Rust-based collection finds all expected tests."""

    def create_test_project(self) -> Path:
        """Create a temporary test project with sample test files."""
        temp_dir = tempfile.mkdtemp()
        project_path = Path(temp_dir)

        # Create test_sample.py
        sample_content = textwrap.dedent("""
            def test_simple_function():
                assert 1 + 1 == 2

            def test_another_function():
                assert "hello".upper() == "HELLO"

            def helper_method():
                return "not a test"

            class TestExampleClass:
                def test_method_one(self):
                    assert True

                def test_method_two(self):
                    assert len([1, 2, 3]) == 3

            def not_a_test():
                return False
        """)
        (project_path / "test_sample.py").write_text(sample_content)

        # Create test_math.py
        math_content = textwrap.dedent("""
            def test_math_operations():
                assert 2 * 3 == 6

            class TestCalculator:
                def test_addition(self):
                    assert 5 + 3 == 8

                def test_subtraction(self):
                    assert 10 - 4 == 6
        """)
        (project_path / "test_math.py").write_text(math_content)

        # Create utils.py (non-test file)
        utils_content = textwrap.dedent("""
            def utility_function():
                return "utility"

            def test_in_non_test_file():
                # This should not be collected
                pass
        """)
        (project_path / "utils.py").write_text(utils_content)

        return project_path

    def test_collection_finds_all_tests(self) -> None:
        """Test that collection finds all expected test patterns."""
        project_path = self.create_test_project()

        output, output_lines = run_collection(project_path)

        # Check for expected test patterns
        expected_patterns = [
            "test_sample.py::test_simple_function",
            "test_sample.py::test_another_function",
            "test_sample.py::TestExampleClass::test_method_one",
            "test_sample.py::TestExampleClass::test_method_two",
            "test_math.py::test_math_operations",
            "test_math.py::TestCalculator::test_addition",
            "test_math.py::TestCalculator::test_subtraction",
        ]

        assert_tests_found(output_lines, expected_patterns)

        # Should NOT find these patterns
        patterns_to_not_find = ["utils.py", "helper_method", "not_a_test"]
        assert_patterns_not_found(output, patterns_to_not_find)

    def test_collection_with_no_tests(self) -> None:
        """Test collection with no test files."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create a non-test Python file
            regular_content = textwrap.dedent("""
                def regular_function():
                    return "hello"

                class RegularClass:
                    def method(self):
                        pass
            """)
            (project_path / "regular.py").write_text(regular_content)

            captured_stdout = StringIO()
            captured_stderr = StringIO()
            with patch("sys.stdout", captured_stdout), patch("sys.stderr", captured_stderr):
                run_tests([str(project_path)])

    def test_collection_with_syntax_errors(self) -> None:
        """Test collection handles malformed Python files gracefully."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create a Python file with syntax errors
            malformed_content = """def test_function():
    if True  # Missing colon
        pass"""
            (project_path / "test_malformed.py").write_text(malformed_content)

            # Should not crash, but may collect errors
            captured_stdout = StringIO()
            captured_stderr = StringIO()
            with patch("sys.stdout", captured_stdout), patch("sys.stderr", captured_stderr):
                try:
                    run_tests([str(project_path)])
                except Exception as e:
                    self.fail(f"Collection should handle syntax errors gracefully, but got: {e}")

    def test_collection_missing_colon_error(self) -> None:
        """Test collection with missing colon syntax error."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            content = textwrap.dedent("""
                def test_broken():
                    if True
                        assert False  # Missing colon after if
            """)
            (project_path / "test_syntax_error.py").write_text(content)

            # Should not crash on syntax error
            captured_stdout = StringIO()
            captured_stderr = StringIO()
            with patch("sys.stdout", captured_stdout), patch("sys.stderr", captured_stderr):
                try:
                    run_tests([str(project_path)])
                except Exception as e:
                    self.fail(f"Collection should not crash on syntax error, but got: {e}")

    def test_collection_while_stmt_missing_condition(self) -> None:
        """Test collection with while statement missing condition."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            content = """while : ..."""
            (project_path / "test_while_error.py").write_text(content)

            # Should not crash on while statement syntax error
            captured_stdout = StringIO()
            captured_stderr = StringIO()
            with patch("sys.stdout", captured_stdout), patch("sys.stderr", captured_stderr):
                try:
                    run_tests([str(project_path)])
                except Exception as e:
                    self.fail(f"Collection should not crash on while statement syntax error, but got: {e}")

    def test_display_collection_results(self) -> None:
        """Test that collection output display doesn't crash."""
        files = {
            "test_file.py": textwrap.dedent("""
                def test_function():
                    assert True

                class TestClass:
                    def test_method(self):
                        assert True
            """)
        }

        with create_test_project(files) as project_path:
            # This should not crash
            output, output_lines = run_collection(project_path)

            # Should contain the test identifiers
            expected_tests = ["test_file.py::test_function", "test_file.py::TestClass::test_method"]
            assert_tests_found(output_lines, expected_tests)

    def test_collection_with_absolute_path(self) -> None:
        """Test that collection handles absolute paths correctly."""
        files = {
            "test_abs.py": textwrap.dedent("""
                def test_absolute_path():
                    assert True
            """)
        }

        with create_test_project(files) as project_path:
            # Use resolve() to ensure we have an absolute path
            absolute_path = project_path.resolve()

            # Run tests with absolute path
            output, output_lines = run_collection(absolute_path)

            # Should find the test
            self.assertIn("test_abs.py::test_absolute_path", output)
            self.assertIn("collected 1 item", output)


if __name__ == "__main__":
    unittest.main()
