//! Test discovery types and main entry point.

use crate::collection::{CollectionError, CollectionResult, Function, Location};
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
            python_classes: vec!["Test*".to_string()],
            python_functions: vec!["test*".to_string()],
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_discover_tests() {
        let source = r#"
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
"#;

        let config = TestDiscoveryConfig::default();
        let tests = discover_tests(&PathBuf::from("test.py"), source, &config).unwrap();

        assert_eq!(tests.len(), 2);

        assert_eq!(tests[0].name, "test_simple");
        assert!(!tests[0].is_method);
        assert_eq!(tests[0].class_name, None);

        assert_eq!(tests[1].name, "test_method");
        assert!(tests[1].is_method);
        assert_eq!(tests[1].class_name, Some("TestClass".to_string()));
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
        assert_eq!(tests[0].class_name, Some("TestWithoutInit".to_string()));
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
}
