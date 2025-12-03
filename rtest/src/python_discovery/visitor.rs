//! AST visitor for discovering tests in Python code.

use crate::python_discovery::{
    discovery::{TestDiscoveryConfig, TestInfo},
    pattern,
};
use ruff_python_ast::{Expr, ExprAttribute, ExprCall, ModModule, Stmt, StmtClassDef, StmtFunctionDef};
use std::collections::{HashMap, HashSet};

/// Visitor to discover test functions and classes in Python AST
pub(crate) struct TestDiscoveryVisitor {
    config: TestDiscoveryConfig,
    tests: Vec<TestInfo>,
    current_class: Option<String>,
    /// Class-level parametrize decorators that apply to all methods
    current_class_parametrize: Vec<ParametrizeInfo>,
    /// Maps class names to (methods, has_init, class_parametrize) for inheritance resolution
    class_methods: HashMap<String, (Vec<TestInfo>, bool, Vec<ParametrizeInfo>)>,
}

/// Information about a single parametrize decorator
#[derive(Debug, Clone)]
struct ParametrizeInfo {
    /// Parameter names (e.g., ["a", "b", "threshold"])
    param_names: Vec<String>,
    /// List of parameter value tuples (formatted strings)
    values: Vec<Vec<ParamValue>>,
    /// Explicit IDs if provided via ids= parameter (e.g., ["case1", "case2"])
    explicit_ids: Option<Vec<String>>,
}

/// Represents a parameter value and whether it needs auto-ID generation
#[derive(Debug, Clone)]
enum ParamValue {
    /// Simple value that can be displayed as-is (e.g., "20", "True", "alice")
    Simple(String),
    /// Complex expression that needs auto-ID (e.g., Decimal(20) -> use "a0" instead)
    Complex,
    /// Attribute access (e.g., SalesLeadEventName.ARM_RESET) - we use the full path
    /// but pytest's behavior varies (Enums keep path, string constants don't), so
    /// files with these should be marked uncertain
    AttributeAccess(String),
}

/// Check if a function has any parametrize decorator (even if we can't parse values)
fn has_parametrize_decorator(func: &StmtFunctionDef) -> bool {
    for decorator in &func.decorator_list {
        if is_any_parametrize_call(&decorator.expression) {
            return true;
        }
    }
    false
}

/// Check if a class has any parametrize decorator
fn class_has_parametrize_decorator(class: &StmtClassDef) -> bool {
    for decorator in &class.decorator_list {
        if is_any_parametrize_call(&decorator.expression) {
            return true;
        }
    }
    false
}

/// Check if an expression is any form of parametrize call
/// This includes:
/// - pytest.mark.parametrize(...)
/// - parametrize(...) - custom wrappers
fn is_any_parametrize_call(expr: &Expr) -> bool {
    if let Expr::Call(ExprCall { func, .. }) = expr {
        // Check for pytest.mark.parametrize
        if is_parametrize_call(func) {
            return true;
        }
        // Check for bare parametrize(...) calls - custom helpers that wrap pytest
        if let Expr::Name(name) = func.as_ref() {
            if name.id.as_str() == "parametrize" {
                return true;
            }
        }
    }
    false
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

/// Extract parametrize decorators from a class definition
fn extract_class_parametrize_decorators(class: &StmtClassDef) -> Vec<ParametrizeInfo> {
    let mut parametrize_infos = Vec::new();

    for decorator in &class.decorator_list {
        if let Some(info) = parse_parametrize_decorator(&decorator.expression) {
            parametrize_infos.push(info);
        }
    }

    parametrize_infos
}

/// Parse a single decorator expression to see if it's pytest.mark.parametrize
fn parse_parametrize_decorator(expr: &Expr) -> Option<ParametrizeInfo> {
    // Check if this is a Call expression
    if let Expr::Call(ExprCall { func, arguments, .. }) = expr {
        // Check if it's pytest.mark.parametrize
        if is_parametrize_call(func) {
            // Extract arguments: first arg is param names, second is values
            if arguments.args.len() >= 2 {
                let param_names = extract_param_names(&arguments.args[0])?;
                let values = extract_param_values(&arguments.args[1], param_names.len())?;

                // Check for explicit ids= keyword argument
                let explicit_ids = extract_explicit_ids(arguments);

                return Some(ParametrizeInfo {
                    param_names,
                    values,
                    explicit_ids,
                });
            }
        }
    }
    None
}

/// Extract explicit IDs from the ids= keyword argument
fn extract_explicit_ids(arguments: &ruff_python_ast::Arguments) -> Option<Vec<String>> {
    for keyword in &arguments.keywords {
        if let Some(arg) = &keyword.arg {
            if arg.as_str() == "ids" {
                return extract_ids_list(&keyword.value);
            }
        }
    }
    None
}

/// Extract a list of string IDs from an expression
fn extract_ids_list(expr: &Expr) -> Option<Vec<String>> {
    match expr {
        Expr::List(list) => {
            let mut ids = Vec::new();
            for elem in &list.elts {
                if let Expr::StringLiteral(s) = elem {
                    ids.push(s.value.to_str().to_string());
                } else {
                    // If any ID is not a string literal, fall back to auto-generation
                    return None;
                }
            }
            Some(ids)
        }
        Expr::Tuple(tuple) => {
            let mut ids = Vec::new();
            for elem in &tuple.elts {
                if let Expr::StringLiteral(s) = elem {
                    ids.push(s.value.to_str().to_string());
                } else {
                    return None;
                }
            }
            Some(ids)
        }
        _ => None,
    }
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
            Some(
                param_str
                    .split(',')
                    .map(|p| p.trim().to_string())
                    .collect(),
            )
        }
        _ => None,
    }
}

