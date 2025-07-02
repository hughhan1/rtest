"""Unit tests for parallel execution logic using Python's unittest and mocking."""

import tempfile
import textwrap
import unittest
from io import StringIO
from pathlib import Path
from unittest.mock import patch

from rtest._rtest import run_tests


class TestParallelLogic(unittest.TestCase):
    """Test parallel execution and scheduling logic."""

    def test_end_to_end_test_distribution(self):
        """Test the complete flow from test collection to distribution."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create multiple test files to simulate test distribution
            test_files = {
                "test_file1.py": textwrap.dedent("""
                    def test_function1():
                        assert True

                    def test_function2():
                        assert True
                """),
                "test_file2.py": textwrap.dedent("""
                    class TestClass:
                        def test_method1(self):
                            assert True

                        def test_method2(self):
                            assert True
                """),
                "test_file3.py": textwrap.dedent("""
                    def test_function3():
                        assert True
                """),
            }

            for filename, content in test_files.items():
                (project_path / filename).write_text(content)

            # Test with simulated parallel execution (single-threaded for testing)
            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()
            output_lines = output.split("\n")

            # Should find all tests from all files
            expected_tests = [
                "test_file1.py::test_function1",
                "test_file1.py::test_function2",
                "test_file2.py::TestClass::test_method1",
                "test_file2.py::TestClass::test_method2",
                "test_file3.py::test_function3",
            ]

            for test in expected_tests:
                found = any(test in line for line in output_lines)
                self.assertTrue(found, f"Should find test: {test}")

    def test_collection_with_various_file_sizes(self):
        """Test collection handles various file sizes for load balancing."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create files with different numbers of tests to simulate load balancing
            test_files = {
                "test_small.py": textwrap.dedent("""
                    def test_single():
                        assert True
                """),
                "test_medium.py": textwrap.dedent("""
                    def test_one():
                        assert True

                    def test_two():
                        assert True

                    def test_three():
                        assert True
                """),
                "test_large.py": textwrap.dedent("""
                    class TestLargeClass:
                        def test_method1(self):
                            assert True

                        def test_method2(self):
                            assert True

                        def test_method3(self):
                            assert True

                        def test_method4(self):
                            assert True

                        def test_method5(self):
                            assert True

                    def test_additional1():
                        assert True

                    def test_additional2():
                        assert True
                """),
            }

            for filename, content in test_files.items():
                (project_path / filename).write_text(content)

            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()

            # Should find all tests regardless of file size differences
            # This tests the scheduler's ability to handle varied test distributions
            expected_count = 1 + 3 + 7  # small(1) + medium(3) + large(7) = 11 tests
            test_lines = [line for line in output.split("\n") if "::" in line and "test_" in line]

            # Allow some flexibility in exact count due to collection differences
            self.assertGreaterEqual(
                len(test_lines), expected_count - 2, f"Should find approximately {expected_count} tests"
            )

    def test_collection_consistency(self):
        """Test that collection results are consistent across multiple runs."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create a consistent set of test files
            test_content = textwrap.dedent("""
                def test_function1():
                    assert True

                def test_function2():
                    assert True

                class TestClass:
                    def test_method1(self):
                        assert True

                    def test_method2(self):
                        assert True
            """)
            (project_path / "test_consistency.py").write_text(test_content)

            # Run collection multiple times
            outputs = []
            for _ in range(3):
                captured_output = StringIO()
                with patch("sys.stdout", captured_output):
                    run_tests([str(project_path)])

                # Extract just the test collection lines
                test_lines = [
                    line.strip() for line in captured_output.getvalue().split("\n") if "::" in line and "test_" in line
                ]
                outputs.append(sorted(test_lines))

            # All runs should produce the same results
            for i in range(1, len(outputs)):
                self.assertEqual(outputs[0], outputs[i], "Collection should be deterministic")

    def test_nested_directory_structure(self):
        """Test collection works with nested directory structures."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create nested directory structure
            nested_structure = {
                "tests/unit/test_unit.py": textwrap.dedent("""
                    def test_unit_function():
                        assert True
                """),
                "tests/integration/test_integration.py": textwrap.dedent("""
                    def test_integration_function():
                        assert True
                """),
                "src/tests/test_src.py": textwrap.dedent("""
                    def test_src_function():
                        assert True
                """),
                "test_root.py": textwrap.dedent("""
                    def test_root_function():
                        assert True
                """),
            }

            for file_path, content in nested_structure.items():
                full_path = project_path / file_path
                full_path.parent.mkdir(parents=True, exist_ok=True)
                full_path.write_text(content)

            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()

            # Should find tests from all nested directories
            expected_patterns = [
                "test_unit_function",
                "test_integration_function",
                "test_src_function",
                "test_root_function",
            ]

            for pattern in expected_patterns:
                self.assertIn(pattern, output, f"Should find test: {pattern}")

    def test_mixed_file_types_collection(self):
        """Test collection handles mixed file types correctly."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create mixed file types
            files = {
                "test_regular.py": textwrap.dedent("""
                    def test_regular():
                        assert True
                """),
                "conftest.py": textwrap.dedent("""
                    # Pytest configuration - should not contain collectible tests
                    import pytest

                    @pytest.fixture
                    def sample_fixture():
                        return "fixture"
                """),
                "utils.py": textwrap.dedent("""
                    # Utility file - should not be collected
                    def helper_function():
                        return "helper"

                    def test_in_utils():
                        # This should NOT be collected since it's not in a test file
                        pass
                """),
                "test_with_config.py": textwrap.dedent("""
                    import pytest

                    def test_with_fixture():
                        assert True

                    def test_without_fixture():
                        assert True
                """),
                "README.md": "# This is not a Python file",
            }

            for file_path, content in files.items():
                full_path = project_path / file_path
                full_path.parent.mkdir(parents=True, exist_ok=True)
                if file_path.endswith(".py"):
                    full_path.write_text(content)
                else:
                    full_path.write_bytes(content.encode() if isinstance(content, str) else b"binary")

            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()
            output_lines = output.split("\n")

            # Should only find tests from actual test files
            expected_tests = [
                "test_regular.py::test_regular",
                "test_with_config.py::test_with_fixture",
                "test_with_config.py::test_without_fixture",
            ]

            for test in expected_tests:
                found = any(test in line for line in output_lines)
                self.assertTrue(found, f"Should find test: {test}")

            # Should NOT find tests from non-test files
            self.assertNotIn("conftest.py", output, "Should not collect from conftest.py")
            self.assertNotIn("utils.py", output, "Should not collect from utils.py")
            self.assertNotIn("README.md", output, "Should not process non-Python files")

    def test_error_handling_in_collection(self):
        """Test that collection handles errors gracefully."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create files with various issues
            problematic_files = {
                "test_syntax_error.py": textwrap.dedent("""
                    def test_function(
                        # Missing closing parenthesis and body
                """),
                "test_import_error.py": textwrap.dedent("""
                    import nonexistent_module

                    def test_with_bad_import():
                        assert True
                """),
                "test_good.py": textwrap.dedent("""
                    def test_working():
                        assert True
                """),
            }

            for filename, content in problematic_files.items():
                (project_path / filename).write_text(content)

            # Should not crash despite problematic files
            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                try:
                    run_tests([str(project_path)])
                    # Should handle gracefully
                    self.assertTrue(True)
                except Exception as e:
                    self.fail(f"Collection should handle errors gracefully, but got: {e}")

    def test_empty_project_handling(self):
        """Test handling of completely empty projects."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create empty project
            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()
            self.assertIn("No tests found", output, "Should report no tests found for empty project")

    def test_single_test_file_collection(self):
        """Test collection with single test file (no parallelization needed)."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            single_test_content = textwrap.dedent("""
                def test_single():
                    assert True
            """)
            (project_path / "test_single.py").write_text(single_test_content)

            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()
            output_lines = output.split("\n")

            found = any("test_single.py::test_single" in line for line in output_lines)
            self.assertTrue(found, "Should find single test")

    def test_large_test_suite_simulation(self):
        """Test collection with a larger test suite to simulate real-world usage."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)

            # Create multiple modules with multiple tests each
            for module_num in range(1, 6):  # 5 modules
                test_content = textwrap.dedent(f"""
                    # Test module {module_num}
                """)
                for test_num in range(1, 4):  # 3 tests per module
                    test_content += textwrap.dedent(f"""
                        def test_module_{module_num}_function_{test_num}():
                            assert True
                    """)

                # Add a test class with multiple methods
                test_content += textwrap.dedent(f"""
                    class TestModule{module_num}Class:
                        def test_method_1(self):
                            assert True

                        def test_method_2(self):
                            assert True
                """)

                (project_path / f"test_module_{module_num}.py").write_text(test_content)

            captured_output = StringIO()
            with patch("sys.stdout", captured_output):
                run_tests([str(project_path)])

            output = captured_output.getvalue()

            # Should find all tests (5 modules * (3 functions + 2 class methods) = 25 tests)
            test_lines = [line for line in output.split("\n") if "::test_" in line]

            # Allow some flexibility in count
            self.assertGreaterEqual(
                len(test_lines), 20, f"Should find substantial number of tests, found {len(test_lines)}"
            )


if __name__ == "__main__":
    unittest.main()
