//! Integration tests for CLI functionality.

use std::collections::HashMap;
use std::process::Command;
use tempfile::TempDir;

mod common;
use common::{create_test_file, create_test_project_with_files, get_rtest_binary};

/// Test that the CLI shows help when requested
#[test]
fn test_cli_help() {
    let output = Command::new(get_rtest_binary())
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: rtest"));
    assert!(stdout.contains("--env"));
    assert!(stdout.contains("--numprocesses"));
    assert!(stdout.contains("--dist"));
}

/// Test that the CLI shows version when requested
#[test]
fn test_cli_version() {
    let output = Command::new(get_rtest_binary())
        .arg("--version")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("rtest"));
}

/// Test CLI argument parsing for distribution modes
#[test]
fn test_distribution_args() {
    // Test with default (load)
    let output = Command::new(get_rtest_binary())
        .arg("--help")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("[default: load]"));
}

/// Test that invalid arguments are rejected
#[test]
fn test_invalid_args() {
    let output = Command::new(get_rtest_binary())
        .arg("--invalid-flag")
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error") || stderr.contains("unrecognized"));
}

/// Test collection functionality with temporary test files
#[test]
fn test_collection_phase() {
    let (_temp_dir, project_path) = common::create_test_project();

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only", "test_sample.py"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    // Check the collection output
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Check that collection found the expected tests
    assert!(
        combined.contains("test_sample.py::test_simple_function"),
        "Expected to find test_simple_function in output: {combined}"
    );
    assert!(
        combined.contains("test_sample.py::test_another_function"),
        "Expected to find test_another_function in output: {combined}"
    );
    assert!(
        combined.contains("test_sample.py::TestExampleClass::test_method_one"),
        "Expected to find TestExampleClass::test_method_one in output: {combined}"
    );
    assert!(
        combined.contains("test_sample.py::TestExampleClass::test_method_two"),
        "Expected to find TestExampleClass::test_method_two in output: {combined}"
    );
}

/// Test collection-only mode
#[test]
fn test_collect_only_mode() {
    let (_temp_dir, project_path) = common::create_test_project();

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success(), "collect-only should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show collected tests without running them
    assert!(stdout.contains("collected"));
    assert!(stdout.contains("test_sample.py"));
}

/// Test specific file path collection
#[test]
fn test_specific_file_path() {
    let (_temp_dir, project_path) = common::create_test_project();

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only", "test_sample.py"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should only collect from specified file
    assert!(stdout.contains("test_sample.py"));
}

/// Test non-existent file handling
#[test]
fn test_nonexistent_file() {
    let (_temp_dir, project_path) = common::create_test_project();

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only", "nonexistent.py"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    // Should handle gracefully
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    // Should indicate no tests found or file not found
    assert!(
        combined.contains("No tests")
            || combined.contains("not found")
            || combined.contains("0 collected"),
        "Expected error message for nonexistent file, got: {combined}"
    );
}

/// Test verbose output mode
#[test]
fn test_verbose_mode() {
    let (_temp_dir, project_path) = common::create_test_project();

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only", "-v"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    // Check if verbose flag is recognized
    let _stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // The test should either succeed with verbose output or indicate that -v is not supported
    assert!(
        output.status.success() || stderr.contains("unexpected argument"),
        "Expected either success or clear error about -v flag"
    );
}

