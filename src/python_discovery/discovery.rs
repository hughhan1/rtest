//! Test discovery types and main entry point.

use crate::collection::error::{CollectionError, CollectionResult, CollectionWarning};
use crate::collection::nodes::Function;
use crate::collection::types::Location;
use crate::python_discovery::cases::CasesExpansion;
use crate::python_discovery::module_resolver::ModuleResolver;
use crate::python_discovery::semantic_analyzer::SemanticTestDiscovery;
use crate::python_discovery::visitor::TestDiscoveryVisitor;
use ruff_python_ast::Mod;
use ruff_python_parser::{parse, Mode, ParseOptions};
use std::path::Path;

/// Information about a discovered test
#[derive(Debug, Clone)]
pub struct TestInfo {
    pub name: String,
    pub line: usize,
    #[allow(dead_code)]
    pub is_method: bool,
    pub class_name: Option<String>,
    /// Test cases expansion result from parsing decorators.
    pub cases_expansion: CasesExpansion,
}

/// Configuration for test discovery
#[derive(Debug, Clone)]
pub struct TestDiscoveryConfig {
    pub python_classes: Vec<String>,
    pub python_functions: Vec<String>,
}

impl Default for TestDiscoveryConfig {
    fn default() -> Self {
        Self {
            python_classes: vec!["Test*".into()],
            python_functions: vec!["test*".into()],
        }
    }
}

/// Parse a Python file and discover test functions/methods
pub fn discover_tests(
    path: &Path,
    source: &str,
    config: &TestDiscoveryConfig,
) -> CollectionResult<Vec<TestInfo>> {
    let parsed = parse(source, ParseOptions::from(Mode::Module)).map_err(|e| {
        CollectionError::ParseError(format!("Failed to parse {}: {:?}", path.display(), e))
    })?;

    let mut visitor = TestDiscoveryVisitor::new(config);
    let module = parsed.into_syntax();
    if let Mod::Module(module) = module {
        visitor.visit_module(&module);
    }

    Ok(visitor.into_tests())
}

/// Discover tests with cross-module inheritance support
pub fn discover_tests_with_inheritance(
    path: &Path,
    source: &str,
    config: &TestDiscoveryConfig,
    root_path: &Path,
) -> CollectionResult<(Vec<TestInfo>, Vec<CollectionWarning>)> {
    let module_path = path_to_module_path(path, root_path);
    let mut module_resolver = ModuleResolver::new(root_path)?;
    let mut discovery = SemanticTestDiscovery::new(config.clone());

    discovery.discover_tests(path, source, module_path, &mut module_resolver)
}

/// Convert a file path to a module path
fn path_to_module_path(file_path: &Path, root_path: &Path) -> Vec<String> {
    let relative = file_path.strip_prefix(root_path).unwrap_or(file_path);
    let last_component = relative.components().next_back();

    let mut parts = Vec::new();

    for component in relative.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();

            // Strip .py extension from the last component
            if name_str.ends_with(".py") && Some(component) == last_component {
                let without_ext = name_str.strip_suffix(".py").unwrap();
                if without_ext != "__init__" {
                    parts.push(without_ext.to_string());
                }
            } else {
                parts.push(name_str.to_string());
            }
        }
    }

    parts
}

