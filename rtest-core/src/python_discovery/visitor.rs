//! AST visitor for discovering tests in Python code.

use crate::python_discovery::{
    discovery::{TestDiscoveryConfig, TestInfo},
    import_tracker::ImportTracker,
    pattern,
};
use ruff_python_ast::{Expr, ExprAttribute, ExprCall, ExprName, ExprStringLiteral, ModModule, Stmt, StmtClassDef, StmtFunctionDef, StmtImport, StmtImportFrom};

/// Visitor to discover test functions and classes in Python AST
pub(crate) struct TestDiscoveryVisitor {
    config: TestDiscoveryConfig,
    tests: Vec<TestInfo>,
    current_class: Option<String>,
    import_tracker: ImportTracker,
}

impl TestDiscoveryVisitor {
    pub fn new(config: &TestDiscoveryConfig) -> Self {
        Self {
            config: config.clone(),
            tests: Vec::new(),
            current_class: None,
            import_tracker: ImportTracker::new(),
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
            Stmt::Import(import) => self.visit_import(import),
            Stmt::ImportFrom(import_from) => self.visit_import_from(import_from),
            _ => {}
        }
    }

    fn visit_function(&mut self, func: &StmtFunctionDef) {
        let name = func.name.as_str();
        if self.is_test_function(name) {
            let xdist_group = self.extract_xdist_group_from_decorators(&func.decorator_list);
            self.tests.push(TestInfo {
                name: name.into(),
                line: func.range.start().to_u32() as usize,
                is_method: self.current_class.is_some(),
                class_name: self.current_class.clone(),
                xdist_group,
            });
        }
    }

    fn visit_class(&mut self, class: &StmtClassDef) {
        let name = class.name.as_str();
        if self.is_test_class(name) && !self.class_has_init(class) {
            let prev_class = self.current_class.clone();
            self.current_class = Some(name.into());

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

    fn visit_import(&mut self, import: &StmtImport) {
        for alias in &import.names {
            match alias.name.id.as_str() {
                // import pytest
                "pytest" => {
                    let local_name = alias.asname.as_ref()
                        .map(|n| n.id.as_str())
                        .unwrap_or("pytest");
                    self.import_tracker.add_import(local_name, "pytest");
                }
                _ => {}
            }
        }
    }

    fn visit_import_from(&mut self, import_from: &StmtImportFrom) {
        if let Some(module) = &import_from.module {
            let module_str = module.id.as_str();
            
            for alias in &import_from.names {
                let imported_name = alias.name.id.as_str();
                let local_name = alias.asname.as_ref()
                    .map(|n| n.id.as_str())
                    .unwrap_or(imported_name);
                
                match (module_str, imported_name) {
                    // from pytest import mark
                    ("pytest", "mark") => {
                        self.import_tracker.add_import(local_name, "pytest.mark");
                    }
                    // from pytest import fixture, ...
                    ("pytest", _) => {
                        self.import_tracker.add_import(local_name, &format!("pytest.{}", imported_name));
                    }
                    // from pytest.mark import parametrize, ...
                    ("pytest.mark", _) => {
                        self.import_tracker.add_import(local_name, &format!("pytest.mark.{}", imported_name));
                    }
                    _ => {}
                }
            }
        }
    }

    fn extract_xdist_group_from_decorators(&self, decorators: &[ruff_python_ast::Decorator]) -> Option<String> {
        for decorator in decorators {
            if let Some(group_name) = self.parse_xdist_group_decorator(&decorator.expression) {
                return Some(group_name);
            }
        }
        None
    }

    fn parse_xdist_group_decorator(&self, expr: &Expr) -> Option<String> {
        match expr {
            // Handle @pytest.mark.xdist_group(name="group_name")
            Expr::Call(ExprCall { func, arguments, .. }) => {
                if self.is_xdist_group_call(func) {
                    // Look for name= keyword argument
                    for keyword in &arguments.keywords {
                        if let Some(arg_name) = &keyword.arg {
                            if arg_name.as_str() == "name" {
                                if let Expr::StringLiteral(ExprStringLiteral { value, .. }) = &keyword.value {
                                    return Some(value.to_string());
                                }
                            }
                        }
                    }
                    // Also check positional arguments
                    if let Some(first_arg) = arguments.args.first() {
                        if let Expr::StringLiteral(ExprStringLiteral { value, .. }) = first_arg {
                            return Some(value.to_string());
                        }
                    }
                }
            }
            // Handle @pytest.mark.xdist_group("group_name") - call form
            _ => {}
        }
        None
    }

    fn is_xdist_group_call(&self, func_expr: &Expr) -> bool {
        match func_expr {
            // Handle patterns like: pytest.mark.xdist_group, pt.mark.xdist_group, mark.xdist_group
            Expr::Attribute(ExprAttribute { attr, value, .. }) => {
                if attr.as_str() == "xdist_group" {
                    return self.is_pytest_mark_expr(value);
                }
            }
            _ => {}
        }
        false
    }

    fn is_pytest_mark_expr(&self, expr: &Expr) -> bool {
        match expr {
            // Handle mark.* when mark is imported from pytest
            Expr::Name(ExprName { id, .. }) => {
                self.import_tracker.is_pytest_mark(id.as_str())
            }
            // Handle pytest.mark.* or alias.mark.*
            Expr::Attribute(ExprAttribute { attr, value, .. }) => {
                if attr.as_str() == "mark" {
                    if let Expr::Name(ExprName { id, .. }) = &**value {
                        return self.import_tracker.is_pytest(id.as_str());
                    }
                }
                false
            }
            _ => false
        }
    }
}
