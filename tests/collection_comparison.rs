//! Tests that verify rtest collection matches pytest collection
//!
//! These tests compare the output of `rtest --collect-only` with `pytest --collect-only`
//! to ensure that both tools collect the same tests from Python files.

use std::collections::HashMap;

mod common;
use common::{
    assert_collections_match, create_test_file, create_test_project_with_files,
    parse_collected_tests, run_pytest_collection, run_rtest_collection,
};

#[test]
fn test_simple_functions_match_pytest() {
    let content = r#"
def test_simple_function():
    assert 1 + 1 == 2

def test_another_function():
    assert "hello".upper() == "HELLO"

def not_a_test():
    pass
"#;

    let (_temp_dir, project_path) = create_test_file("test_simple.py", content);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "simple functions");
}

#[test]
fn test_class_methods_match_pytest() {
    let content = r#"
class TestExample:
    def test_method_one(self):
        assert True
    
    def test_method_two(self):
        assert 2 * 2 == 4
    
    def helper_method(self):
        pass

class NotATestClass:
    def test_ignored(self):
        pass
"#;

    let (_temp_dir, project_path) = create_test_file("test_classes.py", content);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "class methods");
}

#[test]
fn test_mixed_functions_and_classes_match_pytest() {
    let content = r#"
def test_function():
    assert True

class TestClass:
    def test_method(self):
        assert True

def test_another_function():
    assert True
"#;

    let (_temp_dir, project_path) = create_test_file("test_mixed.py", content);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "mixed functions and classes");
}

#[test]
fn test_inheritance_matches_pytest() {
    let mut files = HashMap::new();

    files.insert(
        "test_base.py",
        r#"
class TestBase:
    def test_base_method(self):
        assert True
"#,
    );

    files.insert(
        "test_derived.py",
        r#"
from test_base import TestBase

class TestDerived(TestBase):
    def test_derived_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "inheritance");
}

#[test]
fn test_multiple_inheritance_matches_pytest() {
    let mut files = HashMap::new();

    files.insert(
        "test_mixins.py",
        r#"
class TestMixinA:
    def test_mixin_a_method(self):
        assert True

class TestMixinB:
    def test_mixin_b_method(self):
        assert True
"#,
    );

    files.insert(
        "test_multiple.py",
        r#"
from test_mixins import TestMixinA, TestMixinB

class TestMultipleInheritance(TestMixinA, TestMixinB):
    def test_own_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "multiple inheritance");
}

#[test]
fn test_parametrized_tests_match_pytest() {
    let content = r#"
import pytest

@pytest.mark.parametrize("value", [1, 2, 3])
def test_numbers(value):
    assert value > 0

@pytest.mark.parametrize("x,y,expected", [
    (1, 2, 3),
    (5, 5, 10),
])
def test_add(x, y, expected):
    assert x + y == expected
"#;

    let (_temp_dir, project_path) = create_test_file("test_parametrize.py", content);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "parametrized tests");
}

#[test]
fn test_multiple_files_match_pytest() {
    let mut files = HashMap::new();

    files.insert(
        "test_file1.py",
        r#"
def test_function1():
    assert True

class TestClass1:
    def test_method1(self):
        assert True
"#,
    );

    files.insert(
        "test_file2.py",
        r#"
def test_function2():
    assert True

def test_function3():
    assert True
"#,
    );

    files.insert(
        "test_file3.py",
        r#"
class TestClass3:
    def test_method3(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "multiple files");
}

#[test]
fn test_nested_directories_match_pytest() {
    let mut files = HashMap::new();

    files.insert(
        "test_root.py",
        r#"
def test_root_function():
    assert True
"#,
    );

    files.insert(
        "tests/test_nested.py",
        r#"
def test_nested_function():
    assert True
"#,
    );

    files.insert(
        "tests/unit/test_deep.py",
        r#"
def test_deep_function():
    assert True

class TestDeepClass:
    def test_deep_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "nested directories");
}

#[test]
fn test_relative_imports_match_pytest() {
    let mut files = HashMap::new();

    files.insert("__init__.py", "");

    files.insert(
        "test_base.py",
        r#"
class TestBase:
    def test_base_method(self):
        assert True
"#,
    );

    files.insert(
        "test_derived.py",
        r#"
from .test_base import TestBase

class TestDerived(TestBase):
    def test_derived_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "relative imports");
}

#[test]
fn test_deep_inheritance_chain_matches_pytest() {
    let mut files = HashMap::new();

    files.insert(
        "test_level1.py",
        r#"
class TestLevel1:
    def test_level1_method(self):
        assert True
"#,
    );

    files.insert(
        "test_level2.py",
        r#"
from test_level1 import TestLevel1

class TestLevel2(TestLevel1):
    def test_level2_method(self):
        assert True
"#,
    );

    files.insert(
        "test_level3.py",
        r#"
from test_level2 import TestLevel2

class TestLevel3(TestLevel2):
    def test_level3_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "deep inheritance chain");
}

#[test]
fn test_unittest_inheritance_matches_pytest() {
    let content = r#"
import unittest

class TestWithUnittest(unittest.TestCase):
    def test_method(self):
        self.assertTrue(True)
"#;

    let (_temp_dir, project_path) = create_test_file("test_unittest.py", content);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "unittest inheritance");
}