/// Convert TestInfo to one or more Function collectors.
///
/// Returns multiple Functions if the test has expanded cases (e.g., `test_foo[0]`, `test_foo[1]`).
/// Returns a single Function if not decorated or if cases cannot be statically expanded.
pub fn test_info_to_functions(
    test: &TestInfo,
    module_path: &Path,
    module_nodeid: &str,
) -> Vec<Function> {
    let base_nodeid = if let Some(class_name) = &test.class_name {
        format!("{}::{}::{}", module_nodeid, class_name, test.name)
    } else {
        format!("{}::{}", module_nodeid, test.name)
    };

    match &test.cases_expansion {
        CasesExpansion::NotDecorated | CasesExpansion::CannotExpand(_) => {
            vec![Function {
                name: test.name.clone(),
                nodeid: base_nodeid,
                location: Location {
                    path: module_path.to_path_buf(),
                    line: Some(test.line),
                    name: test.name.clone(),
                },
            }]
        }
        CasesExpansion::Expanded(cases) => cases
            .iter()
            .map(|case| {
                let nodeid = format!("{}[{}]", base_nodeid, case.case_id);
                let name_with_case = format!("{}[{}]", test.name, case.case_id);
                Function {
                    name: name_with_case.clone(),
                    nodeid,
                    location: Location {
                        path: module_path.to_path_buf(),
                        line: Some(test.line),
                        name: name_with_case,
                    },
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use indoc::indoc;
    use std::path::PathBuf;

    #[test]
    fn test_discover_tests() {
        let source = indoc! {r#"
            def test_simple():
                pass

            def not_a_test():
                pass

            class TestClass:
                def test_method(self):
                    pass
                
                def not_a_test_method(self):
                    pass

            class NotATestClass:
                def test_ignored(self):
                    pass
        "#};

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        assert_eq!(tests.len(), 2);

        assert_eq!(tests[0].name, "test_simple");
        assert!(!tests[0].is_method);
        assert_eq!(tests[0].class_name, None);

        assert_eq!(tests[1].name, "test_method");
        assert!(tests[1].is_method);
        assert_eq!(tests[1].class_name, Some("TestClass".into()));
    }

    #[test]
    fn test_skip_classes_with_init() {
        let source = r#"
class TestWithInit:
    def __init__(self):
        pass
        
    def test_should_be_skipped(self):
        pass

class TestWithoutInit:
    def test_should_be_collected(self):
        pass
"#;

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].name, "test_should_be_collected");
        assert_eq!(tests[0].class_name, Some("TestWithoutInit".into()));
    }

    #[test]
    fn test_camel_case_functions() {
        let source = r#"
def test_snake_case():
    pass

def testCamelCase():
    pass

def testThisIsAlsoATest():
    pass

class TestClass:
    def test_method_snake_case(self):
        pass
    
    def testMethodCamelCase(self):
        pass

def not_a_test():
    pass
"#;

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        assert_eq!(tests.len(), 5);

        let test_names: Vec<&str> = tests.iter().map(|t| t.name.as_str()).collect();
        assert!(test_names.contains(&"test_snake_case"));
        assert!(test_names.contains(&"testCamelCase"));
        assert!(test_names.contains(&"testThisIsAlsoATest"));
        assert!(test_names.contains(&"test_method_snake_case"));
        assert!(test_names.contains(&"testMethodCamelCase"));
    }

    #[test]
    fn test_class_inheritance_same_module() {
        let source = r#"
class TestBase:
    def test_base_method(self):
        pass
    
    def test_another_base_method(self):
        pass

class TestDerived(TestBase):
    def test_derived_method(self):
        pass

class TestMultiLevel(TestDerived):
    def test_multi_level_method(self):
        pass
"#;

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        // Should collect:
        // - TestBase: test_base_method, test_another_base_method (2)
        // - TestDerived: test_base_method, test_another_base_method (inherited), test_derived_method (3)
        // - TestMultiLevel: test_derived_method (inherited), test_multi_level_method (2)
        // Total: 7 tests
        assert_eq!(tests.len(), 7);

        // Check that TestBase has its own methods
        let base_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.class_name.as_ref().is_some_and(|c| c == "TestBase"))
            .collect();
        assert_eq!(base_tests.len(), 2);

        // Check that TestDerived has both inherited and its own methods
        let derived_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.class_name.as_ref().is_some_and(|c| c == "TestDerived"))
            .collect();
        assert_eq!(derived_tests.len(), 3);

        let derived_method_names: Vec<&str> =
            derived_tests.iter().map(|t| t.name.as_str()).collect();
        assert!(derived_method_names.contains(&"test_base_method"));
        assert!(derived_method_names.contains(&"test_another_base_method"));
        assert!(derived_method_names.contains(&"test_derived_method"));

        // Check that TestMultiLevel has inherited and its own methods
        let multi_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.class_name.as_ref().is_some_and(|c| c == "TestMultiLevel"))
            .collect();
        assert_eq!(multi_tests.len(), 2);

        let multi_method_names: Vec<&str> = multi_tests.iter().map(|t| t.name.as_str()).collect();
        assert!(multi_method_names.contains(&"test_derived_method"));
        assert!(multi_method_names.contains(&"test_multi_level_method"));
    }

    #[test]
    fn test_inheritance_with_init_skipped() {
        let source = r#"
class TestBaseWithInit:
    def __init__(self):
        pass
        
    def test_should_not_be_collected(self):
        pass

class TestDerivedFromInitClass(TestBaseWithInit):
    def test_derived_method(self):
        pass
"#;

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        // Both classes should be skipped because base class has __init__
        assert_eq!(tests.len(), 0);
    }

    #[test]
    fn test_cross_module_inheritance() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temporary directory structure
        let temp_dir = TempDir::new().unwrap();
        let tests_dir = temp_dir.path().join("tests");
        fs::create_dir(&tests_dir).unwrap();

        // Create parent test module
        let parent_module = r#"
class TestBase:
    def test_base_method(self):
        pass
        
    def test_another_base_method(self):
        pass
"#;
        fs::write(tests_dir.join("test_base.py"), parent_module).unwrap();

        // Create child test module that imports from parent
        let child_module = r#"
from tests.test_base import TestBase

class TestDerived(TestBase):
    def test_derived_method(self):
        pass
"#;
        let child_path = tests_dir.join("test_child.py");
        fs::write(&child_path, child_module).unwrap();

        // Test with cross-module inheritance enabled
        let config = TestDiscoveryConfig::default();
        let (tests, _warnings) =
            discover_tests_with_inheritance(&child_path, child_module, &config, temp_dir.path())
                .unwrap();

        // Should find 3 tests: 2 inherited from TestBase + 1 from TestDerived
        assert_eq!(tests.len(), 3);

        let method_names: Vec<&str> = tests.iter().map(|t| t.name.as_str()).collect();
        assert!(method_names.contains(&"test_base_method"));
        assert!(method_names.contains(&"test_another_base_method"));
        assert!(method_names.contains(&"test_derived_method"));

        // All should be under TestDerived class
        assert!(tests
            .iter()
            .all(|t| t.class_name.as_ref().is_some_and(|c| c == "TestDerived")));
    }

    #[test]
    fn test_inherited_method_with_class_parametrize() {
        let source = r#"
import pytest

class TestParent:
    def test_inherited(self, x):
        pass

@pytest.mark.parametrize("x", [1, 2])
class TestChild(TestParent):
    pass
"#;

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        // Should have 2 TestInfo:
        // - TestParent::test_inherited (no parametrize)
        // - TestChild::test_inherited (with parametrize expansion)
        assert_eq!(tests.len(), 2, "Expected 2 TestInfo, got: {:?}", tests);

        // Check parent test (no expansion)
        let parent_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.class_name.as_ref().is_some_and(|c| c == "TestParent"))
            .collect();
        assert_eq!(parent_tests.len(), 1);
        assert!(matches!(
            parent_tests[0].cases_expansion,
            CasesExpansion::NotDecorated
        ));

        // Check child test (should have Expanded with 2 cases)
        let child_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.class_name.as_ref().is_some_and(|c| c == "TestChild"))
            .collect();
        assert_eq!(child_tests.len(), 1, "Child should have 1 TestInfo");

        // The expansion should produce [1] and [2]
        if let CasesExpansion::Expanded(cases) = &child_tests[0].cases_expansion {
            assert_eq!(cases.len(), 2, "Expected 2 cases in expansion");
            assert_eq!(cases[0].case_id, "1");
            assert_eq!(cases[1].case_id, "2");
        } else {
            panic!(
                "Expected child test to have Expanded cases, got: {:?}",
                child_tests[0].cases_expansion
            );
        }
    }

    #[test]
    fn test_stdlib_inheritance_graceful() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path().join("test_enum.py");

        let source = r#"
