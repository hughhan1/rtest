//! Test discovery types and main entry point.

use crate::collection::error::{CollectionError, CollectionResult, CollectionWarning};
use crate::collection::nodes::Function;
use crate::collection::types::Location;
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
    /// Whether this test originated from a parametrized decorator
    pub is_parametrized: bool,
    /// Whether this test has parametrize values with uncertain formatting
    /// (e.g., attribute accesses like Enum.VALUE where pytest's ID formatting varies)
    pub has_uncertain_params: bool,
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

    let mut parts = Vec::new();

    for component in relative.components() {
        if let std::path::Component::Normal(name) = component {
            let name_str = name.to_string_lossy();

            // Strip .py extension from the last component
            if name_str.ends_with(".py") && component == relative.components().last().unwrap() {
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

/// Convert TestInfo to Function collector
pub fn test_info_to_function(test: &TestInfo, module_path: &Path, module_nodeid: &str) -> Function {
    let nodeid = if let Some(class_name) = &test.class_name {
        format!("{}::{}::{}", module_nodeid, class_name, test.name)
    } else {
        format!("{}::{}", module_nodeid, test.name)
    };

    Function {
        name: test.name.clone(),
        nodeid,
        location: Location {
            path: module_path.to_path_buf(),
            line: Some(test.line),
            name: test.name.clone(),
        },
        is_parametrized_flag: test.is_parametrized,
        has_uncertain_params: test.has_uncertain_params,
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
            .filter(|t| t.class_name.as_ref().map_or(false, |c| c == "TestBase"))
            .collect();
        assert_eq!(base_tests.len(), 2);

        // Check that TestDerived has both inherited and its own methods
        let derived_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.class_name.as_ref().map_or(false, |c| c == "TestDerived"))
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
            .filter(|t| {
                t.class_name
                    .as_ref()
                    .map_or(false, |c| c == "TestMultiLevel")
            })
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
            .all(|t| t.class_name.as_ref().map_or(false, |c| c == "TestDerived")));
    }

    #[test]
    fn test_parametrize_basic_types() {
        let source = indoc! {r#"
            import pytest

            @pytest.mark.parametrize("value", [1, 2, 3])
            def test_with_ints(value):
                pass

            @pytest.mark.parametrize("name", ["alice", "bob", "charlie"])
            def test_with_strings(name):
                pass

            @pytest.mark.parametrize("flag", [True, False])
            def test_with_bools(flag):
                pass

            @pytest.mark.parametrize("value", [None, "something"])
            def test_with_none(value):
                pass
        "#};

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        // Should create 3 + 3 + 2 + 2 = 10 test items
        assert_eq!(tests.len(), 10);

        // Check int parametrization
        let int_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.name.starts_with("test_with_ints"))
            .collect();
        assert_eq!(int_tests.len(), 3);
        assert!(int_tests.iter().any(|t| t.name == "test_with_ints[1]"));
        assert!(int_tests.iter().any(|t| t.name == "test_with_ints[2]"));
        assert!(int_tests.iter().any(|t| t.name == "test_with_ints[3]"));

        // Check string parametrization
        let string_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.name.starts_with("test_with_strings"))
            .collect();
        assert_eq!(string_tests.len(), 3);
        assert!(string_tests
            .iter()
            .any(|t| t.name == "test_with_strings[alice]"));
        assert!(string_tests
            .iter()
            .any(|t| t.name == "test_with_strings[bob]"));
        assert!(string_tests
            .iter()
            .any(|t| t.name == "test_with_strings[charlie]"));

        // Check bool parametrization
        let bool_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.name.starts_with("test_with_bools"))
            .collect();
        assert_eq!(bool_tests.len(), 2);
        assert!(bool_tests
            .iter()
            .any(|t| t.name == "test_with_bools[True]"));
        assert!(bool_tests
            .iter()
            .any(|t| t.name == "test_with_bools[False]"));

        // Check None parametrization
        let none_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.name.starts_with("test_with_none"))
            .collect();
        assert_eq!(none_tests.len(), 2);
        assert!(none_tests
            .iter()
            .any(|t| t.name == "test_with_none[None]"));
        assert!(none_tests
            .iter()
            .any(|t| t.name == "test_with_none[something]"));
    }

    #[test]
    fn test_parametrize_function_calls() {
        let source = indoc! {r#"
            import pytest
            from decimal import Decimal

            @pytest.mark.parametrize("a,b,expected", [
                (Decimal(20), Decimal(20), True),
                (Decimal(20), Decimal(40), False),
                (Decimal(40), Decimal(20), False),
            ])
            def test_with_decimal(a, b, expected):
                pass
        "#};

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        assert_eq!(tests.len(), 3);

        // Check that Decimal calls generate auto-IDs (like pytest does)
        assert!(tests
            .iter()
            .any(|t| t.name == "test_with_decimal[a0-b0-True]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_with_decimal[a1-b1-False]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_with_decimal[a2-b2-False]"));

        // Make sure we don't have ugly AST debug output
        assert!(!tests.iter().any(|t| t.name.contains("Call(ExprCall")));
        // And we don't show "Decimal(20)" - we use auto-IDs like pytest
        assert!(!tests.iter().any(|t| t.name.contains("Decimal(")));
    }

    #[test]
    fn test_parametrize_multiple_params() {
        let source = indoc! {r#"
            import pytest

            @pytest.mark.parametrize("x,y,expected", [
                (1, 2, 3),
                (10, 20, 30),
                (-5, 5, 0),
            ])
            def test_addition(x, y, expected):
                pass
        "#};

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        assert_eq!(tests.len(), 3);

        // Check multi-parameter formatting
        assert!(tests
            .iter()
            .any(|t| t.name == "test_addition[1-2-3]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_addition[10-20-30]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_addition[-5-5-0]"));
    }

    #[test]
    fn test_parametrize_stacked_decorators() {
        let source = indoc! {r#"
            import pytest

            @pytest.mark.parametrize("x", [1, 2])
            @pytest.mark.parametrize("y", [10, 20])
            def test_stacked(x, y):
                pass
        "#};

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        // Should create 2 * 2 = 4 test items
        assert_eq!(tests.len(), 4);

        // Check all combinations exist
        assert!(tests
            .iter()
            .any(|t| t.name == "test_stacked[10-1]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_stacked[10-2]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_stacked[20-1]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_stacked[20-2]"));
    }

    #[test]
    fn test_parametrize_on_class_methods() {
        let source = indoc! {r#"
            import pytest

            class TestMath:
                @pytest.mark.parametrize("value", [1, 2, 3])
                def test_positive(self, value):
                    pass

                @pytest.mark.parametrize("value", [-1, -2, -3])
                def test_negative(self, value):
                    pass
        "#};

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        assert_eq!(tests.len(), 6);

        // Check positive tests
        let positive_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.name.starts_with("test_positive"))
            .collect();
        assert_eq!(positive_tests.len(), 3);
        assert!(positive_tests
            .iter()
            .all(|t| t.class_name.as_ref() == Some(&"TestMath".to_string())));

        // Check negative tests
        let negative_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.name.starts_with("test_negative"))
            .collect();
        assert_eq!(negative_tests.len(), 3);
        assert!(negative_tests
            .iter()
            .any(|t| t.name == "test_negative[-1]"));
        assert!(negative_tests
            .iter()
            .any(|t| t.name == "test_negative[-2]"));
        assert!(negative_tests
            .iter()
            .any(|t| t.name == "test_negative[-3]"));
    }

    #[test]
    fn test_parametrize_with_complex_types() {
        let source = indoc! {r#"
            import pytest

            @pytest.mark.parametrize("data", [
                [1, 2, 3],
                (4, 5, 6),
            ])
            def test_with_collections(data):
                pass
        "#};

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        assert_eq!(tests.len(), 2);

        // Lists and tuples are complex types, so they get auto-IDs
        assert!(tests
            .iter()
            .any(|t| t.name == "test_with_collections[data0]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_with_collections[data1]"));
    }

    #[test]
    fn test_parametrize_mixed_with_regular_tests() {
        let source = indoc! {r#"
            import pytest

            def test_regular():
                pass

            @pytest.mark.parametrize("value", [1, 2])
            def test_parametrized(value):
                pass

            class TestClass:
                def test_regular_method(self):
                    pass

                @pytest.mark.parametrize("value", [10, 20])
                def test_parametrized_method(self, value):
                    pass
        "#};

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        // Should have: 1 regular + 2 parametrized + 1 regular method + 2 parametrized methods = 6
        assert_eq!(tests.len(), 6);

        // Check we have both regular and parametrized tests
        assert!(tests.iter().any(|t| t.name == "test_regular"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_parametrized[1]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_parametrized[2]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_regular_method"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_parametrized_method[10]"));
        assert!(tests
            .iter()
            .any(|t| t.name == "test_parametrized_method[20]"));
    }

    #[test]
    fn test_parametrize_inherited_methods() {
        let source = indoc! {r#"
            import pytest

            class TestBase:
                @pytest.mark.parametrize("value", [1, 2])
                def test_base_parametrized(self, value):
                    pass

            class TestDerived(TestBase):
                @pytest.mark.parametrize("value", [10, 20])
                def test_derived_parametrized(self, value):
                    pass
        "#};

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        // TestBase: 2 tests
        // TestDerived: 2 inherited + 2 own = 4 tests
        // Total: 6 tests
        assert_eq!(tests.len(), 6);

        // Check base class tests
        let base_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.class_name.as_ref() == Some(&"TestBase".to_string()))
            .collect();
        assert_eq!(base_tests.len(), 2);

        // Check derived class tests (should have both inherited and own)
        let derived_tests: Vec<&TestInfo> = tests
            .iter()
            .filter(|t| t.class_name.as_ref() == Some(&"TestDerived".to_string()))
            .collect();
        assert_eq!(derived_tests.len(), 4);

        // Verify inherited parametrized tests
        assert!(derived_tests
            .iter()
            .any(|t| t.name == "test_base_parametrized[1]"));
        assert!(derived_tests
            .iter()
            .any(|t| t.name == "test_base_parametrized[2]"));

        // Verify own parametrized tests
        assert!(derived_tests
            .iter()
            .any(|t| t.name == "test_derived_parametrized[10]"));
        assert!(derived_tests
            .iter()
            .any(|t| t.name == "test_derived_parametrized[20]"));
    }
}
