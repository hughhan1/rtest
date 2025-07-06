"""Integration tests for AST parsing with all import patterns"""

import tempfile
import unittest
from pathlib import Path
from io import StringIO
from unittest.mock import patch

from rtest._rtest import run_tests


class TestASTParsingIntegration(unittest.TestCase):
    """Test that AST parsing correctly handles all pytest import patterns."""

    def test_all_import_patterns(self):
        """Test collection with all supported import patterns."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)
            
            # Create test file with all import patterns
            test_file = project_path / "test_imports.py"
            test_file.write_text('''
# Pattern 1: Simple import
import pytest

# Pattern 2: From import
from pytest import mark

# Pattern 3: Aliased import
import pytest as pt

# Pattern 4: From import with alias
from pytest import mark as m

# Pattern 5: Multiple from imports
from pytest import mark, fixture

# Test basic pytest.mark.xdist_group
@pytest.mark.xdist_group("group1")
def test_basic_import():
    assert True

# Test from import mark
@mark.xdist_group("group2")
def test_from_import():
    assert True

# Test aliased pytest
@pt.mark.xdist_group("group3")
def test_aliased_import():
    assert True

# Test aliased mark
@m.xdist_group("group4")
def test_aliased_mark():
    assert True

# Test without group
def test_no_group():
    assert True

# Test class with method groups
class TestClassGroups:
    @pytest.mark.xdist_group("method_group")
    def test_method_with_group(self):
        assert True
    
    def test_method_no_group(self):
        assert True
''')

            # Run collection
            args = ["--collect-only", str(test_file)]
            
            with patch("sys.stdout", new=StringIO()) as fake_out:
                result = run_tests(args)
                output = fake_out.getvalue()
            
            # Verify all tests were collected
            expected_tests = [
                "test_basic_import",
                "test_from_import",
                "test_aliased_import",
                "test_aliased_mark",
                "test_no_group",
                "TestClassGroups::test_method_with_group",
                "TestClassGroups::test_method_no_group",
            ]
            
            for test_name in expected_tests:
                self.assertIn(test_name, output, f"Test {test_name} not found in collection")
            
            # Count collected items
            collected_count = output.count("test_imports.py::")
            self.assertEqual(collected_count, 7, "Should collect exactly 7 tests")

    def test_complex_decorator_combinations(self):
        """Test complex decorator combinations with xdist_group."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)
            
            test_file = project_path / "test_complex.py"
            test_file.write_text('''
import pytest
from pytest import mark

# Multiple decorators with xdist_group
@pytest.mark.skip
@pytest.mark.xdist_group(name="complex_group")
@pytest.mark.parametrize("value", [1, 2, 3])
def test_multiple_decorators(value):
    assert value > 0

# Stacked mark decorators
@mark.skip
@mark.xdist_group("stacked")
def test_stacked_marks():
    assert True

# Class-level and method-level groups
@pytest.mark.xdist_group("class_level")
class TestMixedGroups:
    def test_inherits_class_group(self):
        assert True
    
    @pytest.mark.xdist_group("overrides_class")
    def test_override_group(self):
        assert True
''')

            args = ["--collect-only", str(test_file)]
            
            with patch("sys.stdout", new=StringIO()) as fake_out:
                result = run_tests(args)
                output = fake_out.getvalue()
            
            # Parametrized test should create 3 items
            self.assertEqual(output.count("test_multiple_decorators"), 3)
            self.assertIn("test_stacked_marks", output)
            self.assertIn("TestMixedGroups", output)

    def test_distribution_with_xdist_groups(self):
        """Test that tests with xdist_group are distributed correctly."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_path = Path(temp_dir)
            
            # Create test files with different groups
            test_db = project_path / "test_database.py"
            test_db.write_text('''
import pytest

@pytest.mark.xdist_group("database")
def test_db_connection():
    assert True

@pytest.mark.xdist_group("database")
def test_db_query():
    assert True

@pytest.mark.xdist_group("database")
def test_db_transaction():
    assert True
''')

            test_ui = project_path / "test_ui.py"
            test_ui.write_text('''
from pytest import mark

@mark.xdist_group("ui")
def test_ui_login():
    assert True

@mark.xdist_group("ui")
def test_ui_navigation():
    assert True
''')

            test_api = project_path / "test_api.py"
            test_api.write_text('''
import pytest as pt

@pt.mark.xdist_group("api")
def test_api_auth():
    assert True

def test_api_no_group():
    assert True
''')

            # Run with --dist loadgroup and multiple workers
            args = ["--dist", "loadgroup", "-n", "3", str(project_path)]
            
            with patch("sys.stdout", new=StringIO()) as fake_out:
                result = run_tests(args)
                output = fake_out.getvalue()
            
            # All tests should pass
            self.assertEqual(result, 0, "All tests should pass")
            
            # Should see evidence of parallel execution
            self.assertIn("worker", output.lower(), "Should show worker execution")


if __name__ == "__main__":
    unittest.main()