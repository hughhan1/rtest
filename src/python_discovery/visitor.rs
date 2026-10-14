//! AST visitor for discovering tests in Python code.

use crate::python_discovery::{
    cases::{
        combine_and_expand_specs, parse_decorators_for_cases, parse_decorators_to_specs,
        MethodCasesInfo,
    },
    constant_resolver::ConstantResolver,
    discovery::{class_has_init, TestDiscoveryConfig, TestInfo},
};
use ruff_python_ast::{Expr, ModModule, Stmt, StmtClassDef, StmtFunctionDef};
use std::collections::{HashMap, HashSet};

/// Cached information about a test method for inheritance resolution.
///
/// Stores method-level specs (not expanded) so they can be combined with
/// child class decorators during inheritance.
#[derive(Clone)]
struct CachedMethodInfo {
    name: String,
    line: usize,
    /// Method-level decorator specs (not yet combined with class decorators)
    method_specs: MethodCasesInfo,
}

/// Cached information about a test class.
struct CachedClassInfo {
    methods: Vec<CachedMethodInfo>,
    has_init: bool,
}

/// Visitor to discover test functions and classes in Python AST
pub(crate) struct TestDiscoveryVisitor {
    config: TestDiscoveryConfig,
    tests: Vec<TestInfo>,
    /// Maps class names to cached class info for inheritance resolution
    class_cache: HashMap<String, CachedClassInfo>,
}

impl TestDiscoveryVisitor {
    pub fn new(config: &TestDiscoveryConfig) -> Self {
        Self {
            config: config.clone(),
            tests: Vec::new(),
            class_cache: HashMap::new(),
        }
    }

    pub fn visit_module(&mut self, module: &ModModule) {
        // Build constant resolver for this module
        let resolver = ConstantResolver::from_module(module);

        // First pass: collect all test classes and their methods (specs only, not expanded)
        self.collect_class_info(module, &resolver);

        // Second pass: visit statements and handle inheritance
        for stmt in &module.body {
            self.visit_stmt(stmt, &resolver);
        }
    }

    pub fn into_tests(self) -> Vec<TestInfo> {
        self.tests
    }

    /// First pass: collect class info with method-level specs for inheritance resolution
    fn collect_class_info(&mut self, module: &ModModule, resolver: &ConstantResolver) {
        for stmt in &module.body {
            if let Stmt::ClassDef(class) = stmt {
                let name = class.name.as_str();
                if self.config.is_test_class(name) {
                    let has_init = class_has_init(class);

                    let methods = self.collect_test_methods(class, resolver);

                    self.class_cache
                        .insert(name.to_string(), CachedClassInfo { methods, has_init });
                }
            }
        }
    }

    fn collect_test_methods(
        &self,
        class: &StmtClassDef,
        resolver: &ConstantResolver,
    ) -> Vec<CachedMethodInfo> {
        let mut methods = Vec::new();
        for stmt in &class.body {
            if let Stmt::FunctionDef(func) = stmt {
                let name = func.name.as_str();
                if self.config.is_test_function(name) {
                    methods.push(CachedMethodInfo {
                        name: name.to_string(),
                        line: func.range.start().to_u32() as usize,
                        method_specs: parse_decorators_to_specs(
                            &func.decorator_list,
                            Some(resolver),
                        ),
                    });
                }
            }
        }
        methods
    }

    fn visit_stmt(&mut self, stmt: &Stmt, resolver: &ConstantResolver) {
        match stmt {
            Stmt::FunctionDef(func) => self.visit_function(func, resolver),
            Stmt::ClassDef(class) => self.visit_class(class, resolver),
            _ => {}
        }
    }

    fn visit_function(&mut self, func: &StmtFunctionDef, resolver: &ConstantResolver) {
        let name = func.name.as_str();
        if self.config.is_test_function(name) {
            let cases_expansion = parse_decorators_for_cases(&func.decorator_list, Some(resolver));
            self.tests.push(TestInfo {
                name: name.into(),
                line: func.range.start().to_u32() as usize,
                is_method: false,
                class_name: None,
                cases_expansion,
            });
        }
    }

    fn visit_class(&mut self, class: &StmtClassDef, resolver: &ConstantResolver) {
        let class_name = class.name.as_str();
        if !self.config.is_test_class(class_name) {
            return;
        }

        // Check if this class should be skipped (has __init__ or parent has __init__)
        if self.should_skip_class(class) {
            return;
        }

        // Parse class-level decorators once
        let class_specs = parse_decorators_to_specs(&class.decorator_list, Some(resolver));

        let own_methods = self.collect_test_methods(class, resolver);
        let own_method_names: HashSet<&str> = own_methods
            .iter()
            .map(|method| method.name.as_str())
            .collect();

        // First, collect inherited methods from base classes
        if let Some(arguments) = &class.arguments {
            for base_expr in arguments.args.iter() {
                if let Expr::Name(base_name) = base_expr {
                    let base_class_name = base_name.id.as_str();

                    if let Some(parent_info) = self.class_cache.get(base_class_name) {
                        for parent_method in &parent_info.methods {
                            // Skip if this method is overridden in the child class
                            if own_method_names.contains(parent_method.name.as_str()) {
                                continue;
                            }

                            // Combine CHILD class specs with PARENT method specs
                            // This is the key fix: inherited methods get the child's class decorators
                            let cases_expansion =
                                combine_and_expand_specs(&class_specs, &parent_method.method_specs);

                            self.tests.push(TestInfo {
                                name: parent_method.name.clone(),
                                line: parent_method.line,
                                is_method: true,
                                class_name: Some(class_name.into()),
                                cases_expansion,
                            });
                        }
                    }
                }
            }
        }

        for method in own_methods {
            let cases_expansion = combine_and_expand_specs(&class_specs, &method.method_specs);
            self.tests.push(TestInfo {
                name: method.name,
                line: method.line,
                is_method: true,
                class_name: Some(class_name.into()),
                cases_expansion,
            });
        }
    }

    /// Check if a class should be skipped (has __init__ or inherits from class with __init__)
    fn should_skip_class(&self, class: &StmtClassDef) -> bool {
        if class_has_init(class) {
            return true;
        }

        // Check parent classes for __init__
        if let Some(arguments) = &class.arguments {
            for base_expr in arguments.args.iter() {
                if let Expr::Name(base_name) = base_expr {
                    let base_class_name = base_name.id.as_str();
                    if let Some(parent_info) = self.class_cache.get(base_class_name) {
                        if parent_info.has_init {
                            return true;
                        }
                    }
                }
            }
        }

        false
    }
}