/// Test parallel execution options
#[test]
fn test_parallel_options() {
    let output = Command::new(get_rtest_binary())
        .args(["--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should show parallel execution options
    assert!(stdout.contains("-n") || stdout.contains("--numprocesses"));
    assert!(stdout.contains("--maxprocesses"));
}

/// Test empty project handling
#[test]
fn test_empty_project() {
    let temp_dir = tempfile::tempdir().unwrap();
    let project_path = temp_dir.path().join("test_project");
    std::fs::create_dir_all(&project_path).expect("Failed to create project directory");

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    // Should handle empty projects gracefully
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    assert!(
        combined.contains("0 collected") || combined.contains("No tests"),
        "Expected message about no tests found, got: {combined}"
    );
}

/// Test collection with syntax errors in test files
#[test]
fn test_syntax_error_handling() {
    let (_temp_dir, project_path) =
        create_test_file("test_syntax_error.py", "def test_function(\n    pass");

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only", "test_syntax_error.py"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    // Should handle syntax errors gracefully
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    // Should indicate syntax error or collection error or no tests
    assert!(
        combined.contains("error")
            || combined.contains("Error")
            || combined.contains("failed")
            || combined.contains("SyntaxError")
            || combined.contains("No tests"),
        "Expected error message for syntax error, got: {combined}"
    );
}

/// Test cross-module inheritance with multi-level chains
#[test]
fn test_cross_module_multi_level_inheritance() {
    let mut files = HashMap::new();

    files.insert(
        "test_base.py",
        r#"class TestBase:
    def test_base_method(self):
        assert True
    
    def test_another_base_method(self):
        assert True
"#,
    );

    files.insert(
        "test_derived.py",
        r#"from test_base import TestBase

class TestDerived(TestBase):
    def test_derived_method(self):
        assert True
"#,
    );

    files.insert(
        "test_multi_level.py",
        r#"from test_derived import TestDerived

class TestMultiLevel(TestDerived):
    def test_multi_level_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("=== test_cross_module_multi_level_inheritance debug ===");
    eprintln!("Exit status: {:?}", output.status);
    eprintln!("Stdout: {stdout}");
    eprintln!("Stderr: {stderr}");
    eprintln!("====================================================");

    assert!(output.status.success(), "Command should succeed");

    // Should find all tests including inherited ones
    assert!(stdout.contains("test_base.py::TestBase::test_base_method"));
    assert!(stdout.contains("test_base.py::TestBase::test_another_base_method"));
    assert!(stdout.contains("test_derived.py::TestDerived::test_base_method"));
    assert!(stdout.contains("test_derived.py::TestDerived::test_another_base_method"));
    assert!(stdout.contains("test_derived.py::TestDerived::test_derived_method"));
    assert!(stdout.contains("test_multi_level.py::TestMultiLevel::test_base_method"));
    assert!(stdout.contains("test_multi_level.py::TestMultiLevel::test_another_base_method"));
    assert!(stdout.contains("test_multi_level.py::TestMultiLevel::test_derived_method"));
    assert!(stdout.contains("test_multi_level.py::TestMultiLevel::test_multi_level_method"));

    // Should collect 9 tests total (2 + 3 + 4)
    assert!(stdout.contains("collected 9 items"));
}

/// Test multiple inheritance pattern
#[test]
fn test_multiple_inheritance() {
    let mut files = HashMap::new();

    files.insert(
        "test_mixins.py",
        r#"class TestMixinA:
    def test_mixin_a_method(self):
        assert True

class TestMixinB:
    def test_mixin_b_method(self):
        assert True
    
    def test_mixin_b_another(self):
        assert True
"#,
    );

    files.insert(
        "test_multiple.py",
        r#"from test_mixins import TestMixinA, TestMixinB

class TestMultipleInheritance(TestMixinA, TestMixinB):
    def test_own_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Command should succeed");

    // Check for all expected patterns
    assert!(stdout.contains("test_mixins.py::TestMixinA::test_mixin_a_method"));
    assert!(stdout.contains("test_mixins.py::TestMixinB::test_mixin_b_method"));
    assert!(stdout.contains("test_mixins.py::TestMixinB::test_mixin_b_another"));
    assert!(stdout.contains("test_multiple.py::TestMultipleInheritance::test_mixin_a_method"));
    assert!(stdout.contains("test_multiple.py::TestMultipleInheritance::test_mixin_b_method"));
    assert!(stdout.contains("test_multiple.py::TestMultipleInheritance::test_mixin_b_another"));
    assert!(stdout.contains("test_multiple.py::TestMultipleInheritance::test_own_method"));

    // Should collect 7 tests total
    assert!(stdout.contains("collected 7 items"));
}

/// Test diamond inheritance pattern
#[test]
fn test_diamond_inheritance() {
    let mut files = HashMap::new();

    files.insert(
        "test_diamond_base.py",
        r#"class TestDiamondBase:
    def test_base_method(self):
        assert True
"#,
    );

    files.insert(
        "test_diamond_middle.py",
        r#"from test_diamond_base import TestDiamondBase

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
        r#"from test_diamond_middle import TestDiamondLeft, TestDiamondRight

class TestDiamondBottom(TestDiamondLeft, TestDiamondRight):
    def test_bottom_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Command should succeed");

    // Check all expected patterns
    assert!(stdout.contains("test_diamond_base.py::TestDiamondBase::test_base_method"));
    assert!(stdout.contains("test_diamond_middle.py::TestDiamondLeft::test_base_method"));
    assert!(stdout.contains("test_diamond_middle.py::TestDiamondLeft::test_left_method"));
    assert!(stdout.contains("test_diamond_middle.py::TestDiamondRight::test_base_method"));
    assert!(stdout.contains("test_diamond_middle.py::TestDiamondRight::test_right_method"));
    assert!(stdout.contains("test_diamond_bottom.py::TestDiamondBottom::test_base_method"));
    assert!(stdout.contains("test_diamond_bottom.py::TestDiamondBottom::test_left_method"));
    assert!(stdout.contains("test_diamond_bottom.py::TestDiamondBottom::test_right_method"));
    assert!(stdout.contains("test_diamond_bottom.py::TestDiamondBottom::test_bottom_method"));
}

/// Test method override in inheritance
#[test]
fn test_method_override_inheritance() {
    let mut files = HashMap::new();

    files.insert(
        "test_override_base.py",
        r#"class TestOverrideBase:
    def test_method_to_override(self):
        assert False  # Base implementation
    
    def test_not_overridden(self):
        assert True
"#,
    );

    files.insert(
        "test_override_child.py",
        r#"from test_override_base import TestOverrideBase

class TestOverrideChild(TestOverrideBase):
    def test_method_to_override(self):
        assert True  # Overridden implementation
    
    def test_child_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Command should succeed");

    // Check all expected tests are found
    assert!(stdout.contains("test_override_base.py::TestOverrideBase::test_method_to_override"));
    assert!(stdout.contains("test_override_base.py::TestOverrideBase::test_not_overridden"));
    assert!(stdout.contains("test_override_child.py::TestOverrideChild::test_method_to_override"));
    assert!(stdout.contains("test_override_child.py::TestOverrideChild::test_not_overridden"));
    assert!(stdout.contains("test_override_child.py::TestOverrideChild::test_child_method"));
}

/// Test deep inheritance chain (5 levels)
#[test]
fn test_deep_inheritance_chain() {
    let mut files = HashMap::new();

    // Create level 1
    files.insert(
        "test_level1.py",
        r#"class TestLevel1:
    def test_level1_method(self):
        assert True
"#,
    );

    // Create level 2
    files.insert(
        "test_level2.py",
        r#"from test_level1 import TestLevel1

class TestLevel2(TestLevel1):
    def test_level2_method(self):
        assert True
"#,
    );

    // Create level 3
    files.insert(
        "test_level3.py",
        r#"from test_level2 import TestLevel2

class TestLevel3(TestLevel2):
    def test_level3_method(self):
        assert True
"#,
    );

    // Create level 4
    files.insert(
        "test_level4.py",
        r#"from test_level3 import TestLevel3

class TestLevel4(TestLevel3):
    def test_level4_method(self):
        assert True
"#,
    );

    // Create level 5
    files.insert(
        "test_level5.py",
        r#"from test_level4 import TestLevel4

class TestLevel5(TestLevel4):
    def test_level5_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Command should succeed");

    // Level 5 should have all 5 methods
    for level in 1..=5 {
        assert!(
            stdout.contains(&format!(
                "test_level5.py::TestLevel5::test_level{level}_method"
            )),
            "Expected to find level {level} method in level 5 class"
        );
    }

    // Total: 1 + 2 + 3 + 4 + 5 = 15 tests
    assert!(stdout.contains("collected 15 items"));
}

/// Test inheritance from non-test classes
#[test]
fn test_non_test_class_inheritance() {
    let (_temp_dir, project_path) = create_test_file(
        "test_helpers.py",
        r#"class BaseHelper:  # Not a test class
    def helper_method(self):
        return "helper"
    
    def test_should_not_be_collected(self):
        # This should not be collected as BaseHelper is not a test class
        assert True

class TestWithHelper(BaseHelper):
    def test_actual_test(self):
        assert self.helper_method() == "helper"
"#,
    );

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should only find the test in TestWithHelper, not in BaseHelper
    assert!(combined.contains("test_helpers.py::TestWithHelper::test_actual_test"));

    // Should NOT find these - BaseHelper might appear in inheritance info, but not as a collected test
    assert!(!combined.contains("test_helpers.py::BaseHelper::"));
    assert!(!combined.contains("test_should_not_be_collected"));
}

/// Test pattern filtering with -k option
#[test]
fn test_pattern_filtering() {
    let (_temp_dir, project_path) = common::create_test_project();

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only", "-k", "simple"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should filter tests by pattern (or indicate -k is not supported)
    assert!(
        combined.contains("test_simple_function") || combined.contains("unexpected argument"),
        "Expected filtered results or unsupported flag message, got: {combined}"
    );
}

/// Test multiple file paths
#[test]
fn test_multiple_paths() {
    let mut files = HashMap::new();
    files.insert("test_one.py", "def test_one():\n    pass");
    files.insert("test_two.py", "def test_two():\n    pass");

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only", "test_one.py", "test_two.py"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should handle multiple paths
    assert!(
        combined.contains("test_one") || combined.contains("test_two") || output.status.success(),
        "Expected to handle multiple paths, got: {combined}"
    );
}

/// Test class inheritance collection
#[test]
fn test_class_inheritance() {
    let mut files = HashMap::new();

    files.insert(
        "test_base.py",
        r#"class TestBase:
    def test_base_method(self):
        pass
"#,
    );

    files.insert(
        "test_child.py",
        r#"from test_base import TestBase

class TestChild(TestBase):
    def test_child_method(self):
        pass
"#,
    );

    let (temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only", "test_child.py"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Print debug info for diagnosing platform differences
    eprintln!("=== test_class_inheritance debug ===");
    eprintln!("Exit status: {:?}", output.status);
    eprintln!("Stdout: {stdout}");
    eprintln!("Stderr: {stderr}");
    eprintln!("Working dir: {:?}", temp_dir.path());
    eprintln!("==================================");

    // First check if the command executed successfully
    if !output.status.success() {
        panic!(
            "rtest command failed with status {:?}\nstderr: {}\nstdout: {}",
            output.status, stderr, stdout
        );
    }

    // Should collect both base and inherited test methods
    assert!(
        stdout.contains("test_child.py::TestChild::test_child_method"),
        "Expected to find child's own method, got stdout: {stdout}"
    );

    // This is the key assertion - inherited methods should be collected
    assert!(
        stdout.contains("test_child.py::TestChild::test_base_method"),
        "Expected to find inherited test_base_method from parent class, got stdout: {stdout}"
    );
}

/// Test classes with __init__ constructors
#[test]
fn test_init_constructor_handling() {
    let (_temp_dir, project_path) = create_test_file(
        "test_init_classes.py",
        r#"class TestWithInit:
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
"#,
    );

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should only collect TestWithoutInit
    assert!(combined.contains("test_init_classes.py::TestWithoutInit::test_should_be_collected"));
    assert!(combined.contains("collected 1 item"));

    // Should emit warnings for classes with __init__
    assert!(combined.contains("RtestCollectionWarning: cannot collect test class 'TestWithInit'"));
    assert!(
        combined.contains("RtestCollectionWarning: cannot collect test class 'TestBaseWithInit'")
    );
    assert!(combined
        .contains("RtestCollectionWarning: cannot collect test class 'TestDerivedFromInit'"));
}

/// Test circular inheritance detection
#[test]
fn test_circular_inheritance_detection() {
    let (_temp_dir, project_path) = create_test_file(
        "test_circular.py",
        r#"# Forward reference to TestB
class TestA(TestB):  # type: ignore
    def test_a_method(self):
        assert True

class TestB(TestA):
    def test_b_method(self):
        assert True
"#,
    );

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should exit with error code due to circular inheritance
    assert!(!output.status.success());
    // Error message should mention circular inheritance
    assert!(combined.contains("Circular inheritance detected"));
}

/// Test circular inheritance across modules
#[test]
fn test_circular_inheritance_cross_module() {
    let mut files = HashMap::new();

    files.insert(
        "test_circular_a.py",
        r#"from test_circular_b import TestB

class TestA(TestB):
    def test_a_method(self):
        assert True
"#,
    );

    files.insert(
        "test_circular_b.py",
        r#"from test_circular_a import TestA

class TestB(TestA):
    def test_b_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should exit with error code due to circular inheritance
    assert!(!output.status.success());
    // Error message should mention circular inheritance
    assert!(combined.contains("Circular inheritance detected"));
}

/// Test unresolvable base class errors
#[test]
fn test_unresolvable_base_class() {
    let (_temp_dir, project_path) = create_test_file(
        "test_unresolvable.py",
        r#"# Test with an imported base class that doesn't exist
from nonexistent_module import NonExistentClass

class TestWithUnresolvableImportedBase(NonExistentClass):
    def test_method(self):
        assert True
"#,
    );

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");

    // Should exit with error code due to import error
    assert!(!output.status.success());
    // Error message should mention the import error
    assert!(combined.contains("Could not find module: nonexistent_module"));
}

/// Test unittest.TestCase inheritance
#[test]
fn test_unittest_testcase_inheritance() {
    let (_temp_dir, project_path) = create_test_file(
        "test_unittest.py",
        r#"import unittest

class TestWithUnittestBase(unittest.TestCase):
    def test_method(self):
        self.assertTrue(True)
"#,
    );

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should succeed - unittest.TestCase inheritance is supported
    assert!(
        output.status.success(),
        "unittest.TestCase inheritance should be supported. Error: {stderr}"
    );

    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("test_unittest.py::TestWithUnittestBase::test_method"));
    assert!(combined.contains("collected 1 item"));
}

/// Test parameterized test patterns via inheritance
#[test]
fn test_parameterized_inheritance() {
    let (_temp_dir, project_path) = create_test_file(
        "test_parameterized.py",
        r#"class TestStringConcat:
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
"#,
    );

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "Parameterized test inheritance should work. Error: {stderr}"
    );

    // Should collect both test methods (one from each class)
    assert!(stdout.contains("test_parameterized.py::TestStringConcat::test_concatenation"));
    assert!(stdout.contains("test_parameterized.py::TestStringJoin::test_concatenation"));
    assert!(stdout.contains("collected 2 items"));
}

/// Test import pattern variations
#[test]
fn test_import_pattern_variations() {
    let mut files = HashMap::new();

    files.insert(
        "test_base_module.py",
        r#"class TestBase1:
    def test_base1_method(self):
        assert True

class TestBase2:
    def test_base2_method(self):
        assert True
"#,
    );

    files.insert(
        "test_import_patterns.py",
        r#"# Test different import styles
from test_base_module import TestBase1
import test_base_module

class TestImportStyle1(TestBase1):
    def test_style1_method(self):
        assert True

class TestImportStyle2(test_base_module.TestBase2):
    def test_style2_method(self):
        assert True
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Command should succeed");

    // Check all expected patterns
    assert!(stdout.contains("test_base_module.py::TestBase1::test_base1_method"));
    assert!(stdout.contains("test_base_module.py::TestBase2::test_base2_method"));
    assert!(stdout.contains("test_import_patterns.py::TestImportStyle1::test_base1_method"));
    assert!(stdout.contains("test_import_patterns.py::TestImportStyle1::test_style1_method"));
    assert!(stdout.contains("test_import_patterns.py::TestImportStyle2::test_base2_method"));
    assert!(stdout.contains("test_import_patterns.py::TestImportStyle2::test_style2_method"));
}

/// Test sys.path import resolution
#[test]
fn test_sys_path_import_resolution() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().join("test_project");
    let external_path = temp_dir.path().join("external_libs");

    std::fs::create_dir_all(&project_path).expect("Failed to create project directory");
    std::fs::create_dir_all(&external_path).expect("Failed to create external directory");

    // Create base test class in external directory
    let base_content = r#"class BaseTestClass:
    def test_base_method(self):
        assert True
"#;
    let base_path = external_path.join("base_module.py");
    std::fs::write(&base_path, base_content).expect("Failed to write base module");

    // Create test file that inherits from external module
    let test_content = r#"from base_module import BaseTestClass

class TestDerived(BaseTestClass):
    def test_derived_method(self):
        assert True
"#;
    let test_path = project_path.join("test_inheritance.py");
    std::fs::write(&test_path, test_content).expect("Failed to write test file");

    // Run test collection with PYTHONPATH set
    let mut cmd = Command::new(get_rtest_binary());
    cmd.args(["--collect-only"])
        .current_dir(&project_path)
        .env("PYTHONPATH", external_path.to_str().unwrap());

    let output = cmd.output().expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Command should succeed");

    // Should find both inherited and derived tests
    assert!(stdout.contains("test_inheritance.py::TestDerived::test_base_method"));
    assert!(stdout.contains("test_inheritance.py::TestDerived::test_derived_method"));
    assert!(stdout.contains("collected 2 items"));
}

/// Test PYTHONPATH priority over project directory
#[test]
fn test_pythonpath_multiple_directories() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().join("test_project");
    let external1 = temp_dir.path().join("external1");
    let external2 = temp_dir.path().join("external2");

    std::fs::create_dir_all(&project_path).expect("Failed to create project directory");
    std::fs::create_dir_all(&external1).expect("Failed to create external1 directory");
    std::fs::create_dir_all(&external2).expect("Failed to create external2 directory");

    // Create same module in multiple locations
    let module1_content = r#"class SharedModule:
    def test_from_external1(self):
        assert True
"#;
    std::fs::write(external1.join("shared.py"), module1_content)
        .expect("Failed to write to external1");

    let module2_content = r#"class SharedModule:
    def test_from_external2(self):
        assert True
"#;
    std::fs::write(external2.join("shared.py"), module2_content)
        .expect("Failed to write to external2");

    let project_module_content = r#"class SharedModule:
    def test_from_project(self):
        assert True
"#;
    std::fs::write(project_path.join("shared.py"), project_module_content)
        .expect("Failed to write to project");

    // Create test that imports the shared module
    let test_content = r#"from shared import SharedModule

class TestPriority(SharedModule):
    def test_priority_method(self):
        assert True
"#;
    std::fs::write(project_path.join("test_priority.py"), test_content)
        .expect("Failed to write test file");

    // Test with external1 first in PYTHONPATH
    let pythonpath = format!(
        "{}{}{}",
        external1.to_str().unwrap(),
        if cfg!(windows) { ";" } else { ":" },
        external2.to_str().unwrap()
    );

    let mut cmd = Command::new(get_rtest_binary());
    cmd.args(["--collect-only"])
        .current_dir(&project_path)
        .env("PYTHONPATH", &pythonpath);

    let output = cmd.output().expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("=== test_pythonpath_multiple_directories debug ===");
    eprintln!("Exit status: {:?}", output.status);
    eprintln!("Stdout: {stdout}");
    eprintln!("Stderr: {stderr}");
    eprintln!("PYTHONPATH: {pythonpath}");
    eprintln!("================================================");

    assert!(output.status.success(), "Command should succeed");

    // Should use external1's version (first in PYTHONPATH)
    assert!(stdout.contains("test_priority.py::TestPriority::test_from_external1"));
    assert!(stdout.contains("test_priority.py::TestPriority::test_priority_method"));
    assert!(!stdout.contains("test_from_external2"));
    assert!(!stdout.contains("test_from_project"));
}

/// Test importing and inheriting from site-packages
#[test]
fn test_site_packages_inheritance() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().join("test_project");
    let site_packages = temp_dir.path().join("site-packages");

    std::fs::create_dir_all(&project_path).expect("Failed to create project directory");
    std::fs::create_dir_all(&site_packages).expect("Failed to create site-packages directory");

    // Create a mock third-party test utilities module
    let utils_content = r#"class ThirdPartyTestBase:
    def test_third_party_method(self):
        assert True
"#;
    std::fs::write(
        site_packages.join("test_utils_third_party.py"),
        utils_content,
    )
    .expect("Failed to write third party module");

    // Create test that uses third-party module
    let test_content = r#"from test_utils_third_party import ThirdPartyTestBase

class TestWithThirdParty(ThirdPartyTestBase):
    def test_our_method(self):
        assert True
"#;
    std::fs::write(project_path.join("test_third_party.py"), test_content)
        .expect("Failed to write test file");

    // Run with site-packages in PYTHONPATH
    let mut cmd = Command::new(get_rtest_binary());
    cmd.args(["--collect-only"])
        .current_dir(&project_path)
        .env("PYTHONPATH", site_packages.to_str().unwrap());

    let output = cmd.output().expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);

    assert!(output.status.success(), "Command should succeed");

    // Should find both methods
    assert!(stdout.contains("test_third_party.py::TestWithThirdParty::test_third_party_method"));
    assert!(stdout.contains("test_third_party.py::TestWithThirdParty::test_our_method"));
    assert!(stdout.contains("collected 2 items"));
}

/// Test nested package imports from sys.path
#[test]
fn test_nested_package_import_from_sys_path() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().join("test_project");
    let external_path = temp_dir.path().join("external");

    std::fs::create_dir_all(&project_path).expect("Failed to create project directory");
    std::fs::create_dir_all(&external_path).expect("Failed to create external directory");

    // Create nested package structure
    let package_path = external_path.join("mypackage");
    let subpackage_path = package_path.join("testing");
    std::fs::create_dir_all(&subpackage_path).expect("Failed to create package directories");

    // Create __init__.py files
    std::fs::write(package_path.join("__init__.py"), "").expect("Failed to write package init");
    std::fs::write(subpackage_path.join("__init__.py"), "")
        .expect("Failed to write subpackage init");

    // Create base test class in nested package
    let base_content = r#"class PackageTestBase:
    def test_package_base_method(self):
        assert True
"#;
    std::fs::write(subpackage_path.join("base.py"), base_content)
        .expect("Failed to write base module");

    // Create test that imports from nested package
    let test_content = r#"from mypackage.testing.base import PackageTestBase

class TestNestedImport(PackageTestBase):
    def test_nested_method(self):
        assert True
"#;
    std::fs::write(project_path.join("test_nested.py"), test_content)
        .expect("Failed to write test file");

    // Run with external path in PYTHONPATH
    let mut cmd = Command::new(get_rtest_binary());
    cmd.args(["--collect-only"])
        .current_dir(&project_path)
        .env("PYTHONPATH", external_path.to_str().unwrap());

    let output = cmd.output().expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    eprintln!("=== test_nested_package_import_from_sys_path debug ===");
    eprintln!("Exit status: {:?}", output.status);
    eprintln!("Stdout: {stdout}");
    eprintln!("Stderr: {stderr}");
    eprintln!("======================================================");

    assert!(output.status.success(), "Command should succeed");

    // Should find both methods
    assert!(stdout.contains("test_nested.py::TestNestedImport::test_package_base_method"));
    assert!(stdout.contains("test_nested.py::TestNestedImport::test_nested_method"));
    assert!(stdout.contains("collected 2 items"));
}

/// Test unittest inheritance
#[test]
fn test_unittest_inheritance() {
    let mut files = HashMap::new();

    files.insert(
        "test_unittest_inheritance.py",
        r#"import unittest

class TestWithStdlib(unittest.TestCase):
    def test_using_unittest(self):
        self.assertTrue(True)
"#,
    );

    let (_temp_dir, project_path) = create_test_project_with_files(files);

    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // Should successfully collect the test
    assert!(
        output.status.success(),
        "Command should succeed. Error: {stderr}"
    );

    let combined = format!("{stdout}{stderr}");
    assert!(combined.contains("test_unittest_inheritance.py::TestWithStdlib::test_using_unittest"));
    assert!(combined.contains("collected 1 item"));
}

/// Test module resolution with nested directory structure
#[test]
fn test_module_resolution_nested_directories() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().join("test_project");
    let subdir_path = project_path.join("tests").join("unit");

    std::fs::create_dir_all(&subdir_path).expect("Failed to create nested directories");

    // Create a base test module in project root
    let base_content = r#"class TestBase:
    def test_base_method(self):
        assert True
"#;
    std::fs::write(project_path.join("test_base.py"), base_content)
        .expect("Failed to write base module");

    // Create a test file in nested directory that imports from root
    let nested_content = r#"from test_base import TestBase

class TestNested(TestBase):
    def test_nested_method(self):
        assert True
"#;
    std::fs::write(subdir_path.join("test_nested.py"), nested_content)
        .expect("Failed to write nested test file");

    // Run collection from project root
    let output = Command::new(get_rtest_binary())
        .args(["--collect-only"])
        .current_dir(&project_path)
        .output()
        .expect("Failed to execute command");

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    // With session root resolution, the import should work
    assert!(
        output.status.success(),
        "Command failed with exit status: {:?}\nstdout: {}\nstderr: {}",
        output.status,
        stdout,
        stderr
    );

    // Should find both the base test and the nested test with inheritance
    assert!(stdout.contains("test_base.py::TestBase::test_base_method"));

    // Use platform-agnostic path construction
    let nested_test_path = std::path::Path::new("tests")
        .join("unit")
        .join("test_nested.py")
        .display()
        .to_string();
    assert!(stdout.contains(&format!(
        "{}::TestNested::test_base_method",
        nested_test_path
    )));
    assert!(stdout.contains(&format!(
        "{}::TestNested::test_nested_method",
        nested_test_path
    )));
}
