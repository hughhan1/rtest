"""Integration test using real Python test files to ensure collection works on actual pytest files."""

import tempfile
import textwrap
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from rtest._rtest import run_tests


class TestRealFileIntegration(unittest.TestCase):
    """Test collection on realistic pytest files."""

    def test_collection_on_comprehensive_pytest_file(self):
        """Test collection on a realistic pytest file with comprehensive test patterns."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create a realistic pytest file with comprehensive test patterns
            real_pytest_content = textwrap.dedent("""
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

            test_file_path = project_path / "test_comprehensive.py"
            test_file_path.write_text(real_pytest_content)

            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()
            output_lines = output.split("\n")

            # Expected test functions
            expected_functions = [
                "test_comprehensive.py::test_simple_assertion",
                "test_comprehensive.py::test_string_operations",
                "test_comprehensive.py::test_list_operations",
            ]

            # Expected test class methods
            expected_class_methods = [
                "test_comprehensive.py::TestMathOperations::test_addition",
                "test_comprehensive.py::TestMathOperations::test_subtraction",
                "test_comprehensive.py::TestStringMethods::test_capitalize",
                "test_comprehensive.py::TestStringMethods::test_split",
            ]

            # Verify all expected tests are found
            for expected in expected_functions:
                found = any(expected in line for line in output_lines)
                self.assertTrue(found, f"Should find test function: {expected}")

            for expected in expected_class_methods:
                found = any(expected in line for line in output_lines)
                self.assertTrue(found, f"Should find test method: {expected}")

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

            for item in should_not_collect:
                self.assertNotIn(item, output, f"Should not collect non-test item: {item}")

    def test_collection_with_various_test_patterns(self):
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

            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()
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

    def test_collection_on_multiple_test_files(self):
        """Test collection across multiple test files."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create multiple test files
            files_and_content = {
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

            for file_path, content in files_and_content.items():
                full_path = project_path / file_path
                full_path.parent.mkdir(parents=True, exist_ok=True)
                full_path.write_text(content)

            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()
            output_lines = output.split("\n")

            # Should find tests from all files
            expected_tests = [
                "test_file1.py::test_file1_function",
                "test_file1.py::TestFile1Class::test_method",
                "test_file2.py::test_file2_function1",
                "test_file2.py::test_file2_function2",
            ]

            for test in expected_tests:
                found = any(test in line for line in output_lines)
                self.assertTrue(found, f"Should find test: {test}")

            # For nested paths, check both forward and backslash versions
            nested_pattern = "test_nested.py::test_nested_function"
            found = any(nested_pattern in line for line in output_lines)
            self.assertTrue(found, f"Should find test: {nested_pattern}")


if __name__ == "__main__":
    unittest.main()