from enum import Enum

class TestStatus(Enum):
    A = 1

class TestReal:
    def test_works(self):
        pass
"#;
        fs::write(&test_path, source).unwrap();

        let config = TestDiscoveryConfig::default();
        let (tests, _warnings) =
            discover_tests_with_inheritance(&test_path, source, &config, temp_dir.path()).unwrap();

        // TestStatus(Enum) should be skipped gracefully, TestReal::test_works collected
        let test_names: Vec<&str> = tests.iter().map(|t| t.name.as_str()).collect();
        assert!(
            test_names.contains(&"test_works"),
            "Expected test_works, got: {:?}",
            test_names
        );
        assert_eq!(tests.len(), 1, "Expected only 1 test, got: {:?}", tests);
    }

    #[test]
    fn test_external_package_inheritance_graceful() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path().join("test_external.py");

        let source = r#"
from nonexistent_package import SomeBase

class TestDerived(SomeBase):
    def test_method(self):
        pass
"#;
        fs::write(&test_path, source).unwrap();

        let config = TestDiscoveryConfig::default();
        let (tests, _warnings) =
            discover_tests_with_inheritance(&test_path, source, &config, temp_dir.path()).unwrap();

        // Should not crash; TestDerived::test_method should still be collected
        let test_names: Vec<&str> = tests.iter().map(|t| t.name.as_str()).collect();
        assert!(
            test_names.contains(&"test_method"),
            "Expected test_method, got: {:?}",
            test_names
        );
        assert_eq!(tests.len(), 1, "Expected 1 test, got: {:?}", tests);
    }

    #[test]
    fn test_inherited_method_with_class_parametrize_semantic() {
        use std::fs;
        use tempfile::TempDir;

        // Create a temp directory
        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path().join("test_inherit.py");

        let source = r#"
import pytest

class TestParent:
    def test_inherited(self, x):
        pass

@pytest.mark.parametrize("x", [1, 2])
class TestChild(TestParent):
    pass
"#;
        fs::write(&test_path, source).unwrap();

        let config = TestDiscoveryConfig::default();
        let (tests, _warnings) =
            discover_tests_with_inheritance(&test_path, source, &config, temp_dir.path()).unwrap();

        // Should have 2 TestInfo:
        // - TestParent::test_inherited (no parametrize)
        // - TestChild::test_inherited (with parametrize expansion)
        assert_eq!(tests.len(), 2, "Expected 2 TestInfo, got: {:?}", tests);

        // Check parent test (no expansion)
        let parent_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.class_name.as_ref().is_some_and(|c| c == "TestParent"))
            .collect();
        assert_eq!(parent_tests.len(), 1);
        assert!(matches!(
            parent_tests[0].cases_expansion,
            CasesExpansion::NotDecorated
        ));

        // Check child test (should have Expanded with 2 cases)
        let child_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.class_name.as_ref().is_some_and(|c| c == "TestChild"))
            .collect();
        assert_eq!(child_tests.len(), 1, "Child should have 1 TestInfo");

        // The expansion should produce [1] and [2]
        if let CasesExpansion::Expanded(cases) = &child_tests[0].cases_expansion {
            assert_eq!(cases.len(), 2, "Expected 2 cases in expansion");
            assert_eq!(cases[0].case_id, "1");
            assert_eq!(cases[1].case_id, "2");
        } else {
            panic!(
                "Expected child test to have Expanded cases, got: {:?}",
                child_tests[0].cases_expansion
            );
        }
    }
}
