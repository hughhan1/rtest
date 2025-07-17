"""Integration tests for test collection functionality."""

import os
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

    def test_collection_finds_all_tests(self) -> None:
        """Test that collection finds all expected test patterns."""
        files = {
            "test_sample.py": textwrap.dedent("""
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
            """),
            "test_math.py": textwrap.dedent("""
                def test_math_operations():
                    assert 2 * 3 == 6

                class TestCalculator:
                    def test_addition(self):
                        assert 5 + 3 == 8

                    def test_subtraction(self):
                        assert 10 - 4 == 6
            """),
            "utils.py": textwrap.dedent("""
                def utility_function():
                    return "utility"

                def test_in_non_test_file():
                    # This should not be collected
                    pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

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

            assert_tests_found(result.output_lines, expected_patterns)

            # Should NOT find these patterns
            patterns_to_not_find = ["utils.py", "helper_method", "not_a_test"]
            assert_patterns_not_found(result.output, patterns_to_not_find)

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
            result = run_collection(project_path)

            # Should contain the test identifiers
            expected_tests = ["test_file.py::test_function", "test_file.py::TestClass::test_method"]
            assert_tests_found(result.output_lines, expected_tests)

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
            result = run_collection(absolute_path)

            # Should find the test
            self.assertIn("test_abs.py::test_absolute_path", result.output)
            self.assertIn("collected 1 item", result.output)

    def test_cross_module_inheritance(self) -> None:
        """Test that collection handles test class inheritance across modules."""
        files = {
            "test_base.py": textwrap.dedent("""
                class TestBase:
                    def test_base_method(self):
                        assert True

                    def test_another_base_method(self):
                        assert True
            """),
            "test_derived.py": textwrap.dedent("""
                from test_base import TestBase

                class TestDerived(TestBase):
                    def test_derived_method(self):
                        assert True
            """),
            "test_multi_level.py": textwrap.dedent("""
                from test_derived import TestDerived

                class TestMultiLevel(TestDerived):
                    def test_multi_level_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Should find all tests including inherited ones
            expected_patterns = [
                # Base class tests
                "test_base.py::TestBase::test_base_method",
                "test_base.py::TestBase::test_another_base_method",
                # Derived class should have both inherited and its own tests
                "test_derived.py::TestDerived::test_base_method",
                "test_derived.py::TestDerived::test_another_base_method",
                "test_derived.py::TestDerived::test_derived_method",
                # Multi-level class should have all inherited tests from full chain
                "test_multi_level.py::TestMultiLevel::test_base_method",
                "test_multi_level.py::TestMultiLevel::test_another_base_method",
                "test_multi_level.py::TestMultiLevel::test_derived_method",
                "test_multi_level.py::TestMultiLevel::test_multi_level_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)

            # Should collect 9 tests total (2 + 3 + 4)
            self.assertIn("collected 9 items", result.output)

    def test_multiple_inheritance(self) -> None:
        """Test that collection handles multiple inheritance correctly."""
        files = {
            "test_mixins.py": textwrap.dedent("""
                class TestMixinA:
                    def test_mixin_a_method(self):
                        assert True

                class TestMixinB:
                    def test_mixin_b_method(self):
                        assert True

                    def test_mixin_b_another(self):
                        assert True
            """),
            "test_multiple.py": textwrap.dedent("""
                from test_mixins import TestMixinA, TestMixinB

                class TestMultipleInheritance(TestMixinA, TestMixinB):
                    def test_own_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                # TestMixinA tests
                "test_mixins.py::TestMixinA::test_mixin_a_method",
                # TestMixinB tests
                "test_mixins.py::TestMixinB::test_mixin_b_method",
                "test_mixins.py::TestMixinB::test_mixin_b_another",
                # TestMultipleInheritance should have all inherited + own
                "test_multiple.py::TestMultipleInheritance::test_mixin_a_method",
                "test_multiple.py::TestMultipleInheritance::test_mixin_b_method",
                "test_multiple.py::TestMultipleInheritance::test_mixin_b_another",
                "test_multiple.py::TestMultipleInheritance::test_own_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 7 items", result.output)

    def test_diamond_inheritance(self) -> None:
        """Test diamond inheritance pattern."""
        files = {
            "test_diamond_base.py": textwrap.dedent("""
                class TestDiamondBase:
                    def test_base_method(self):
                        assert True
            """),
            "test_diamond_middle.py": textwrap.dedent("""
                from test_diamond_base import TestDiamondBase

                class TestDiamondLeft(TestDiamondBase):
                    def test_left_method(self):
                        assert True

                class TestDiamondRight(TestDiamondBase):
                    def test_right_method(self):
                        assert True
            """),
            "test_diamond_bottom.py": textwrap.dedent("""
                from test_diamond_middle import TestDiamondLeft, TestDiamondRight

                class TestDiamondBottom(TestDiamondLeft, TestDiamondRight):
                    def test_bottom_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                # Base class
                "test_diamond_base.py::TestDiamondBase::test_base_method",
                # Left branch
                "test_diamond_middle.py::TestDiamondLeft::test_base_method",
                "test_diamond_middle.py::TestDiamondLeft::test_left_method",
                # Right branch
                "test_diamond_middle.py::TestDiamondRight::test_base_method",
                "test_diamond_middle.py::TestDiamondRight::test_right_method",
                # Bottom (diamond point) - should have base, left, right, and own
                "test_diamond_bottom.py::TestDiamondBottom::test_base_method",
                "test_diamond_bottom.py::TestDiamondBottom::test_left_method",
                "test_diamond_bottom.py::TestDiamondBottom::test_right_method",
                "test_diamond_bottom.py::TestDiamondBottom::test_bottom_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)

    def test_inheritance_with_method_override(self) -> None:
        """Test that overridden methods are handled correctly."""
        files = {
            "test_override_base.py": textwrap.dedent("""
                class TestOverrideBase:
                    def test_method_to_override(self):
                        assert False  # Base implementation

                    def test_not_overridden(self):
                        assert True
            """),
            "test_override_child.py": textwrap.dedent("""
                from test_override_base import TestOverrideBase

                class TestOverrideChild(TestOverrideBase):
                    def test_method_to_override(self):
                        assert True  # Overridden implementation

                    def test_child_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                # Base class
                "test_override_base.py::TestOverrideBase::test_method_to_override",
                "test_override_base.py::TestOverrideBase::test_not_overridden",
                # Child class - overridden method should appear, not inherited one
                "test_override_child.py::TestOverrideChild::test_method_to_override",
                "test_override_child.py::TestOverrideChild::test_not_overridden",
                "test_override_child.py::TestOverrideChild::test_child_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)

    def test_deep_inheritance_chain(self) -> None:
        """Test very deep inheritance chain (5 levels)."""
        files = {
            "test_level1.py": textwrap.dedent("""
                class TestLevel1:
                    def test_level1_method(self):
                        assert True
            """),
            "test_level2.py": textwrap.dedent("""
                from test_level1 import TestLevel1

                class TestLevel2(TestLevel1):
                    def test_level2_method(self):
                        assert True
            """),
            "test_level3.py": textwrap.dedent("""
                from test_level2 import TestLevel2

                class TestLevel3(TestLevel2):
                    def test_level3_method(self):
                        assert True
            """),
            "test_level4.py": textwrap.dedent("""
                from test_level3 import TestLevel3

                class TestLevel4(TestLevel3):
                    def test_level4_method(self):
                        assert True
            """),
            "test_level5.py": textwrap.dedent("""
                from test_level4 import TestLevel4

                class TestLevel5(TestLevel4):
                    def test_level5_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Level 5 should have all 5 methods
            expected_patterns = [
                "test_level5.py::TestLevel5::test_level1_method",
                "test_level5.py::TestLevel5::test_level2_method",
                "test_level5.py::TestLevel5::test_level3_method",
                "test_level5.py::TestLevel5::test_level4_method",
                "test_level5.py::TestLevel5::test_level5_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)

            # Total: 1 + 2 + 3 + 4 + 5 = 15 tests
            self.assertIn("collected 15 items", result.output)

    def test_mixed_local_and_imported_inheritance(self) -> None:
        """Test mixing local and imported base classes."""
        files = {
            "test_imported_base.py": textwrap.dedent("""
                class TestImportedBase:
                    def test_imported_method(self):
                        assert True
            """),
            "test_mixed.py": textwrap.dedent("""
                from test_imported_base import TestImportedBase

                class TestLocalBase:
                    def test_local_method(self):
                        assert True

                class TestMixed(TestLocalBase, TestImportedBase):
                    def test_mixed_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                # Imported base
                "test_imported_base.py::TestImportedBase::test_imported_method",
                # Local base
                "test_mixed.py::TestLocalBase::test_local_method",
                # Mixed class should have both local and imported methods
                "test_mixed.py::TestMixed::test_local_method",
                "test_mixed.py::TestMixed::test_imported_method",
                "test_mixed.py::TestMixed::test_mixed_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)

    def test_inheritance_from_non_test_class(self) -> None:
        """Test that inheritance from non-test classes is handled correctly."""
        files = {
            "test_helpers.py": textwrap.dedent("""
                class BaseHelper:  # Not a test class
                    def helper_method(self):
                        return "helper"

                    def test_should_not_be_collected(self):
                        # This should not be collected as BaseHelper is not a test class
                        assert True

                class TestWithHelper(BaseHelper):
                    def test_actual_test(self):
                        assert self.helper_method() == "helper"
            """)
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Should only find the test in TestWithHelper, not in BaseHelper
            expected_patterns = ["test_helpers.py::TestWithHelper::test_actual_test"]

            assert_tests_found(result.output_lines, expected_patterns)

            # Should NOT find these
            patterns_to_not_find = ["BaseHelper", "test_should_not_be_collected"]
            assert_patterns_not_found(result.output, patterns_to_not_find)

    def test_complex_import_patterns(self) -> None:
        """Test various import patterns for inheritance."""
        files = {
            "test_base_module.py": textwrap.dedent("""
                class TestBase1:
                    def test_base1_method(self):
                        assert True

                class TestBase2:
                    def test_base2_method(self):
                        assert True
            """),
            "test_import_patterns.py": textwrap.dedent("""
                # Test different import styles
                from test_base_module import TestBase1
                import test_base_module

                class TestImportStyle1(TestBase1):
                    def test_style1_method(self):
                        assert True

                class TestImportStyle2(test_base_module.TestBase2):
                    def test_style2_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                # Base classes
                "test_base_module.py::TestBase1::test_base1_method",
                "test_base_module.py::TestBase2::test_base2_method",
                # Import style 1 (from X import Y)
                "test_import_patterns.py::TestImportStyle1::test_base1_method",
                "test_import_patterns.py::TestImportStyle1::test_style1_method",
                # Import style 2 (import X, then X.Y)
                "test_import_patterns.py::TestImportStyle2::test_base2_method",
                "test_import_patterns.py::TestImportStyle2::test_style2_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)

    def test_inheritance_with_init_in_chain(self) -> None:
        """Test that __init__ in any parent class skips the entire chain."""
        files = {
            "test_init_chain.py": textwrap.dedent("""
                class TestGrandparent:
                    def test_grandparent_method(self):
                        assert True

                class TestParentWithInit(TestGrandparent):
                    def __init__(self):
                        pass

                    def test_parent_method(self):
                        assert True

                class TestChild(TestParentWithInit):
                    def test_child_method(self):
                        assert True

                class TestGrandchild(TestChild):
                    def test_grandchild_method(self):
                        assert True
            """)
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Only TestGrandparent should be collected
            # All others should be skipped due to __init__ in the inheritance chain
            expected_patterns = ["test_init_chain.py::TestGrandparent::test_grandparent_method"]

            assert_tests_found(result.output_lines, expected_patterns)

            # Should NOT find these test identifiers
            patterns_to_not_find = [
                "test_init_chain.py::TestParentWithInit::test_parent_method",
                "test_init_chain.py::TestChild::test_child_method",
                "test_init_chain.py::TestGrandchild::test_grandchild_method",
            ]
            assert_patterns_not_found(result.output, patterns_to_not_find)

            self.assertIn("collected 1 item", result.output)

    def test_collection_warnings_for_init_classes(self) -> None:
        """Test that warnings are emitted for classes with __init__ constructors."""
        files = {
            "test_warnings.py": textwrap.dedent("""
                class TestWithInit:
                    def __init__(self):
                        pass

                    def test_should_be_skipped(self):
                        assert True

                class TestWithoutInit:
                    def test_should_be_collected(self):
                        assert True

                class TestBaseWithInit:
                    def __init__(self):
                        pass

                    def test_base_method(self):
                        assert True

                class TestDerivedFromInit(TestBaseWithInit):
                    def test_derived_method(self):
                        assert True
            """)
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            output = result.stdout + result.stderr

            # Should collect only TestWithoutInit
            self.assertIn("test_warnings.py::TestWithoutInit::test_should_be_collected", result.output)
            self.assertIn("collected 1 item", result.output)

            # Should emit warnings for both classes with __init__
            self.assertIn("RtestCollectionWarning: cannot collect test class 'TestWithInit'", result.output)
            self.assertIn("RtestCollectionWarning: cannot collect test class 'TestBaseWithInit'", result.output)
            self.assertIn("RtestCollectionWarning: cannot collect test class 'TestDerivedFromInit'", result.output)

            # Should show file and line numbers for all three classes
            self.assertIn("test_warnings.py:", result.output)  # All should have file paths
            warning_lines = [line for line in output.split("\n") if "RtestCollectionWarning" in line]
            self.assertEqual(len(warning_lines), 3)  # Should have exactly 3 warnings

    def test_circular_inheritance(self) -> None:
        """Test that collection detects and reports circular inheritance as an error."""
        files = {
            "test_circular.py": textwrap.dedent("""
                # Forward reference to TestB
                class TestA(TestB):  # type: ignore
                    def test_a_method(self):
                        assert True

                class TestB(TestA):
                    def test_b_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            # This should fail due to circular inheritance
            result = run_collection(project_path)

            # Should exit with error code due to circular inheritance
            self.assertNotEqual(result.returncode, 0)
            # Error message should mention circular inheritance (could be in stdout or stderr)
            error_text = result.stdout + result.stderr
            self.assertIn("Circular inheritance detected", error_text)

    def test_circular_inheritance_cross_module(self) -> None:
        """Test that collection detects circular inheritance across modules."""
        files = {
            "test_circular_a.py": textwrap.dedent("""
                from test_circular_b import TestB

                class TestA(TestB):
                    def test_a_method(self):
                        assert True
            """),
            "test_circular_b.py": textwrap.dedent("""
                from test_circular_a import TestA

                class TestB(TestA):
                    def test_b_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            # This should fail due to circular inheritance
            result = run_collection(project_path)

            # Should exit with error code due to circular inheritance
            self.assertNotEqual(result.returncode, 0)
            # Error message should mention circular inheritance (could be in stdout or stderr)
            error_text = result.stdout + result.stderr
            self.assertIn("Circular inheritance detected", error_text)

    def test_circular_inheritance_with_init(self) -> None:
        """Test circular inheritance where one class has __init__."""
        files = {
            "test_circular_init.py": textwrap.dedent("""
                class TestA(TestB):  # type: ignore
                    def __init__(self):
                        pass

                    def test_a_method(self):
                        assert True

                class TestB(TestA):
                    def test_b_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Both classes should be skipped due to circular inheritance with __init__
            patterns_to_not_find = [
                "test_circular_init.py::TestA::test_a_method",
                "test_circular_init.py::TestB::test_b_method",
            ]
            assert_patterns_not_found(result.output, patterns_to_not_find)
            # Check that no tests were collected
            self.assertIn("No tests collected", result.output)

    def test_unresolvable_base_class_error(self) -> None:
        """Test that unresolvable base classes result in collection errors."""
        files = {
            "test_unresolvable.py": textwrap.dedent("""
                # Test with an imported base class that doesn't exist
                from nonexistent_module import NonExistentClass

                class TestWithUnresolvableImportedBase(NonExistentClass):
                    def test_method(self):
                        assert True
            """)
        }

        with create_test_project(files) as project_path:
            # This should fail due to unresolvable imported base class
            result = run_collection(project_path)

            # Should exit with error code due to import error
            self.assertNotEqual(result.returncode, 0)
            # Error message should mention the import error (could be in stdout or stderr)
            error_text = result.stdout + result.stderr
            self.assertIn("Could not find module: nonexistent_module", error_text)

    def test_unittest_testcase_inheritance_supported(self) -> None:
        """Test that inheritance from unittest.TestCase is properly supported."""
        files = {
            "test_unittest_inheritance.py": textwrap.dedent("""
                import unittest

                class TestWithUnittestBase(unittest.TestCase):
                    def test_method(self):
                        self.assertTrue(True)
            """)
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(
                result.returncode,
                0,
                f"unittest.TestCase inheritance should be supported. Error: {result.stdout + result.stderr}",
            )

            output = result.stdout + result.stderr
            self.assertIn("test_unittest_inheritance.py::TestWithUnittestBase::test_method", output)
            self.assertIn("collected 1 item", output)

    def test_parameterized_tests_via_inheritance(self) -> None:
        """
        This addresses the specific case where developers use class inheritance to implement
        parameterized test cases. Both parent and child classes should have their test
        methods collected, allowing the same test logic to run with different parameters.

        Example scenario: TestStringConcat and TestStringJoin both run test_concatenation
        but with different operation implementations and expected results.
        """
        files = {
            "test_string_operations.py": textwrap.dedent("""
                class TestStringConcat:
                    def test_concatenation(self):
                        result = self.operation(self.input_a, self.input_b)
                        assert result == self.expected

                    operation = lambda self, a, b: a + b
                    input_a = "hello"
                    input_b = "world"
                    expected = "helloworld"

                class TestStringJoin(TestStringConcat):
                    operation = lambda self, a, b: " ".join([a, b])
                    expected = "hello world"
            """)
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Should succeed and collect all three parameterized variations
            self.assertEqual(
                result.returncode,
                0,
                f"Parameterized test inheritance should work. Error: {result.stdout + result.stderr}",
            )

            output = result.stdout + result.stderr

            # Should collect both test methods (one from each class) - fixes the original issue
            # where only the parent class test was collected
            self.assertIn("test_string_operations.py::TestStringConcat::test_concatenation", output)
            self.assertIn("test_string_operations.py::TestStringJoin::test_concatenation", output)
            self.assertIn("collected 2 items", output)

    def test_multi_level_parameterized_inheritance(self) -> None:
        """
        This extends the parameterized test case scenario to test inheritance chains where
        each level in the hierarchy represents a different parameterization of the same
        test logic. All classes in the chain should have their test methods collected.

        Example: TestBase (identity), TestSquare (x²), TestCube (x³) all run test_operation
        but with progressively different mathematical operations.
        """
        files = {
            "test_multi_param.py": textwrap.dedent("""
                class TestBase:
                    def test_operation(self):
                        result = self.operation(self.input_value)
                        assert result == self.expected

                    operation = lambda self, x: x
                    input_value = 5
                    expected = 5

                class TestSquare(TestBase):
                    operation = lambda self, x: x * x
                    expected = 25

                class TestCube(TestSquare):
                    operation = lambda self, x: x * x * x
                    expected = 125
            """)
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Should succeed and collect all three parameterized variations
            self.assertEqual(
                result.returncode,
                0,
                f"Multi-level parameterized inheritance should work. Error: {result.stdout + result.stderr}",
            )

            output = result.stdout + result.stderr

            # Should collect all three test methods (one from each class in the inheritance chain)
            # This ensures no parameterized test variations are missed in deep inheritance hierarchies
            self.assertIn("test_multi_param.py::TestBase::test_operation", output)
            self.assertIn("test_multi_param.py::TestSquare::test_operation", output)
            self.assertIn("test_multi_param.py::TestCube::test_operation", output)
            self.assertIn("collected 3 items", output)

    def test_sys_path_import_resolution(self) -> None:
        """Test that imports from directories in sys.path are resolved correctly."""
        with tempfile.TemporaryDirectory() as external_dir:
            # Create a module in the external directory
            external_module_content = textwrap.dedent("""
                class TestExternalBase:
                    def test_external_method(self):
                        assert True

                    def test_another_external_method(self):
                        assert True
            """)
            (Path(external_dir) / "external_test_module.py").write_text(external_module_content)

            # Create test file that imports from external module
            files = {
                "test_sys_path_import.py": textwrap.dedent("""
                    from external_test_module import TestExternalBase

                    class TestWithSysPathImport(TestExternalBase):
                        def test_local_method(self):
                            assert True
                """)
            }

            with create_test_project(files) as project_path:
                # Set PYTHONPATH to include the external directory
                env = os.environ.copy()
                env["PYTHONPATH"] = external_dir

                result = run_collection(project_path, env=env)

                # Should succeed and find inherited methods from sys.path
                self.assertEqual(
                    result.returncode,
                    0,
                    f"sys.path import resolution should work. Error: {result.output}",
                )

                # Should collect both inherited methods and local method
                self.assertIn("test_sys_path_import.py::TestWithSysPathImport::test_external_method", result.output)
                self.assertIn(
                    "test_sys_path_import.py::TestWithSysPathImport::test_another_external_method", result.output
                )
                self.assertIn("test_sys_path_import.py::TestWithSysPathImport::test_local_method", result.output)
                self.assertIn("collected 3 items", result.output)

    def test_pythonpath_multiple_directories(self) -> None:
        """Test that multiple directories in PYTHONPATH are searched in order."""
        # Create two external directories
        with tempfile.TemporaryDirectory() as external_dir1, tempfile.TemporaryDirectory() as external_dir2:
            # Create same module name in both directories with different content
            module1_content = textwrap.dedent("""
                class TestPriority:
                    def test_from_first_dir(self):
                        assert True
            """)
            (Path(external_dir1) / "priority_module.py").write_text(module1_content)

            module2_content = textwrap.dedent("""
                class TestPriority:
                    def test_from_second_dir(self):
                        assert True
            """)
            (Path(external_dir2) / "priority_module.py").write_text(module2_content)

            # Create test file
            files = {
                "test_priority.py": textwrap.dedent("""
                    from priority_module import TestPriority

                    class TestPriorityChild(TestPriority):
                        def test_child_method(self):
                            assert True
                """)
            }

            with create_test_project(files) as project_path:
                # Set PYTHONPATH with dir1 first (should take precedence)
                env = os.environ.copy()
                env["PYTHONPATH"] = f"{external_dir1}{os.pathsep}{external_dir2}"

                result = run_collection(project_path, env=env)

                self.assertEqual(result.returncode, 0)

                # Should use the module from the first directory
                self.assertIn("test_priority.py::TestPriorityChild::test_from_first_dir", result.output)
                self.assertNotIn("test_from_second_dir", result.output)
                self.assertIn("test_priority.py::TestPriorityChild::test_child_method", result.output)

    def test_unittest_inheritance(self) -> None:
        """Test that importing and inheriting from unittest.TestCase works."""
        files = {
            "test_unittest_inheritance.py": textwrap.dedent("""
                import unittest

                class TestWithStdlib(unittest.TestCase):
                    def test_using_unittest(self):
                        self.assertTrue(True)
            """)
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Should successfully collect the test
            self.assertEqual(result.returncode, 0)

            self.assertIn("test_unittest_inheritance.py::TestWithStdlib::test_using_unittest", result.output)
            self.assertIn("collected 1 item", result.output)

    def test_site_packages_inheritance(self) -> None:
        """Test that imports from site-packages are resolved correctly."""

        # Create test file that imports from site-packages
        files = {
            "test_site_inheritance.py": textwrap.dedent("""
                from site_test_module import TestSitePackageBase

                class TestWithSitePackageImport(TestSitePackageBase):
                    def test_local_method(self):
                        assert True
            """)
        }

        with tempfile.TemporaryDirectory() as site_packages_dir:
            # Create a module in the site-packages directory
            site_module_content = textwrap.dedent("""
                class TestSitePackageBase:
                    def test_site_package_method(self):
                        assert True

                    def test_another_site_method(self):
                        assert True
            """)
            (Path(site_packages_dir) / "site_test_module.py").write_text(site_module_content)

            with create_test_project(files) as project_path:
                # Set PYTHONPATH to include the site-packages directory
                env = os.environ.copy()
                env["PYTHONPATH"] = site_packages_dir

                result = run_collection(project_path, env=env)

                # Should succeed and find inherited methods from site-packages
                self.assertEqual(
                    result.returncode,
                    0,
                    f"site-packages import resolution should work. Error: {result.output}",
                )

                # Should collect both inherited methods and local method
                self.assertIn(
                    "test_site_inheritance.py::TestWithSitePackageImport::test_site_package_method", result.output
                )
                self.assertIn(
                    "test_site_inheritance.py::TestWithSitePackageImport::test_another_site_method", result.output
                )
                self.assertIn("test_site_inheritance.py::TestWithSitePackageImport::test_local_method", result.output)
                self.assertIn("collected 3 items", result.output)

    def test_nested_package_import_from_sys_path(self) -> None:
        """Test importing from nested packages in sys.path."""
        with tempfile.TemporaryDirectory() as external_dir:
            # Create nested package structure
            package_dir = Path(external_dir) / "mypackage"
            subpackage_dir = package_dir / "subpackage"
            subpackage_dir.mkdir(parents=True)

            # Create __init__ files
            (package_dir / "__init__.py").write_text("")
            (subpackage_dir / "__init__.py").write_text("")

            # Create test module in subpackage
            nested_module_content = textwrap.dedent("""
                class TestNestedBase:
                    def test_nested_method(self):
                        assert True
            """)
            (subpackage_dir / "nested_module.py").write_text(nested_module_content)

            # Create test file
            files = {
                "test_nested_import.py": textwrap.dedent("""
                    from mypackage.subpackage.nested_module import TestNestedBase

                    class TestNestedChild(TestNestedBase):
                        def test_child_method(self):
                            assert True
                """)
            }

            with create_test_project(files) as project_path:
                env = os.environ.copy()
                env["PYTHONPATH"] = external_dir

                result = run_collection(project_path, env=env)

                self.assertEqual(
                    result.returncode,
                    0,
                    f"Nested package import should work. Error: {result.output}",
                )

                # Should find both inherited and local methods
                self.assertIn("test_nested_import.py::TestNestedChild::test_nested_method", result.output)
                self.assertIn("test_nested_import.py::TestNestedChild::test_child_method", result.output)
                self.assertIn("collected 2 items", result.output)

    def test_module_resolution_with_nested_directories(self) -> None:
        """Test that module resolution uses session root for imports from nested directories."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)
            subdir_path = project_path / "tests" / "unit"
            subdir_path.mkdir(parents=True)

            # Create a base test module in project root
            base_content = textwrap.dedent("""
                class TestBase:
                    def test_base_method(self):
                        assert True
            """)
            (project_path / "test_base.py").write_text(base_content)

            # Create a test file in nested directory that imports from root
            nested_content = textwrap.dedent("""
                from test_base import TestBase

                class TestNested(TestBase):
                    def test_nested_method(self):
                        assert True
            """)
            (subdir_path / "test_nested.py").write_text(nested_content)

            # Run collection from project root
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0)
            # Should find both the base test and the nested test with inheritance
            self.assertIn("test_base.py::TestBase::test_base_method", result.output)

            # Use platform-agnostic path construction
            nested_test_path = str(Path("tests", "unit", "test_nested.py"))
            self.assertIn(f"{nested_test_path}::TestNested::test_base_method", result.output)
            self.assertIn(f"{nested_test_path}::TestNested::test_nested_method", result.output)


if __name__ == "__main__":
    unittest.main()
