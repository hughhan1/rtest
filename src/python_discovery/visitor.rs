//! AST visitor for discovering tests in Python code.

use crate::python_discovery::{
    cases::{combine_and_expand_specs, parse_decorators_for_cases, parse_decorators_to_specs},
    constant_resolver::ConstantResolver,
    discovery::{class_has_init, collect_test_methods, TestDiscoveryConfig, TestInfo},
    semantic_analyzer::TestMethodInfo,
};
use ruff_python_ast::{Expr, ModModule, Stmt, StmtClassDef, StmtFunctionDef};
use std::collections::{HashMap, HashSet};

/// Cached information about a test class.
struct CachedClassInfo {
    methods: Vec<TestMethodInfo>,
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

                    let methods = collect_test_methods(class, &self.config, resolver);

                    self.class_cache
                        .insert(name.to_string(), CachedClassInfo { methods, has_init });
                }
            }
        }
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

        let own_methods = collect_test_methods(class, &self.config, resolver);
        let own_method_names: HashSet<&str> = own_methods
            .iter()
            .map(|method| method.name.as_str())
            .collect();

        let inherited_methods: Vec<_> = self
            .base_classes(class)
            .flat_map(|parent| &parent.methods)
            .filter(|method| !own_method_names.contains(method.name.as_str()))
            .cloned()
            .collect();

        for method in inherited_methods.into_iter().chain(own_methods) {
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
        class_has_init(class) || self.base_classes(class).any(|parent| parent.has_init)
    }

    fn base_classes<'a>(
        &'a self,
        class: &'a StmtClassDef,
    ) -> impl Iterator<Item = &'a CachedClassInfo> {
        class
            .arguments
            .iter()
            .flat_map(|args| args.args.iter())
            .filter_map(|base| match base {
                Expr::Name(name) => self.class_cache.get(name.id.as_str()),
                _ => None,
            })
    }
}
