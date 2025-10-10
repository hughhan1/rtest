//! AST visitor for discovering tests in Python code.

use crate::python_discovery::{
    discovery::{TestDiscoveryConfig, TestInfo},
    pattern,
};
use ruff_python_ast::{
    Expr, ExprAttribute, ExprCall, ModModule, Stmt, StmtClassDef, StmtFunctionDef,
};
use std::collections::{HashMap, HashSet};

/// Visitor to discover test functions and classes in Python AST
pub(crate) struct TestDiscoveryVisitor {
    config: TestDiscoveryConfig,
    tests: Vec<TestInfo>,
    current_class: Option<String>,
    /// Maps class names to (methods, has_init) for inheritance resolution
    class_methods: HashMap<String, (Vec<TestInfo>, bool)>,
}

/// Information about a single parametrize decorator
#[derive(Debug, Clone)]
struct ParametrizeInfo {
    /// Parameter names (e.g., "x,y" or "value")
    #[allow(dead_code)]
    param_names: Vec<String>,
    /// List of parameter value tuples
    values: Vec<Vec<String>>,
}

/// Extract parametrize decorators from a function
fn extract_parametrize_decorators(func: &StmtFunctionDef) -> Vec<ParametrizeInfo> {
    let mut parametrize_infos = Vec::new();

    for decorator in &func.decorator_list {
        if let Some(info) = parse_parametrize_decorator(&decorator.expression) {
            parametrize_infos.push(info);
        }
    }

    parametrize_infos
}

/// Parse a single decorator expression to see if it's pytest.mark.parametrize
fn parse_parametrize_decorator(expr: &Expr) -> Option<ParametrizeInfo> {
    // Check if this is a Call expression
    if let Expr::Call(ExprCall {
        func, arguments, ..
    }) = expr
    {
        // Check if it's pytest.mark.parametrize
        if is_parametrize_call(func) {
            // Extract arguments: first arg is param names, second is values
            if arguments.args.len() >= 2 {
                let param_names = extract_param_names(&arguments.args[0])?;
                let values = extract_param_values(&arguments.args[1])?;
                return Some(ParametrizeInfo {
                    param_names,
                    values,
                });
            }
        }
    }
    None
}

/// Check if an expression is pytest.mark.parametrize
fn is_parametrize_call(expr: &Expr) -> bool {
    // Looking for: pytest.mark.parametrize
    if let Expr::Attribute(ExprAttribute { value, attr, .. }) = expr {
        if attr.as_str() == "parametrize" {
            if let Expr::Attribute(ExprAttribute {
                value: inner_value,
                attr: inner_attr,
                ..
            }) = value.as_ref()
            {
                if inner_attr.as_str() == "mark" {
                    if let Expr::Name(name) = inner_value.as_ref() {
                        return name.id.as_str() == "pytest";
                    }
                }
            }
        }
    }
    false
}

/// Extract parameter names from the first argument
fn extract_param_names(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::StringLiteral(s) => {
            let param_str = s.value.to_str();
            // Split by comma and trim whitespace
            Some(param_str.split(',').map(|p| p.trim().to_string()).collect())
        }
        _ => None,
    }
}

/// Extract parameter values from the second argument (list of values)
fn extract_param_values(expr: &Expr) -> Option<Vec<Vec<String>>> {
    match expr {
        Expr::List(list) => {
            let mut all_values = Vec::new();
            for elem in &list.elts {
                // Check if element is a tuple (multiple params) or single value
                match elem {
                    Expr::Tuple(tuple) => {
                        // Multiple parameters: convert tuple elements to strings
                        let param_values: Vec<String> =
                            tuple.elts.iter().map(format_param_value).collect();
                        all_values.push(param_values);
                    }
                    _ => {
                        // Single parameter: wrap in vec
                        let formatted = format_param_value(elem);
                        all_values.push(vec![formatted]);
                    }
                }
            }
            Some(all_values)
        }
        _ => None,
    }
}

/// Format a parameter value for display in test name
fn format_param_value(expr: &Expr) -> String {
    match expr {
        Expr::NumberLiteral(num) => {
            // Handle int, float, and complex numbers
            match &num.value {
                ruff_python_ast::Number::Int(i) => i.to_string(),
                ruff_python_ast::Number::Float(f) => {
                    // Format float, removing unnecessary decimals
                    if f.fract() == 0.0 {
                        format!("{:.0}", f)
                    } else {
                        f.to_string()
                    }
                }
                ruff_python_ast::Number::Complex { real, imag } => {
                    format!("{}+{}j", real, imag)
                }
            }
        }
        Expr::StringLiteral(s) => {
            // Return the string value without quotes if it's simple
            s.value.to_str().to_string()
        }
        Expr::BooleanLiteral(b) => {
            if b.value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Expr::NoneLiteral(_) => "None".to_string(),
        Expr::Tuple(tuple) => {
            // Format tuple elements separated by hyphens
            let elements: Vec<String> = tuple.elts.iter().map(format_param_value).collect();
            elements.join("-")
        }
        Expr::List(list) => {
            // Format list similar to tuple
            let elements: Vec<String> = list.elts.iter().map(format_param_value).collect();
            format!("[{}]", elements.join("-"))
        }
        Expr::UnaryOp(unary) => {
            // Handle negative numbers
            if let ruff_python_ast::UnaryOp::USub = unary.op {
                format!("-{}", format_param_value(&unary.operand))
            } else {
                format!("{:?}", expr)
            }
        }
        _ => {
            // For complex expressions, use a simple representation
            format!("{:?}", expr).chars().take(20).collect()
        }
    }
}

/// Generate all parameter combinations from multiple parametrize decorators
fn generate_param_combinations(parametrize_infos: &[ParametrizeInfo]) -> Vec<String> {
    if parametrize_infos.is_empty() {
        return vec![];
    }

    // Start with the first parametrize decorator
    let mut combinations = parametrize_infos[0]
        .values
        .iter()
        .map(|v| v.join("-"))
        .collect::<Vec<_>>();

    // For stacked decorators, create cartesian product
    for info in parametrize_infos.iter().skip(1) {
        let mut new_combinations = Vec::new();
        for existing in &combinations {
            for value_set in &info.values {
                let new_combo = format!("{}-{}", value_set.join("-"), existing);
                new_combinations.push(new_combo);
            }
        }
        combinations = new_combinations;
    }

    combinations
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
                                // Check for parametrize decorators
                                let parametrize_infos = extract_parametrize_decorators(func);

                                if parametrize_infos.is_empty() {
                                    methods.push(TestInfo {
                                        name: method_name.into(),
                                        line: func.range.start().to_u32() as usize,
                                        is_method: true,
                                        class_name: Some(name.into()),
                                    });
                                } else {
                                    // Generate test items for each parameter combination
                                    let combinations =
                                        generate_param_combinations(&parametrize_infos);
                                    for combo in combinations {
                                        let parametrized_name =
                                            format!("{}[{}]", method_name, combo);
                                        methods.push(TestInfo {
                                            name: parametrized_name,
                                            line: func.range.start().to_u32() as usize,
                                            is_method: true,
                                            class_name: Some(name.into()),
                                        });
                                    }
                                }
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
