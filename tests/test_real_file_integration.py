"""Integration test using real Python test files to ensure collection works on actual pytest files."""

import tempfile
import textwrap
import unittest
from pathlib import Path

from test_helpers import (
    assert_patterns_not_found,
    assert_tests_found,
    create_test_project,
    run_collection,
)
from test_utils import run_rtest


class TestRealFileIntegration(unittest.TestCase):
    """Test collection on realistic pytest files."""

    def test_collection_on_comprehensive_pytest_file(self) -> None:
        """Test collection on a realistic pytest file with comprehensive test patterns."""
        # Create a realistic pytest file with comprehensive test patterns
        files = {
            "test_comprehensive.py": textwrap.dedent("""
                def test_simple_assertion():
                    assert 1 + 1 == 2

                def test_string_operations():
                    text = "hello world"
                    assert text.upper() == "HELLO WORLD"

                def test_list_operations():
                    numbers = [1, 2, 3, 4, 5]
                    assert len(numbers) == 5

                def helper_function():
                    return "helper"

                class TestMathOperations:
                    def test_addition(self):
                        assert 10 + 5 == 15

                    def test_subtraction(self):
                        assert 10 - 5 == 5

                    def setup_method(self):
                        pass

                class TestStringMethods:
                    def test_capitalize(self):
                        assert "hello".capitalize() == "Hello"

                    def test_split(self):
                        result = "a,b,c".split(",")
                        assert result == ["a", "b", "c"]

                class UtilityClass:
                    def test_method_should_be_ignored(self):
                        pass

                    def utility_method(self):
                        return True

                def process_data(data):
                    return sum(data)
            """)
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Expected test functions and class methods
            expected_tests = [
                "test_comprehensive.py::test_simple_assertion",
                "test_comprehensive.py::test_string_operations",
                "test_comprehensive.py::test_list_operations",
                "test_comprehensive.py::TestMathOperations::test_addition",
                "test_comprehensive.py::TestMathOperations::test_subtraction",
                "test_comprehensive.py::TestStringMethods::test_capitalize",
                "test_comprehensive.py::TestStringMethods::test_split",
            ]

            assert_tests_found(result.output_lines, expected_tests)

            # Verify we don't collect non-test items
            should_not_collect = [
                "helper_function",
                "setup_method",
                "teardown_method",
                "UtilityClass",
                "test_method_should_be_ignored",
                "utility_method",
                "process_data",
            ]

            assert_patterns_not_found(result.output, should_not_collect)

    def test_collection_with_various_test_patterns(self) -> None:
        """Test collection recognizes various pytest naming patterns."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Test file with various naming patterns
            patterns_content = textwrap.dedent("""
                # Function tests with different prefixes
                def test_basic():
                    pass

                def test_with_underscores():
                    pass

                def test_123_with_numbers():
                    pass

                # Class-based tests
                class TestBasic:
                    def test_method(self):
                        pass

                class TestWithLongName:
                    def test_method_with_long_name(self):
                        pass

                    def test_another_method(self):
                        pass

                class Test123WithNumbers:
                    def test_numeric_method(self):
                        pass

                # Should NOT be collected
                def helper():
                    pass

                def _private_function():
                    pass

                def function_without_test_prefix():
                    pass

                class RegularClass:
                    def method(self):
                        pass

                class TestClass:
                    def helper_method(self):
                        pass

                    def _private_method(self):
                        pass
            """)

            (project_path / "test_patterns.py").write_text(patterns_content)

            return_code, output, stderr = run_rtest(["--collect-only"], cwd=str(project_path))
            output_lines = output.split("\n")

            # Should collect all properly named test functions
            expected_tests = [
                "test_patterns.py::test_basic",
                "test_patterns.py::test_with_underscores",
                "test_patterns.py::test_123_with_numbers",
                "test_patterns.py::TestBasic::test_method",
                "test_patterns.py::TestWithLongName::test_method_with_long_name",
                "test_patterns.py::TestWithLongName::test_another_method",
                "test_patterns.py::Test123WithNumbers::test_numeric_method",
            ]

            for test in expected_tests:
                found = any(test in line for line in output_lines)
                self.assertTrue(found, f"Should collect test: {test}")

            # Should NOT collect these
            should_not_collect = [
                "helper",
                "_private_function",
                "function_without_test_prefix",
                "RegularClass",
                "helper_method",
                "_private_method",
            ]

            for item in should_not_collect:
                self.assertNotIn(item, output, f"Should not collect: {item}")

    def test_collection_on_multiple_test_files(self) -> None:
        """Test collection across multiple test files."""
        # Create multiple test files
        files = {
            "test_file1.py": textwrap.dedent("""
                def test_file1_function():
                    pass

                class TestFile1Class:
                    def test_method(self):
                        pass
            """),
            "test_file2.py": textwrap.dedent("""
                def test_file2_function1():
                    pass

                def test_file2_function2():
                    pass
            """),
            "subdir/test_nested.py": textwrap.dedent("""
                def test_nested_function():
                    pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Should find tests from all files
            expected_tests = [
                "test_file1.py::test_file1_function",
                "test_file1.py::TestFile1Class::test_method",
                "test_file2.py::test_file2_function1",
                "test_file2.py::test_file2_function2",
                "test_nested.py::test_nested_function",
            ]

            assert_tests_found(result.output_lines, expected_tests)


if __name__ == "__main__":
    unittest.main()