/// Extract parameter values from the second argument (list of values)
fn extract_param_values(expr: &Expr, param_count: usize) -> Option<Vec<Vec<ParamValue>>> {
    match expr {
        Expr::List(list) => {
            let mut all_values = Vec::new();
            for elem in &list.elts {
                // Check if element is a tuple and matches parameter count
                match elem {
                    Expr::Tuple(tuple) if tuple.elts.len() == param_count && param_count > 1 => {
                        // Multiple parameters: unpack tuple
                        let param_values: Vec<ParamValue> =
                            tuple.elts.iter().map(format_param_value).collect();
                        all_values.push(param_values);
                    }
                    _ => {
                        // Single parameter or tuple as value: treat as single value
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

/// Build full dotted path from an attribute expression
/// e.g., SalesLeadEventName.ARM_RESET -> "SalesLeadEventName.ARM_RESET"
fn format_attribute_path(attr: &ExprAttribute) -> String {
    match attr.value.as_ref() {
        Expr::Name(name) => {
            // Simple case: Class.MEMBER
            format!("{}.{}", name.id, attr.attr)
        }
        Expr::Attribute(inner_attr) => {
            // Nested case: Module.Class.MEMBER
            format!("{}.{}", format_attribute_path(inner_attr), attr.attr)
        }
        _ => {
            // Fallback: just use attribute name
            attr.attr.to_string()
        }
    }
}

/// Format a parameter value for display in test name
/// Returns ParamValue::Simple for basic types, ParamValue::Complex for objects/calls
fn format_param_value(expr: &Expr) -> ParamValue {
    match expr {
        Expr::NumberLiteral(num) => {
            // Simple types: numbers are displayed as-is
            let value_str = match &num.value {
                ruff_python_ast::Number::Int(i) => i.to_string(),
                ruff_python_ast::Number::Float(f) => {
                    if f.fract() == 0.0 {
                        format!("{:.0}", f)
                    } else {
                        f.to_string()
                    }
                }
                ruff_python_ast::Number::Complex { real, imag } => {
                    format!("{}+{}j", real, imag)
                }
            };
            ParamValue::Simple(value_str)
        }
        Expr::StringLiteral(s) => {
            let str_value = s.value.to_str();
            // Check if string contains special characters that might be formatted differently
            // by pytest (newlines, tabs, non-ASCII characters)
            if str_value.contains('\n') || str_value.contains('\t') || str_value.contains('\r')
                || str_value.chars().any(|c| !c.is_ascii())
            {
                // Mark as uncertain - pytest may format these differently
                ParamValue::Complex
            } else {
                ParamValue::Simple(str_value.to_string())
            }
        }
        Expr::BooleanLiteral(b) => {
            // Simple type: booleans
            ParamValue::Simple(if b.value {
                "True".to_string()
            } else {
                "False".to_string()
            })
        }
        Expr::NoneLiteral(_) => {
            // Simple type: None
            ParamValue::Simple("None".to_string())
        }
        Expr::UnaryOp(unary) => {
            // Handle negative numbers as simple
            if let ruff_python_ast::UnaryOp::USub = unary.op {
                if let ParamValue::Simple(val) = format_param_value(&unary.operand) {
                    return ParamValue::Simple(format!("-{}", val));
                }
            }
            // Other unary operations are complex
            ParamValue::Complex
        }
        Expr::Attribute(attr) => {
            // Use full attribute path for class constants and enums
            // e.g., SalesLeadEventName.ARM_RESET -> "SalesLeadEventName.ARM_RESET"
            // However, pytest's behavior varies: Enums keep path, string constants don't
            // Mark these as AttributeAccess so files can be flagged as uncertain
            ParamValue::AttributeAccess(format_attribute_path(attr))
        }
        Expr::Name(name) => {
            // Simple name references (e.g., variable names, imported constants)
            ParamValue::Simple(name.id.to_string())
        }
        // Complex types: function calls, collections
        Expr::Call(_) | Expr::Tuple(_) | Expr::List(_) | Expr::Dict(_) => {
            ParamValue::Complex
        }
        _ => {
            // Unknown expressions are complex
            ParamValue::Complex
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
        .enumerate()
        .map(|(idx, v)| format_param_set_with_id(&parametrize_infos[0], v, idx))
        .collect::<Vec<_>>();

    // For stacked decorators, create cartesian product
    for info in parametrize_infos.iter().skip(1) {
        let mut new_combinations = Vec::new();
        for existing in &combinations {
            for (idx, value_set) in info.values.iter().enumerate() {
                let formatted = format_param_set_with_id(info, value_set, idx);
                let new_combo = format!("{}-{}", formatted, existing);
                new_combinations.push(new_combo);
            }
        }
        combinations = new_combinations;
    }

    combinations
}

/// Format a parameter set, using explicit ID if available
fn format_param_set_with_id(info: &ParametrizeInfo, values: &[ParamValue], set_index: usize) -> String {
    // If we have explicit IDs and this index is valid, use it
    if let Some(ref ids) = info.explicit_ids {
        if set_index < ids.len() {
            return ids[set_index].clone();
        }
    }
    // Otherwise fall back to auto-generation
    format_param_set(&info.param_names, values, set_index)
}

/// Format a single parameter set with auto-ID generation for complex values
fn format_param_set(param_names: &[String], values: &[ParamValue], set_index: usize) -> String {
    values
        .iter()
        .zip(param_names.iter())
        .map(|(value, name)| match value {
            ParamValue::Simple(s) => s.clone(),
            ParamValue::AttributeAccess(s) => s.clone(), // Use the path, but file will be marked uncertain
            ParamValue::Complex => {
                // Generate auto-ID like "a0", "b1", etc.
                format!("{}{}", name, set_index)
            }
        })
        .collect::<Vec<_>>()
        .join("-")
}

/// Check if any parameter values contain uncertain formatting (attribute accesses or complex expressions)
/// These include:
/// - AttributeAccess: e.g., Enum.VALUE - pytest's behavior varies for these
/// - Complex: function calls, tuples, etc. - pytest evaluates these to get the actual value
fn has_uncertain_param_values(parametrize_infos: &[ParametrizeInfo]) -> bool {
    for info in parametrize_infos {
        for value_set in &info.values {
            for value in value_set {
                if matches!(value, ParamValue::AttributeAccess(_) | ParamValue::Complex) {
                    return true;
                }
            }
        }
    }
    false
}

impl TestDiscoveryVisitor {
    pub fn new(config: &TestDiscoveryConfig) -> Self {
        Self {
            config: config.clone(),
            tests: Vec::new(),
            current_class: None,
            current_class_parametrize: Vec::new(),
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

                    // Extract class-level parametrize decorators
                    let class_parametrize = extract_class_parametrize_decorators(class);

                    // Collect all test methods in this class
                    for stmt in &class.body {
                        if let Stmt::FunctionDef(func) = stmt {
                            let method_name = func.name.as_str();
                            if self.is_test_function(method_name) {
                                // Check for method-level parametrize decorators
                                let mut parametrize_infos = extract_parametrize_decorators(func);

                                // Combine with class-level decorators (class decorators apply to all methods)
                                parametrize_infos.extend(class_parametrize.clone());

                                // Check if method has any parametrize decorator, even if we couldn't parse it
                                let has_any_parametrize = has_parametrize_decorator(func) || !class_parametrize.is_empty();

                                if parametrize_infos.is_empty() {
                                    methods.push(TestInfo {
                                        name: method_name.into(),
                                        line: func.range.start().to_u32() as usize,
                                        is_method: true,
                                        class_name: Some(name.into()),
                                        is_parametrized: has_any_parametrize,
                                        has_uncertain_params: false,
                                    });
                                } else {
                                    // Check if any params have uncertain formatting (e.g., attribute accesses)
                                    let has_uncertain = has_uncertain_param_values(&parametrize_infos);
                                    // Generate test items for each parameter combination
                                    let combinations = generate_param_combinations(&parametrize_infos);
                                    for combo in combinations {
                                        let parametrized_name = format!("{}[{}]", method_name, combo);
                                        methods.push(TestInfo {
                                            name: parametrized_name,
                                            line: func.range.start().to_u32() as usize,
                                            is_method: true,
                                            class_name: Some(name.into()),
                                            is_parametrized: true,
                                            has_uncertain_params: has_uncertain,
                                        });
                                    }
                                }
                            }
                        }
                    }

                    self.class_methods.insert(
                        name.into(),
                        (methods, classes_with_init.contains(name), class_parametrize),
                    );
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
            // Check for function-level parametrize decorators
            let mut parametrize_infos = extract_parametrize_decorators(func);

            // Add class-level parametrize decorators if we're in a class
            parametrize_infos.extend(self.current_class_parametrize.clone());

            // Check if function has any parametrize decorator, even if we couldn't parse it
            let has_any_parametrize = has_parametrize_decorator(func) || !self.current_class_parametrize.is_empty();

            if parametrize_infos.is_empty() {
                // No successfully parsed parametrize decorators - create single test
                // But mark as parametrized if the decorator exists but couldn't be parsed
                self.tests.push(TestInfo {
                    name: name.into(),
                    line: func.range.start().to_u32() as usize,
                    is_method: self.current_class.is_some(),
                    class_name: self.current_class.clone(),
                    is_parametrized: has_any_parametrize,
                    has_uncertain_params: false,
                });
            } else {
                // Check if any params have uncertain formatting (e.g., attribute accesses)
                let has_uncertain = has_uncertain_param_values(&parametrize_infos);
                // Generate test items for each parameter combination
                let combinations = generate_param_combinations(&parametrize_infos);
                for combo in combinations {
                    let parametrized_name = format!("{}[{}]", name, combo);
                    self.tests.push(TestInfo {
                        name: parametrized_name,
                        line: func.range.start().to_u32() as usize,
                        is_method: self.current_class.is_some(),
                        class_name: self.current_class.clone(),
                        is_parametrized: true,
                        has_uncertain_params: has_uncertain,
                    });
                }
            }
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
                            if let Some((_, parent_has_init, _)) =
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
                let prev_class_parametrize = self.current_class_parametrize.clone();

                self.current_class = Some(name.into());
                // Extract and set class-level parametrize decorators
                self.current_class_parametrize = extract_class_parametrize_decorators(class);

                // First, collect inherited methods from base classes
                if let Some(arguments) = &class.arguments {
                    for base_expr in arguments.args.iter() {
                        if let Expr::Name(base_name) = base_expr {
                            let base_class_name = base_name.id.as_str();

                            // If the base class is a test class in the same module,
                            // inherit its methods
                            if let Some((parent_methods, _, _)) =
                                self.class_methods.get(base_class_name)
                            {
                                for parent_method in parent_methods {
                                    // Create a copy of the parent method but with the child class name
                                    self.tests.push(TestInfo {
                                        name: parent_method.name.clone(),
                                        line: parent_method.line,
                                        is_method: true,
                                        class_name: Some(name.into()),
                                        is_parametrized: parent_method.is_parametrized,
                                        has_uncertain_params: parent_method.has_uncertain_params,
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
                self.current_class_parametrize = prev_class_parametrize;
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