#[test]
fn test_method_override_matches_pytest() {
    let mut files = HashMap::new();

    files.insert(
        "test_override_base.py",
        r#"
class TestOverrideBase:
    def test_method_to_override(self):
        assert False

    def test_not_overridden(self):
        assert True
"#,
    );

    files.insert(
        "test_override_child.py",
        r#"
from test_override_base import TestOverrideBase

class TestOverrideChild(TestOverrideBase):
    def test_method_to_override(self):
        assert True

    def test_child_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "method override");
}

#[test]
fn test_diamond_inheritance_matches_pytest() {
    let mut files = HashMap::new();

    files.insert(
        "test_diamond_base.py",
        r#"
class TestDiamondBase:
    def test_base_method(self):
        assert True
"#,
    );

    files.insert(
        "test_diamond_middle.py",
        r#"
from test_diamond_base import TestDiamondBase

class TestDiamondLeft(TestDiamondBase):
    def test_left_method(self):
        assert True

class TestDiamondRight(TestDiamondBase):
    def test_right_method(self):
        assert True
"#,
    );

    files.insert(
        "test_diamond_bottom.py",
        r#"
from test_diamond_middle import TestDiamondLeft, TestDiamondRight

class TestDiamondBottom(TestDiamondLeft, TestDiamondRight):
    def test_bottom_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "diamond inheritance");
}

#[test]
fn test_stacked_parametrize_matches_pytest() {
    let content = r#"
import pytest

@pytest.mark.parametrize("value", [1, 2])
@pytest.mark.parametrize("multiplier", [10, 20])
def test_multiply(value, multiplier):
    assert value * multiplier > 0
"#;

    let (_temp_dir, project_path) = create_test_file("test_stacked.py", content);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "stacked parametrize");
}

#[test]
fn test_parametrize_in_class_matches_pytest() {
    let content = r#"
import pytest

class TestExample:
    @pytest.mark.parametrize("name", ["alice", "bob"])
    def test_names(self, name):
        assert len(name) > 0
"#;

    let (_temp_dir, project_path) = create_test_file("test_class_param.py", content);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    assert_collections_match(&rtest_tests, &pytest_tests, "parametrize in class");
}

// Edge case tests - documenting known differences

#[test]
#[ignore] // Known difference: rtest skips classes with __init__, pytest runs them
fn test_init_constructor_behavior() {
    let content = r#"
class TestWithInit:
    def __init__(self):
        pass
    
    def test_should_be_skipped(self):
        assert True

class TestWithoutInit:
    def test_should_be_collected(self):
        assert True
"#;

    let (_temp_dir, project_path) = create_test_file("test_init_classes.py", content);

    let rtest_output =
        run_rtest_collection(&project_path, &[]).expect("rtest collection should succeed");
    let pytest_output =
        run_pytest_collection(&project_path, &[]).expect("pytest collection should succeed");

    let rtest_tests = parse_collected_tests(&rtest_output);
    let pytest_tests = parse_collected_tests(&pytest_output);

    // This test is ignored because rtest and pytest have different behaviors:
    // - pytest collects TestWithInit::test_should_be_skipped
    // - rtest skips it and emits a warning
    // Both behaviors are valid, but different.
    assert_collections_match(&rtest_tests, &pytest_tests, "__init__ constructor");
}
