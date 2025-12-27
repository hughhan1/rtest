//! AST visitor for discovering tests in Python code.

use crate::python_discovery::{
    discovery::{TestDiscoveryConfig, TestInfo},
    pattern,
};
use ruff_python_ast::{Expr, ModModule, Stmt, StmtClassDef, StmtFunctionDef};
use std::collections::{HashMap, HashSet};

/// Visitor to discover test functions and classes in Python AST
pub(crate) struct TestDiscoveryVisitor {
    config: TestDiscoveryConfig,
    tests: Vec<TestInfo>,
    current_class: Option<String>,
    /// Maps class names to (methods, has_init) for inheritance resolution
    class_methods: HashMap<String, (Vec<TestInfo>, bool)>,
}

impl TestDiscoveryVisitor {
    pub fn new(config: &TestDiscoveryConfig) -> Self {
        Self {
            config: config.clone(),
            tests: Vec::new(),
            current_class: None,
            class_methods: HashMap::new(),
        }
    }

    pub fn visit_module(&mut self, module: &ModModule) {
        // First pass: collect all test classes and their methods
        self.collect_class_methods(module);

        // Second pass: visit statements and handle inheritance
        for stmt in &module.body {
            self.visit_stmt(stmt);
        }
    }

    pub fn into_tests(self) -> Vec<TestInfo> {
        self.tests
    }

    fn collect_class_methods(&mut self, module: &ModModule) {
        // First, collect which classes have __init__
        let mut classes_with_init = HashSet::new();
        for stmt in &module.body {
            if let Stmt::ClassDef(class) = stmt {
                if self.class_has_init(class) {
                    classes_with_init.insert(class.name.as_str());
                }
            }
        }

        // Then collect methods, storing them even for classes with __init__
        // (for inheritance checking)
        for stmt in &module.body {
            if let Stmt::ClassDef(class) = stmt {
                let name = class.name.as_str();
                if self.is_test_class(name) {
                    let mut methods = Vec::new();

                    // Collect all test methods in this class
                    for stmt in &class.body {
                        if let Stmt::FunctionDef(func) = stmt {
                            let method_name = func.name.as_str();
                            if self.is_test_function(method_name) {
                                methods.push(TestInfo {
                                    name: method_name.into(),
                                    line: func.range.start().to_u32() as usize,
                                    is_method: true,
                                    class_name: Some(name.into()),
                                });
                            }
                        }
                    }

                    self.class_methods
                        .insert(name.into(), (methods, classes_with_init.contains(name)));
                }
            }
        }
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
                name: name.into(),
                line: func.range.start().to_u32() as usize,
                is_method: self.current_class.is_some(),
                class_name: self.current_class.clone(),
            });
        }
    }

    fn visit_class(&mut self, class: &StmtClassDef) {
        let name = class.name.as_str();
        if self.is_test_class(name) {
            // Check if this class or any of its parents have __init__
            let mut should_skip = self.class_has_init(class);

            // Check parent classes for __init__
            if !should_skip {
                if let Some(arguments) = &class.arguments {
                    for base_expr in arguments.args.iter() {
                        if let Expr::Name(base_name) = base_expr {
                            let base_class_name = base_name.id.as_str();
                            if let Some((_, parent_has_init)) =
                                self.class_methods.get(base_class_name)
                            {
                                if *parent_has_init {
                                    should_skip = true;
                                    break;
                                }
                            }
                        }
                    }
                }
            }

            if !should_skip {
                let prev_class = self.current_class.clone();
                self.current_class = Some(name.into());

                // First, collect inherited methods from base classes
                if let Some(arguments) = &class.arguments {
                    for base_expr in arguments.args.iter() {
                        if let Expr::Name(base_name) = base_expr {
                            let base_class_name = base_name.id.as_str();

                            // If the base class is a test class in the same module,
                            // inherit its methods
                            if let Some((parent_methods, _)) =
                                self.class_methods.get(base_class_name)
                            {
                                for parent_method in parent_methods {
                                    // Create a copy of the parent method but with the child class name
                                    self.tests.push(TestInfo {
                                        name: parent_method.name.clone(),
                                        line: parent_method.line,
                                        is_method: true,
                                        class_name: Some(name.into()),
                                    });
                                }
                            }
                        }
                    }
                }

                // Then visit methods defined directly in this class
                for stmt in &class.body {
                    self.visit_stmt(stmt);
                }

                self.current_class = prev_class;
            }
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
