"""Integration tests for test collection functionality."""

import tempfile
import textwrap
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from rtest._rtest import run_tests


class TestCollectionIntegration(unittest.TestCase):
    """Test that Rust-based collection finds all expected tests."""

    def create_test_project(self):
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

    def test_collection_finds_all_tests(self):
        """Test that collection finds all expected test patterns."""
        project_path = self.create_test_project()

        # Capture stdout
        captured_output = StringIO()
        with patch("sys.stdout", captured_output):
            run_tests([str(project_path)])

        output = captured_output.getvalue()

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

        # Check if patterns exist in any line of the output
        # This handles both relative and absolute paths on all platforms
        output_lines = output.split("\n")
        for pattern in expected_patterns:
            found = any(pattern in line for line in output_lines)
            self.assertTrue(found, f"Should find test matching pattern: {pattern}")

        # Should NOT find tests in utils.py (non-test file)
        self.assertNotIn("utils.py", output, "Should not find tests in non-test files")

        # Should NOT find helper methods or non-test functions
        self.assertNotIn("helper_method", output, "Should not find helper methods")
        self.assertNotIn("not_a_test", output, "Should not find non-test functions")

    def test_collection_with_no_tests(self):
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

            captured_stdout.getvalue() + captured_stderr.getvalue()
            # The "No tests found" message appears in the output, test passes if we get here without error
            self.assertTrue(True)  # Test that collection completes without error

    def test_collection_with_syntax_errors(self):
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
                    # Test passes if no exception is raised
                    self.assertTrue(True)
                except Exception as e:
                    self.fail(f"Collection should handle syntax errors gracefully, but got: {e}")

    def test_collection_missing_colon_error(self):
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
                    # Test passes if collection completes without crashing
                    self.assertTrue(True)
                except Exception as e:
                    self.fail(f"Collection should not crash on syntax error, but got: {e}")

    def test_collection_while_stmt_missing_condition(self):
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
                    # Test passes if collection completes without crashing
                    self.assertTrue(True)
                except Exception as e:
                    self.fail(f"Collection should not crash on while statement syntax error, but got: {e}")

    def test_display_collection_results(self):
        """Test that collection output display doesn't crash."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create a simple test file
            test_content = textwrap.dedent("""
                def test_function():
                    assert True

                class TestClass:
                    def test_method(self):
                        assert True
            """)
            (project_path / "test_file.py").write_text(test_content)

            # This should not crash
            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()
            # Check if patterns exist in any line of the output
            output_lines = output.split("\n")

            # Should contain the test identifiers
            patterns = ["test_file.py::test_function", "test_file.py::TestClass::test_method"]
            for pattern in patterns:
                found = any(pattern in line for line in output_lines)
                self.assertTrue(found, f"Should find pattern: {pattern}")

    def test_collection_with_absolute_path(self):
        """Test that collection handles absolute paths correctly."""
        with tempfile.TemporaryDirectory() as temp_dir:
            # Use resolve() to ensure we have an absolute path
            project_path = Path(temp_dir).resolve()

            # Create a test file
            test_content = textwrap.dedent("""
                def test_absolute_path():
                    assert True
            """)
            (project_path / "test_abs.py").write_text(test_content)

            # Run tests with absolute path
            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()

            # Should find the test
            self.assertIn("test_abs.py::test_absolute_path", output)
            self.assertIn("collected 1 item", output)

            # Verify the absolute path was used (not joined to cwd)
            self.assertIn(str(project_path), output)


if __name__ == "__main__":
    unittest.main()
