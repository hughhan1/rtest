"""End-to-end integration tests for rtest with xdist-group support"""

import tempfile
import unittest
import subprocess
import sys
import os
from pathlib import Path


class TestEndToEnd(unittest.TestCase):
    """Test complete workflows from collection to execution."""

    def setUp(self):
        """Build the project before running tests."""
        # Try to build the project
        result = subprocess.run(
            ["cargo", "build", "--bin", "rtest"],
            capture_output=True,
            text=True
        )
        if result.returncode != 0:
            self.skipTest(f"Failed to build rtest: {result.stderr}")
        
        self.rtest_path = "./target/debug/rtest"
        if not os.path.exists(self.rtest_path):
            self.skipTest("rtest binary not found")

    def test_basic_execution_workflow(self):
        """Test basic collection and execution workflow."""
        with tempfile.TemporaryDirectory() as temp_dir:
            test_file = Path(temp_dir) / "test_basic.py"
            test_file.write_text('''
def test_pass():
    assert True

def test_another():
    assert 1 + 1 == 2
''')

            # Run tests
            result = subprocess.run(
                [self.rtest_path, str(test_file)],
                capture_output=True,
                text=True
            )
            
            self.assertEqual(result.returncode, 0, f"Tests failed: {result.stderr}")
            self.assertIn("2 passed", result.stdout)

    def test_xdist_group_distribution(self):
        """Test that xdist groups are properly distributed."""
        with tempfile.TemporaryDirectory() as temp_dir:
            project_dir = Path(temp_dir)
            
            # Create test files with different groups
            (project_dir / "test_db.py").write_text('''
import pytest

@pytest.mark.xdist_group("database")
def test_db_create():
    import time
    time.sleep(0.1)  # Simulate DB operation
    assert True

@pytest.mark.xdist_group("database")
def test_db_update():
    import time
    time.sleep(0.1)
    assert True

@pytest.mark.xdist_group("database")
def test_db_delete():
    import time
    time.sleep(0.1)
    assert True
''')

            (project_dir / "test_api.py").write_text('''
from pytest import mark

@mark.xdist_group("api")
def test_api_get():
    import time
    time.sleep(0.1)  # Simulate API call
    assert True

@mark.xdist_group("api")
def test_api_post():
    import time
    time.sleep(0.1)
    assert True
''')

            (project_dir / "test_ui.py").write_text('''
import pytest as pt

@pt.mark.xdist_group("ui")
def test_ui_render():
    assert True

def test_ui_no_group():
    assert True
''')

            # Run with loadgroup distribution
            result = subprocess.run(
                [self.rtest_path, "--dist", "loadgroup", "-n", "3", str(project_dir)],
                capture_output=True,
                text=True
            )
            
            self.assertEqual(result.returncode, 0, f"Tests failed: {result.stderr}")
            self.assertIn("7 passed", result.stdout)
            
            # Should see worker indicators
            output_lower = result.stdout.lower()
            self.assertIn("worker", output_lower)

    def test_collection_only_with_groups(self):
        """Test collection-only mode shows correct test counts."""
        with tempfile.TemporaryDirectory() as temp_dir:
            test_file = Path(temp_dir) / "test_collection.py"
            test_file.write_text('''
import pytest
from pytest import mark
import pytest as pt

@pytest.mark.xdist_group("group1")
def test_one():
    pass

@mark.xdist_group("group2")
def test_two():
    pass

@pt.mark.xdist_group("group3")
def test_three():
    pass

def test_four():
    pass

class TestClass:
    @pytest.mark.xdist_group("class_group")
    def test_method(self):
        pass
''')

            result = subprocess.run(
                [self.rtest_path, "--collect-only", str(test_file)],
                capture_output=True,
                text=True
            )
            
            self.assertEqual(result.returncode, 0)
            
            # Should list all 5 tests
            for test_name in ["test_one", "test_two", "test_three", "test_four", "test_method"]:
                self.assertIn(test_name, result.stdout)
            
            self.assertIn("collected 5 items", result.stdout)

    def test_mixed_import_styles(self):
        """Test that mixed import styles work correctly."""
        with tempfile.TemporaryDirectory() as temp_dir:
            test_file = Path(temp_dir) / "test_mixed.py"
            test_file.write_text('''
# Mix different import styles in one file
import pytest
from pytest import mark
import pytest as pt
from pytest import mark as m

@pytest.mark.xdist_group("style1")
def test_style1():
    assert True

@mark.xdist_group("style2") 
def test_style2():
    assert True

@pt.mark.xdist_group("style3")
def test_style3():
    assert True

@m.xdist_group("style4")
def test_style4():
    assert True

# Verify they can coexist
@pytest.mark.skip
@mark.xdist_group("mixed")
def test_mixed_decorators():
    assert False  # Should be skipped
''')

            result = subprocess.run(
                [self.rtest_path, "-v", str(test_file)],
                capture_output=True,
                text=True
            )
            
            # 4 tests should pass, 1 should be skipped
            self.assertIn("4 passed", result.stdout)
            self.assertIn("1 skipped", result.stdout)

    def test_error_reporting(self):
        """Test that errors are reported correctly."""
        with tempfile.TemporaryDirectory() as temp_dir:
            test_file = Path(temp_dir) / "test_errors.py"
            test_file.write_text('''
import pytest

@pytest.mark.xdist_group("error_group")
def test_will_fail():
    assert False, "This test intentionally fails"

def test_will_pass():
    assert True
''')

            result = subprocess.run(
                [self.rtest_path, str(test_file)],
                capture_output=True,
                text=True
            )
            
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("1 failed", result.stdout)
            self.assertIn("1 passed", result.stdout)
            self.assertIn("This test intentionally fails", result.stdout)


if __name__ == "__main__":
    unittest.main()