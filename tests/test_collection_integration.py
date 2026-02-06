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
    get_collected_count,
    run_collection,
)

from rtest._rtest import run_tests
from rtest.exit_code import ExitCodeValues


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

    def test_relative_import_same_directory(self) -> None:
        """Test relative imports in the same directory."""
        files = {
            "__init__.py": "",
            "test_base.py": textwrap.dedent("""
                class TestBase:
                    def test_base_method(self):
                        assert True
            """),
            "test_derived.py": textwrap.dedent("""
                from .test_base import TestBase

                class TestDerived(TestBase):
                    def test_derived_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                "test_base.py::TestBase::test_base_method",
                "test_derived.py::TestDerived::test_base_method",
                "test_derived.py::TestDerived::test_derived_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)

    def test_relative_import_package_structure(self) -> None:
        """Test relative imports in package structure."""
        files = {
            "tests/__init__.py": "",
            "tests/test_base.py": textwrap.dedent("""
                class TestBase:
                    def test_base_method(self):
                        assert True
            """),
            "tests/test_derived.py": textwrap.dedent("""
                from .test_base import TestBase

                class TestDerived(TestBase):
                    def test_derived_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                f"{Path('tests/test_base.py')}::TestBase::test_base_method",
                f"{Path('tests/test_derived.py')}::TestDerived::test_base_method",
                f"{Path('tests/test_derived.py')}::TestDerived::test_derived_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)

    def test_relative_import_parent_directory(self) -> None:
        """Test relative imports from parent directory."""
        files = {
            "package/__init__.py": "",
            "package/test_base.py": textwrap.dedent("""
                class TestBase:
                    def test_base_method(self):
                        assert True
            """),
            "package/subpackage/__init__.py": "",
            "package/subpackage/test_derived.py": textwrap.dedent("""
                from ..test_base import TestBase

                class TestDerived(TestBase):
                    def test_derived_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                # TestBase from test_base.py should be collected
                f"{Path('package/test_base.py')}::TestBase::test_base_method",
                # TestDerived inherits the method
                f"{Path('package/subpackage/test_derived.py')}::TestDerived::test_base_method",
                f"{Path('package/subpackage/test_derived.py')}::TestDerived::test_derived_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)

    def test_relative_import_multi_level(self) -> None:
        """Test multi-level relative imports."""
        files = {
            "package/__init__.py": "",
            "package/test_base.py": textwrap.dedent("""
                class TestBase:
                    def test_base_method(self):
                        assert True
            """),
            "package/level1/__init__.py": "",
            "package/level1/test_intermediate.py": textwrap.dedent("""
                from ..test_base import TestBase

                class TestIntermediate(TestBase):
                    def test_intermediate_method(self):
                        assert True
            """),
            "package/level1/level2/__init__.py": "",
            "package/level1/level2/test_derived.py": textwrap.dedent("""
                from ...test_base import TestBase
                from ..test_intermediate import TestIntermediate

                class TestDerivedFromBase(TestBase):
                    def test_derived_base_method(self):
                        assert True

                class TestDerivedFromIntermediate(TestIntermediate):
                    def test_derived_intermediate_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                # Base class from test_base.py
                f"{Path('package/test_base.py')}::TestBase::test_base_method",
                # Intermediate class from test_intermediate.py
                f"{Path('package/level1/test_intermediate.py')}::TestIntermediate::test_base_method",
                f"{Path('package/level1/test_intermediate.py')}::TestIntermediate::test_intermediate_method",
                # Derived from base
                f"{Path('package/level1/level2/test_derived.py')}::TestDerivedFromBase::test_base_method",
                f"{Path('package/level1/level2/test_derived.py')}::TestDerivedFromBase::test_derived_base_method",
                # Derived from intermediate
                f"{Path('package/level1/level2/test_derived.py')}::TestDerivedFromIntermediate::test_base_method",
                f"{Path('package/level1/level2/test_derived.py')}::TestDerivedFromIntermediate::test_intermediate_method",
                f"{Path('package/level1/level2/test_derived.py')}::TestDerivedFromIntermediate::test_derived_intermediate_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)

    def test_relative_import_beyond_top_level(self) -> None:
        """Test that relative imports beyond top-level package fail."""
        files = {
            "test_beyond.py": textwrap.dedent("""
                from .. import base

                class TestBeyond(base.TestBase):
                    def test_method(self):
                        assert True
            """)
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Should fail with import error
            self.assertNotEqual(result.returncode, 0, "Test collection should fail for beyond-top-level import")
            error_text = result.stdout + result.stderr
            self.assertIn(
                "Attempted relative import beyond top-level package (level 2 from depth 1)",
                error_text,
            )

    def test_relative_import_beyond_top_level_from_subpackage(self) -> None:
        """Test that relative imports beyond top-level from subpackage fail with Empty module path error."""
        files = {
            "package/__init__.py": "",
            "package/test_beyond.py": textwrap.dedent("""
                from ... import base

                class TestBeyond(base.TestBase):
                    def test_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Should fail with an import error
            self.assertNotEqual(result.returncode, 0, "Test collection should fail for beyond-top-level import")
            error_text = result.stdout + result.stderr
            # Currently we get "Empty module path" as the final error
            # This happens because the error path eventually leads to an empty module path
            self.assertIn(
                "ImportError: Attempted relative import beyond top-level package (level 3 from depth 2)",
                error_text,
            )

    def test_relative_import_with_alias(self) -> None:
        """Test relative imports with aliases."""
        files = {
            "__init__.py": "",
            "test_base.py": textwrap.dedent("""
                class TestBase:
                    def test_base_method(self):
                        assert True
            """),
            "test_alias.py": textwrap.dedent("""
                from .test_base import TestBase as BaseTest

                class TestWithAlias(BaseTest):
                    def test_alias_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                "test_base.py::TestBase::test_base_method",
                "test_alias.py::TestWithAlias::test_base_method",
                "test_alias.py::TestWithAlias::test_alias_method",
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

    def test_unresolvable_base_class_graceful(self) -> None:
        """Test that unresolvable base classes are handled gracefully (issue #131)."""
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
            result = run_collection(project_path)

            # Should succeed — the class's own test methods are still collected
            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")
            assert_tests_found(
                result.output_lines,
                ["test_unresolvable.py::TestWithUnresolvableImportedBase::test_method"],
            )

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

    def test_lazy_collection_single_file_skips_others(self) -> None:
        """Specifying a single file should only parse that file.

        Other files with syntax errors should not cause collection to fail.
        """
        files = {
            "test_target.py": textwrap.dedent("""
                def test_in_target():
                    assert True
            """),
            "test_broken.py": textwrap.dedent("""
                def test_broken():
                    if True  # Missing colon - syntax error
                        pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path, paths=["test_target.py"])

            self.assertEqual(result.returncode, 0)
            assert_tests_found(result.output_lines, ["test_target.py::test_in_target"])
            assert_patterns_not_found(result.output, ["test_broken.py"])
            self.assertEqual(get_collected_count(result.output), 1)

    def test_lazy_collection_directory(self) -> None:
        """Specifying a directory should only parse files in that directory."""
        files = {
            "unit/test_unit.py": "def test_unit(): pass",
            "integration/test_integration.py": "def broken(:",  # Syntax error
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path, paths=["unit/"])

            self.assertEqual(result.returncode, 0)
            assert_tests_found(result.output_lines, ["test_unit.py::test_unit"])
            assert_patterns_not_found(result.output, ["test_integration"])

    def test_lazy_collection_multiple_paths(self) -> None:
        """Multiple files and directories should only parse specified paths."""
        files = {
            "test_first.py": "def test_first(): pass",
            "test_second.py": "def test_second(): pass",
            "subdir/test_subdir.py": "def test_in_subdir(): pass",
            "other/test_broken.py": "def broken(:",  # Syntax error
        }

        with create_test_project(files) as project_path:
            # Test with multiple files
            result = run_collection(project_path, paths=["test_first.py", "test_second.py"])
            self.assertEqual(result.returncode, 0)
            assert_tests_found(
                result.output_lines,
                ["test_first.py::test_first", "test_second.py::test_second"],
            )
            assert_patterns_not_found(result.output, ["test_broken", "test_subdir"])
            self.assertEqual(get_collected_count(result.output), 2)

            # Test with mixed file and directory
            result = run_collection(project_path, paths=["test_first.py", "subdir/"])
            self.assertEqual(result.returncode, 0)
            assert_tests_found(
                result.output_lines,
                ["test_first.py::test_first", "test_subdir.py::test_in_subdir"],
            )
            assert_patterns_not_found(result.output, ["test_broken", "test_second"])
            self.assertEqual(get_collected_count(result.output), 2)

    def test_lazy_collection_nonexistent_path_fails(self) -> None:
        """Nonexistent path should fail with error and USAGE_ERROR exit code."""
        files = {
            "test_exists.py": "def test_exists(): pass",
            "subdir/test_subdir.py": "def test_in_subdir(): pass",
        }

        with create_test_project(files) as project_path:
            # Single nonexistent file
            result = run_collection(project_path, paths=["nonexistent.py"])
            self.assertEqual(result.returncode, ExitCodeValues.USAGE_ERROR)
            self.assertIn("ERROR: file or directory not found:", result.output)
            self.assertIn("nonexistent.py", result.output)

            # Valid file + nonexistent file should still fail
            result = run_collection(project_path, paths=["test_exists.py", "nonexistent.py"])
            self.assertEqual(result.returncode, ExitCodeValues.USAGE_ERROR)
            self.assertIn("nonexistent.py", result.output)
            # Should NOT collect from valid file
            self.assertNotIn("collected", result.output)

            # Nonexistent directory
            result = run_collection(project_path, paths=["missing_dir/"])
            self.assertEqual(result.returncode, ExitCodeValues.USAGE_ERROR)
            self.assertIn("missing_dir", result.output)

    def test_generic_base_class_inheritance(self) -> None:
        """Test that collection handles generic base classes with type parameters (issue #120)."""
        files = {
            "test_generic.py": textwrap.dedent("""
                from typing import Generic, TypeVar

                T = TypeVar("T")

                class TestGenericBase(Generic[T]):
                    def test_base_method(self):
                        assert True

                class MyClass:
                    pass

                class TestDerived(TestGenericBase[MyClass]):
                    def test_derived_method(self):
                        assert True
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            expected_patterns = [
                # TestGenericBase tests
                "test_generic.py::TestGenericBase::test_base_method",
                # TestDerived should have inherited + own methods
                "test_generic.py::TestDerived::test_base_method",
                "test_generic.py::TestDerived::test_derived_method",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 3 items", result.output)

    def test_cases_with_enum_values(self) -> None:
        """Test that @cases with enum values generates correct test IDs."""
        files = {
            "test_enum_cases.py": textwrap.dedent("""
                from enum import Enum
                import rtest

                class Color(Enum):
                    RED = 1
                    GREEN = 2
                    BLUE = 3

                @rtest.mark.cases("color", [Color.RED, Color.GREEN, Color.BLUE])
                def test_with_enum(color):
                    assert color.value in [1, 2, 3]
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Should generate test IDs using the enum path
            expected_patterns = [
                "test_enum_cases.py::test_with_enum[Color.RED]",
                "test_enum_cases.py::test_with_enum[Color.GREEN]",
                "test_enum_cases.py::test_with_enum[Color.BLUE]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 3 items", result.output)

    def test_cases_with_class_constants(self) -> None:
        """Test that @cases with class constants generates correct test IDs."""
        files = {
            "test_class_constants.py": textwrap.dedent("""
                import rtest

                class Config:
                    MAX_SIZE = 100
                    MIN_SIZE = 10
                    DEFAULT = 50

                @rtest.mark.cases("size", [Config.MAX_SIZE, Config.MIN_SIZE, Config.DEFAULT])
                def test_with_class_constant(size):
                    assert 10 <= size <= 100
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Should generate test IDs using the class attribute path
            expected_patterns = [
                "test_class_constants.py::test_with_class_constant[Config.MAX_SIZE]",
                "test_class_constants.py::test_with_class_constant[Config.MIN_SIZE]",
                "test_class_constants.py::test_with_class_constant[Config.DEFAULT]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 3 items", result.output)

    def test_cases_with_module_constants(self) -> None:
        """Test that @cases with module-level constants generates correct test IDs."""
        files = {
            "test_module_constants.py": textwrap.dedent("""
                import rtest

                TEST_DATA = [1, 2, 3]

                @rtest.mark.cases("value", TEST_DATA)
                def test_with_module_constant(value):
                    assert value in [1, 2, 3]
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Should expand the module constant and generate IDs
            expected_patterns = [
                "test_module_constants.py::test_with_module_constant[TEST_DATA[1]]",
                "test_module_constants.py::test_with_module_constant[TEST_DATA[2]]",
                "test_module_constants.py::test_with_module_constant[TEST_DATA[3]]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 3 items", result.output)

    def test_cases_with_nested_class_constants(self) -> None:
        """Test that @cases with nested class constants generates correct test IDs."""
        files = {
            "test_nested_constants.py": textwrap.dedent("""
                import rtest

                class Outer:
                    class Inner:
                        VALUE_A = 1
                        VALUE_B = 2

                @rtest.mark.cases("value", [Outer.Inner.VALUE_A, Outer.Inner.VALUE_B])
                def test_with_nested_constant(value):
                    assert value in [1, 2]
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Should generate test IDs using the full nested path
            expected_patterns = [
                "test_nested_constants.py::test_with_nested_constant[Outer.Inner.VALUE_A]",
                "test_nested_constants.py::test_with_nested_constant[Outer.Inner.VALUE_B]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 2 items", result.output)

    def test_class_level_parametrize(self) -> None:
        """Test that class-level @pytest.mark.parametrize is expanded to all methods (issue #133)."""
        files = {
            "test_class_parametrize.py": textwrap.dedent("""
                import pytest

                @pytest.mark.parametrize("x", [True, False])
                class TestClassLevel:
                    def test_foo(self, x):
                        pass

                    def test_bar(self, x):
                        pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Class parametrize should expand to all methods
            expected_patterns = [
                "test_class_parametrize.py::TestClassLevel::test_foo[True]",
                "test_class_parametrize.py::TestClassLevel::test_foo[False]",
                "test_class_parametrize.py::TestClassLevel::test_bar[True]",
                "test_class_parametrize.py::TestClassLevel::test_bar[False]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 4 items", result.output)

    def test_class_and_method_parametrize_cartesian(self) -> None:
        """Test cartesian product of class-level and method-level parametrize."""
        files = {
            "test_combined.py": textwrap.dedent("""
                import pytest

                @pytest.mark.parametrize("x", [1, 2])
                class TestCombined:
                    @pytest.mark.parametrize("y", ["a", "b"])
                    def test_method(self, x, y):
                        pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Should produce cartesian product: method params vary fastest (innermost)
            expected_patterns = [
                "test_combined.py::TestCombined::test_method[a-1]",
                "test_combined.py::TestCombined::test_method[a-2]",
                "test_combined.py::TestCombined::test_method[b-1]",
                "test_combined.py::TestCombined::test_method[b-2]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 4 items", result.output)

    def test_multiple_class_level_parametrize(self) -> None:
        """Test multiple stacked class-level parametrize decorators."""
        files = {
            "test_multi_class.py": textwrap.dedent("""
                import pytest

                @pytest.mark.parametrize("a", [1, 2])
                @pytest.mark.parametrize("b", [3, 4])
                class TestMultipleClassLevel:
                    def test_multi(self, a, b):
                        pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Should produce cartesian product of both class decorators
            expected_patterns = [
                "test_multi_class.py::TestMultipleClassLevel::test_multi[3-1]",
                "test_multi_class.py::TestMultipleClassLevel::test_multi[3-2]",
                "test_multi_class.py::TestMultipleClassLevel::test_multi[4-1]",
                "test_multi_class.py::TestMultipleClassLevel::test_multi[4-2]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 4 items", result.output)

    def test_inherited_method_with_class_parametrize(self) -> None:
        """Test that inherited methods get the child class's @parametrize decorator."""
        files = {
            "test_inherit.py": textwrap.dedent("""
                import pytest

                class TestParent:
                    def test_inherited(self, x):
                        pass

                @pytest.mark.parametrize("x", [1, 2])
                class TestChild(TestParent):
                    pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Inherited method should get the child class's parametrize
            expected_patterns = [
                "test_inherit.py::TestParent::test_inherited",  # Parent: no parametrize
                "test_inherit.py::TestChild::test_inherited[1]",  # Child: parametrized
                "test_inherit.py::TestChild::test_inherited[2]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 3 items", result.output)

    def test_parametrize_unicode_escape_ids(self) -> None:
        """Test that unicode escape sequences in parametrize generate correct IDs (issue #124)."""
        files = {
            "test_unicode.py": textwrap.dedent("""
                import pytest

                @pytest.mark.parametrize("test_value,expected", [
                    (True, '"\\\\u2603"'),
                    (False, '"\\u2603"'),
                ])
                def test_json_as_unicode(test_value, expected):
                    pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Should escape backslashes and unicode chars to match pytest's behavior
            expected_patterns = [
                'test_unicode.py::test_json_as_unicode[True-"\\\\u2603"]',
                'test_unicode.py::test_json_as_unicode[False-"\\u2603"]',
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 2 items", result.output)

    def test_collection_errors_return_nonzero_exit_code(self) -> None:
        """Test that collection errors return exit code 1, even when some tests are collected."""
        files = {
            "test_valid.py": textwrap.dedent("""
                def test_valid_function():
                    assert True

                class TestValidClass:
                    def test_method(self):
                        pass
            """),
            "test_syntax_error.py": "def test_broken(\n",
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            # Should return exit code 1 due to collection errors
            self.assertEqual(
                result.returncode,
                ExitCodeValues.TESTS_FAILED,
                f"Expected exit code 1 for collection errors, got {result.returncode}. Output: {result.output}",
            )

            # Should still display the valid tests that were collected
            self.assertIn("test_valid.py::test_valid_function", result.output)
            self.assertIn("test_valid.py::TestValidClass::test_method", result.output)

            # Should display error information
            self.assertIn("ERRORS", result.output)
            self.assertIn("test_syntax_error.py", result.output)

    def test_parametrize_with_dataclass_instances(self) -> None:
        """Test that @parametrize with dataclass instances generates positional IDs (issue #134)."""
        files = {
            "test_dataclass_params.py": textwrap.dedent("""
                from dataclasses import dataclass
                import pytest

                @dataclass
                class MyData:
                    value: int

                @pytest.mark.parametrize("data", [MyData(1), MyData(2), MyData(3)])
                def test_with_dataclass(data):
                    assert data.value in [1, 2, 3]
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Should generate positional IDs like data0, data1, data2
            expected_patterns = [
                "test_dataclass_params.py::test_with_dataclass[data0]",
                "test_dataclass_params.py::test_with_dataclass[data1]",
                "test_dataclass_params.py::test_with_dataclass[data2]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 3 items", result.output)

    def test_parametrize_with_nested_dicts(self) -> None:
        """Test that @parametrize with nested dicts generates positional IDs (issue #134)."""
        files = {
            "test_dict_params.py": textwrap.dedent("""
                import pytest

                @pytest.mark.parametrize("config", [
                    {"nested": {"key": 1}},
                    {"nested": {"key": 2}},
                ])
                def test_with_nested_dict(config):
                    assert "nested" in config
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Should generate positional IDs like config0, config1
            expected_patterns = [
                "test_dict_params.py::test_with_nested_dict[config0]",
                "test_dict_params.py::test_with_nested_dict[config1]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 2 items", result.output)

    def test_parametrize_with_mixed_literal_and_complex(self) -> None:
        """Test that @parametrize with mixed literals and complex objects works correctly."""
        files = {
            "test_mixed_params.py": textwrap.dedent("""
                from dataclasses import dataclass
                import pytest

                @dataclass
                class Config:
                    name: str

                @pytest.mark.parametrize("value", [
                    1,
                    "string",
                    Config("test"),
                    None,
                ])
                def test_mixed_types(value):
                    pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Literals should have their value as ID, complex objects get positional ID
            expected_patterns = [
                "test_mixed_params.py::test_mixed_types[1]",
                "test_mixed_params.py::test_mixed_types[string]",
                "test_mixed_params.py::test_mixed_types[value2]",
                "test_mixed_params.py::test_mixed_types[None]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 4 items", result.output)

    def test_parametrize_with_class_instance_call(self) -> None:
        """Test that @parametrize with regular class instantiation generates positional IDs."""
        files = {
            "test_class_params.py": textwrap.dedent("""
                import pytest

                class MyClass:
                    def __init__(self, value):
                        self.value = value

                @pytest.mark.parametrize("obj", [MyClass(1), MyClass(2)])
                def test_with_class_instance(obj):
                    assert obj.value in [1, 2]
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            expected_patterns = [
                "test_class_params.py::test_with_class_instance[obj0]",
                "test_class_params.py::test_with_class_instance[obj1]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 2 items", result.output)

    def test_parametrize_with_set_literal(self) -> None:
        """Test that @parametrize with set literals generates positional IDs."""
        files = {
            "test_set_params.py": textwrap.dedent("""
                import pytest

                @pytest.mark.parametrize("items", [{1, 2}, {3, 4, 5}])
                def test_with_set(items):
                    assert len(items) >= 2
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            expected_patterns = [
                "test_set_params.py::test_with_set[items0]",
                "test_set_params.py::test_with_set[items1]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 2 items", result.output)

    def test_parametrize_multiple_decorators_with_complex_objects(self) -> None:
        """Test multiple @parametrize decorators with complex objects."""
        files = {
            "test_multi_param.py": textwrap.dedent("""
                from dataclasses import dataclass
                import pytest

                @dataclass
                class Input:
                    x: int

                @dataclass
                class Expected:
                    y: int

                @pytest.mark.parametrize("expected", [Expected(10), Expected(20)])
                @pytest.mark.parametrize("input", [Input(1), Input(2)])
                def test_cartesian(input, expected):
                    pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Should generate cartesian product: 2 x 2 = 4 tests
            expected_patterns = [
                "test_multi_param.py::test_cartesian[input0-expected0]",
                "test_multi_param.py::test_cartesian[input0-expected1]",
                "test_multi_param.py::test_cartesian[input1-expected0]",
                "test_multi_param.py::test_cartesian[input1-expected1]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 4 items", result.output)

    def test_parametrize_multi_param_with_complex_in_tuple(self) -> None:
        """Test multi-param @parametrize where tuples contain complex objects."""
        files = {
            "test_tuple_complex.py": textwrap.dedent("""
                from dataclasses import dataclass
                import pytest

                @dataclass
                class Config:
                    value: int

                @pytest.mark.parametrize("a,b", [
                    (1, Config(10)),
                    (2, Config(20)),
                ])
                def test_mixed_tuple(a, b):
                    pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Literal values keep their IDs, opaque values get positional IDs
            # (1, Config(10)) -> "1-b0", (2, Config(20)) -> "2-b1"
            expected_patterns = [
                "test_tuple_complex.py::test_mixed_tuple[1-b0]",
                "test_tuple_complex.py::test_mixed_tuple[2-b1]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 2 items", result.output)

    def test_parametrize_complex_first_with_trailing_literals(self) -> None:
        """Test that complex first argument preserves trailing literal IDs (issue #137)."""
        files = {
            "test_complex_first.py": textwrap.dedent("""
                from dataclasses import dataclass
                import pytest

                @dataclass
                class MyData:
                    value: int

                @pytest.mark.parametrize("data, num, count", [
                    (MyData(1), 2, 3),
                    (MyData(4), 5, 6),
                ])
                def test_complex_first(data, num, count):
                    pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Complex first argument gets positional ID, literals keep their values
            # (MyData(1), 2, 3) -> "data0-2-3", (MyData(4), 5, 6) -> "data1-5-6"
            expected_patterns = [
                "test_complex_first.py::test_complex_first[data0-2-3]",
                "test_complex_first.py::test_complex_first[data1-5-6]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 2 items", result.output)

    def test_parametrize_all_opaque_multi_param(self) -> None:
        """Test that all-opaque multi-param tuples get per-position IDs."""
        files = {
            "test_all_opaque.py": textwrap.dedent("""
                from dataclasses import dataclass
                import pytest

                @dataclass
                class Data:
                    value: int

                @pytest.mark.parametrize("a, b", [
                    (Data(1), Data(2)),
                    (Data(3), Data(4)),
                ])
                def test_all_opaque(a, b):
                    pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Both params are opaque, each gets its own positional ID
            # (Data(1), Data(2)) -> "a0-b0", (Data(3), Data(4)) -> "a1-b1"
            expected_patterns = [
                "test_all_opaque.py::test_all_opaque[a0-b0]",
                "test_all_opaque.py::test_all_opaque[a1-b1]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 2 items", result.output)

    def test_parametrize_opaque_middle_position(self) -> None:
        """Test that opaque value in middle position gets correct argname."""
        files = {
            "test_opaque_middle.py": textwrap.dedent("""
                from dataclasses import dataclass
                import pytest

                @dataclass
                class Config:
                    value: int

                @pytest.mark.parametrize("start, config, end", [
                    (1, Config(10), 100),
                    (2, Config(20), 200),
                ])
                def test_opaque_middle(start, config, end):
                    pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Middle param is opaque, uses its argname "config"
            # (1, Config(10), 100) -> "1-config0-100", (2, Config(20), 200) -> "2-config1-200"
            expected_patterns = [
                "test_opaque_middle.py::test_opaque_middle[1-config0-100]",
                "test_opaque_middle.py::test_opaque_middle[2-config1-200]",
            ]

            assert_tests_found(result.output_lines, expected_patterns)
            self.assertIn("collected 2 items", result.output)

    def test_stdlib_inheritance_skipped_gracefully(self) -> None:
        """Test that inheriting from stdlib classes doesn't crash collection (issue #131)."""
        files = {
            "test_enum_subclass.py": textwrap.dedent("""
                from enum import Enum

                class TestStatus(Enum):
                    PASSED = 1
                    FAILED = 2

                class TestReal:
                    def test_works(self):
                        pass
            """),
            "test_typed_dict.py": textwrap.dedent("""
                from typing import TypedDict

                class TestConfig(TypedDict):
                    name: str
                    value: int

                class TestActual:
                    def test_real_test(self):
                        pass
            """),
            "test_collections.py": textwrap.dedent("""
                from collections import OrderedDict

                class TestOrderedDict(OrderedDict):
                    pass

                class TestSuite:
                    def test_something(self):
                        pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # Real test methods should still be collected
            assert_tests_found(
                result.output_lines,
                [
                    "test_enum_subclass.py::TestReal::test_works",
                    "test_typed_dict.py::TestActual::test_real_test",
                    "test_collections.py::TestSuite::test_something",
                ],
            )

    def test_external_package_inheritance_skipped_gracefully(self) -> None:
        """Test that inheriting from unresolvable external packages doesn't crash collection."""
        files = {
            "test_external.py": textwrap.dedent("""
                from some_nonexistent_package import BaseModel

                class TestModel(BaseModel):
                    def test_method(self):
                        pass

                class TestPlain:
                    def test_plain(self):
                        pass
            """),
        }

        with create_test_project(files) as project_path:
            result = run_collection(project_path)

            self.assertEqual(result.returncode, 0, f"Collection failed: {result.output}")

            # The plain test should be collected; TestModel's own method should also be collected
            assert_tests_found(
                result.output_lines,
                [
                    "test_external.py::TestPlain::test_plain",
                    "test_external.py::TestModel::test_method",
                ],
            )


if __name__ == "__main__":
    unittest.main()
