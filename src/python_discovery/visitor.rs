//! AST visitor for discovering tests in Python code.

use crate::python_discovery::{
    discovery::{TestDiscoveryConfig, TestInfo},
    pattern,
};
use ruff_python_ast::{ModModule, Stmt, StmtClassDef, StmtFunctionDef};

/// Visitor to discover test functions and classes in Python AST
pub(crate) struct TestDiscoveryVisitor {
    config: TestDiscoveryConfig,
    tests: Vec<TestInfo>,
    current_class: Option<String>,
}

impl TestDiscoveryVisitor {
    pub fn new(config: &TestDiscoveryConfig) -> Self {
        Self {
            config: config.clone(),
            tests: Vec::new(),
            current_class: None,
        }
    }

    pub fn visit_module(&mut self, module: &ModModule) {
        for stmt in &module.body {
            self.visit_stmt(stmt);
        }
    }

    pub fn into_tests(self) -> Vec<TestInfo> {
        self.tests
    }

    fn visit_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::FunctionDef(func) => self.visit_function(func),
            Stmt::ClassDef(class) => self.visit_class(class),
            _ => {}
        }
    }

    fn visit_function(&mut self, func: &StmtFunctionDef) {
        let name = func.name.as_str();
        if self.is_test_function(name) {
            self.tests.push(TestInfo {
                name: name.to_string(),
                line: func.range.start().to_u32() as usize,
                is_method: self.current_class.is_some(),
                class_name: self.current_class.clone(),
            });
        }
    }

    fn visit_class(&mut self, class: &StmtClassDef) {
        let name = class.name.as_str();
        if self.is_test_class(name) && !self.class_has_init(class) {
            let prev_class = self.current_class.clone();
            self.current_class = Some(name.to_string());

            // Visit methods in the class
            for stmt in &class.body {
                self.visit_stmt(stmt);
            }

            self.current_class = prev_class;
        }
    }

    fn is_test_function(&self, name: &str) -> bool {
        for pattern in &self.config.python_functions {
            if pattern::matches(pattern, name) {
                return true;
            }
        }
        false
    }

    fn is_test_class(&self, name: &str) -> bool {
        for pattern in &self.config.python_classes {
            if pattern::matches(pattern, name) {
                return true;
            }
        }
        false
    }

    fn class_has_init(&self, class: &StmtClassDef) -> bool {
        for stmt in &class.body {
            if let Stmt::FunctionDef(func) = stmt {
                if func.name.as_str() == "__init__" {
                    return true;
                }
            }
        }
        false
    }
}
