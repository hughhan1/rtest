//! Semantic analysis for test discovery with import resolution.

use ruff_python_ast::{Expr, ExprAttribute, ExprCall, Mod, ModModule, Stmt, StmtClassDef, StmtFunctionDef};
use ruff_python_parser::{parse, Mode, ParseOptions};
use ruff_python_semantic::{Module as SemanticModule, ModuleKind, ModuleSource, SemanticModel};
use ruff_text_size::Ranged;
use std::collections::{HashMap, HashSet};
use std::path::Path;

use crate::collection::error::{CollectionError, CollectionResult, CollectionWarning};
use crate::python_discovery::{
    discovery::{TestDiscoveryConfig, TestInfo},
    module_resolver::ModuleResolver,
    pattern,
};

/// Information about an import
#[derive(Debug, Clone)]
struct ImportInfo {
    module_path: Vec<String>,
    imported_name: String,
    /// The level of relative import (0 for absolute, 1 for '.', 2 for '..', etc.)
    relative_level: usize,
}

/// Resolved base class information
#[derive(Debug, Clone)]
pub struct ResolvedBaseClass {
    module_path: Vec<String>,
    class_name: String,
    is_local: bool,
}

/// Lightweight test method information without redundant data
#[derive(Debug, Clone)]
pub struct TestMethodInfo {
    pub name: String,
    pub line: usize,
    /// Whether this method has parametrize values with uncertain formatting
    pub has_uncertain_params: bool,
}

/// Test class information including methods
#[derive(Debug, Clone)]
pub struct TestClassInfo {
    pub name: String,
    pub has_init: bool,
    pub test_methods: Vec<TestMethodInfo>,
    pub base_classes: Vec<ResolvedBaseClass>,
}

/// Information about a single parametrize decorator
#[derive(Debug, Clone)]
struct ParametrizeInfo {
    /// Parameter names (e.g., ["a", "b", "threshold"])
    param_names: Vec<String>,
    /// List of parameter value tuples
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
    if let Expr::Call(ExprCall { func, arguments, .. }) = expr {
        if is_parametrize_call(func) {
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

/// Enhanced test discovery with semantic analysis
pub struct SemanticTestDiscovery {
    config: TestDiscoveryConfig,
    /// Cache of test classes by module path and class name
    class_cache: HashMap<(Vec<String>, String), TestClassInfo>,
    /// Warnings collected during discovery
    warnings: Vec<CollectionWarning>,
    /// Current file path for warning generation
    current_file_path: Option<String>,
}

impl SemanticTestDiscovery {
    pub fn new(config: TestDiscoveryConfig) -> Self {
        Self {
            config,
            class_cache: HashMap::new(),
            warnings: Vec::new(),
            current_file_path: None,
        }
    }

    /// Discover tests in a module with full import resolution
    pub fn discover_tests(
        &mut self,
        path: &Path,
        source: &str,
        module_path: Vec<String>,
        module_resolver: &mut ModuleResolver,
    ) -> CollectionResult<(Vec<TestInfo>, Vec<CollectionWarning>)> {
        // Store current file path for warning generation
        self.current_file_path = Some(path.to_string_lossy().to_string());

        // Parse the module
        let parsed = parse(source, ParseOptions::from(Mode::Module)).map_err(|e| {
            CollectionError::ParseError(format!("Failed to parse {}: {:?}", path.display(), e))
        })?;

        let ast_module = match parsed.into_syntax() {
            Mod::Module(module) => module,
            _ => return Ok((vec![], vec![])),
        };

        // Build semantic model
        let semantic_module = SemanticModule {
            kind: if path.file_name() == Some(std::ffi::OsStr::new("__init__.py")) {
                ModuleKind::Package
            } else {
                ModuleKind::Module
            },
            source: ModuleSource::File(path),
            python_ast: &ast_module.body,
            name: None,
        };

        let typing_modules = vec![];
        let semantic = SemanticModel::new(&typing_modules, path, semantic_module);

        // First pass: collect imports
        let imports = self.collect_imports(&ast_module, &semantic);

        // Second pass: collect test classes
        self.collect_test_classes(&ast_module, &module_path)?;

        // Third pass: resolve inheritance and collect all tests
        let mut all_tests = Vec::new();

        // Collect module-level test functions
        for stmt in &ast_module.body {
            if let Stmt::FunctionDef(func) = stmt {
                if self.is_test_function(func.name.as_str()) {
                    // Check for parametrize decorators
                    let parametrize_infos = extract_parametrize_decorators(func);

                    // Check if function has any parametrize decorator, even if we couldn't parse it
                    let has_any_parametrize = has_parametrize_decorator(func);

                    if parametrize_infos.is_empty() {
                        // No successfully parsed parametrize decorators - create single test
                        // But mark as parametrized if the decorator exists but couldn't be parsed
                        all_tests.push(TestInfo {
                            name: func.name.to_string(),
                            line: func.range().start().to_u32() as usize,
                            is_method: false,
                            class_name: None,
                            is_parametrized: has_any_parametrize,
                            has_uncertain_params: false,
                        });
                    } else {
                        // Check if any params have uncertain formatting (e.g., attribute accesses)
                        let has_uncertain = has_uncertain_param_values(&parametrize_infos);
                        // Generate test items for each parameter combination
                        let combinations = generate_param_combinations(&parametrize_infos);
                        for combo in combinations {
                            let parametrized_name = format!("{}[{}]", func.name, combo);
                            all_tests.push(TestInfo {
                                name: parametrized_name,
                                line: func.range().start().to_u32() as usize,
                                is_method: false,
                                class_name: None,
                                is_parametrized: true,
                                has_uncertain_params: has_uncertain,
                            });
                        }
                    }
                }
            }
        }

        // Collect test methods from classes
        for stmt in &ast_module.body {
            if let Stmt::ClassDef(class_def) = stmt {
                if let Some(tests) = self.collect_class_tests(
                    class_def,
                    &module_path,
                    &imports,
                    &semantic,
                    module_resolver,
                )? {
                    all_tests.extend(tests);
                }
            }
        }

        Ok((all_tests, std::mem::take(&mut self.warnings)))
    }

    /// Helper to collect imports from a module without requiring a semantic model
    fn collect_imports_from_module(&self, module: &ModModule) -> HashMap<String, ImportInfo> {
        self.collect_imports_impl(module)
    }

    /// Collect imports from the module
    fn collect_imports(
        &self,
        module: &ModModule,
        _semantic: &SemanticModel,
    ) -> HashMap<String, ImportInfo> {
        self.collect_imports_impl(module)
    }

    /// Internal implementation of import collection
    fn collect_imports_impl(&self, module: &ModModule) -> HashMap<String, ImportInfo> {
        let mut imports = HashMap::new();

        for stmt in &module.body {
            match stmt {
                Stmt::ImportFrom(import_from) => {
                    let level = import_from.level as usize;

                    // Get module path, handling both absolute and relative imports
                    let module_path = if let Some(module_name) = &import_from.module {
                        module_name.split('.').map(String::from).collect()
                    } else {
                        // Pure relative import (e.g., "from . import something")
                        Vec::new()
                    };

                    for alias in &import_from.names {
                        let imported_name = alias.name.to_string();
                        let local_name =
                            Self::get_alias_name_or_default(alias.asname.as_ref(), &imported_name);

                        imports.insert(
                            local_name,
                            ImportInfo {
                                module_path: module_path.clone(),
                                imported_name,
                                relative_level: level,
                            },
                        );
                    }
                }
                Stmt::Import(import) => {
                    for alias in &import.names {
                        let parts: Vec<String> = alias.name.split('.').map(String::from).collect();
                        let local_name =
                            Self::get_alias_name_or_default(alias.asname.as_ref(), &alias.name);

                        imports.insert(
                            local_name,
                            ImportInfo {
                                module_path: parts,
                                imported_name: String::new(),
                                relative_level: 0,
                            },
                        );
                    }
                }
                _ => {}
            }
        }

        imports
    }

    /// Collect test classes in the current module
    fn collect_test_classes(
        &mut self,
        module: &ModModule,
        module_path: &[String],
    ) -> CollectionResult<()> {
        for stmt in &module.body {
            if let Stmt::ClassDef(class_def) = stmt {
                let class_name = class_def.name.as_str();

                if self.is_test_class(class_name) {
                    let has_init = self.class_has_init(class_def);
                    let test_methods = self.collect_test_methods(class_def);
                    let imports = self.collect_imports_from_module(module);
                    let base_classes =
                        self.collect_base_class_names(class_def, module_path, &imports)?;

                    let info = TestClassInfo {
                        name: class_name.to_string(),
                        has_init,
                        test_methods,
                        base_classes,
                    };

                    self.class_cache
                        .insert((module_path.to_vec(), class_name.to_string()), info);
                }
            }
        }

        Ok(())
    }

    /// Collect test methods from a class
    fn collect_test_methods(&self, class_def: &StmtClassDef) -> Vec<TestMethodInfo> {
        let mut methods = Vec::new();

        for stmt in &class_def.body {
            if let Stmt::FunctionDef(func) = stmt {
                let method_name = func.name.as_str();
                if self.is_test_function(method_name) {
                    // Check for parametrize decorators
                    let parametrize_infos = extract_parametrize_decorators(func);

                    if parametrize_infos.is_empty() {
                        // No parametrize decorators - create single test method
                        methods.push(TestMethodInfo {
                            name: method_name.to_string(),
                            line: func.range().start().to_u32() as usize,
                            has_uncertain_params: false,
                        });
                    } else {
                        // Check if any params have uncertain formatting (e.g., attribute accesses)
                        let has_uncertain = has_uncertain_param_values(&parametrize_infos);
                        // Generate test items for each parameter combination
                        let combinations = generate_param_combinations(&parametrize_infos);
                        for combo in combinations {
                            let parametrized_name = format!("{}[{}]", method_name, combo);
                            methods.push(TestMethodInfo {
                                name: parametrized_name,
                                line: func.range().start().to_u32() as usize,
                                has_uncertain_params: has_uncertain,
                            });
                        }
                    }
                }
            }
        }

        methods
    }

    /// Collect base class names from a class definition
    fn collect_base_class_names(
        &self,
        class_def: &StmtClassDef,
        current_module_path: &[String],
        imports: &HashMap<String, ImportInfo>,
    ) -> CollectionResult<Vec<ResolvedBaseClass>> {
        let mut base_classes = Vec::new();

        if let Some(arguments) = &class_def.arguments {
            for base in arguments.args.iter() {
                let resolved = self.resolve_base_class(
                    base,
                    current_module_path,
                    imports,
                    &SemanticModel::new(
                        &[],
                        Path::new(""),
                        SemanticModule {
                            kind: ModuleKind::Module,
                            source: ModuleSource::File(Path::new("")),
                            python_ast: &[],
                            name: None,
                        },
                    ),
                )?;

                match resolved {
                    Some(resolved) => base_classes.push(resolved),
                    None => {
                        // Skip unresolvable base classes (e.g., Generic[T], Protocol, etc.)
                        // These are typically typing constructs that don't affect test collection
                        continue;
                    }
                }
            }
        }

        Ok(base_classes)
    }

    /// Collect all tests from a class, including inherited ones
    fn collect_class_tests(
        &mut self,
        class_def: &StmtClassDef,
        current_module_path: &[String],
        imports: &HashMap<String, ImportInfo>,
        semantic: &SemanticModel,
        module_resolver: &mut ModuleResolver,
    ) -> CollectionResult<Option<Vec<TestInfo>>> {
        let class_name = class_def.name.as_str();

        // Check if this class should be collected
        if !self.is_test_class(class_name) {
            return Ok(None);
        }

        // Check if this class or any parent has __init__
        if self.should_skip_class(class_def, current_module_path, imports, module_resolver)? {
            // Add warning about skipped class
            let warning = CollectionWarning {
                file_path: self
                    .current_file_path
                    .clone()
                    .unwrap_or_else(|| "unknown".to_string()),
                line: class_def.range().start().to_u32() as usize + 1, // Convert 0-based to 1-based
                message: format!(
                    "cannot collect test class '{}' because it has a __init__ constructor",
                    class_name
                ),
            };
            self.warnings.push(warning);
            return Ok(None);
        }

        let mut all_tests = Vec::new();

        // Extract class-level parametrize decorators
        let class_parametrize = extract_class_parametrize_decorators(class_def);

        // Collect inherited methods
        if let Some(arguments) = &class_def.arguments {
            for base_expr in arguments.args.iter() {
                let resolved =
                    self.resolve_base_class(base_expr, current_module_path, imports, semantic)?;

                match resolved {
                    Some(resolved) => {
                        // Skip inheritance analysis for known stdlib modules that we can't analyze
                        if self.is_stdlib_module_to_skip(&resolved.module_path) {
                            // Skip inheritance collection for stdlib modules like unittest.TestCase
                            // The class will still be collected with its own methods
                            continue;
                        }

                        // Get methods from the base class
                        if let Some(base_methods) =
                            self.get_base_class_methods(&resolved, module_resolver)?
                        {
                            for method in base_methods {
                                // Create a copy with the current class name
                                // Inherited parametrized methods are still parametrized
                                let is_param = method.name.contains('[');
                                all_tests.push(TestInfo {
                                    name: method.name.clone(),
                                    line: method.line,
                                    is_method: true,
                                    class_name: Some(class_name.to_string()),
                                    is_parametrized: is_param,
                                    has_uncertain_params: method.has_uncertain_params,
                                });
                            }
                        }
                    }
                    None => {
                        // Skip unresolvable base classes (e.g., Generic[T], Protocol, etc.)
                        // These are typically typing constructs that don't affect test collection
                        continue;
                    }
                }
            }
        }

        // Collect methods defined in this class
        let mut own_method_names = std::collections::HashSet::new();
        for stmt in &class_def.body {
            if let Stmt::FunctionDef(func) = stmt {
                let method_name = func.name.as_str();
                if self.is_test_function(method_name) {
                    // Track base method name for override detection
                    own_method_names.insert(method_name.to_string());

                    // Check for method-level parametrize decorators
                    let mut parametrize_infos = extract_parametrize_decorators(func);

                    // Combine with class-level decorators (class decorators apply to all methods)
                    parametrize_infos.extend(class_parametrize.clone());

                    // Check if method has any parametrize decorator, even if we couldn't parse it
                    let has_any_parametrize = has_parametrize_decorator(func) || !class_parametrize.is_empty();

                    if parametrize_infos.is_empty() {
                        // No successfully parsed parametrize decorators - create single test
                        // But mark as parametrized if the decorator exists but couldn't be parsed
                        all_tests.push(TestInfo {
                            name: method_name.to_string(),
                            line: func.range().start().to_u32() as usize,
                            is_method: true,
                            class_name: Some(class_name.to_string()),
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
                            all_tests.push(TestInfo {
                                name: parametrized_name,
                                line: func.range().start().to_u32() as usize,
                                is_method: true,
                                class_name: Some(class_name.to_string()),
                                is_parametrized: true,
                                has_uncertain_params: has_uncertain,
                            });
                        }
                    }
                }
            }
        }

        // Remove inherited methods that are overridden by methods defined in this class
        all_tests.retain(|test| {
            // Keep the test if it's defined in this class OR if it's not overridden
            (test.class_name.as_ref() == Some(&class_name.to_string())
                && test.line >= class_def.range().start().to_u32() as usize)
                || !own_method_names.contains(&test.name)
        });

        Ok(Some(all_tests))
    }

    /// Check if a class should be skipped (has __init__ or inherits from class with __init__)
    fn should_skip_class(
        &mut self,
        class_def: &StmtClassDef,
        current_module_path: &[String],
        imports: &HashMap<String, ImportInfo>,
        module_resolver: &mut ModuleResolver,
    ) -> CollectionResult<bool> {
        // Check if this class has __init__
        if self.class_has_init(class_def) {
            return Ok(true);
        }

        // Check parent classes
        if let Some(arguments) = &class_def.arguments {
            for base_expr in arguments.args.iter() {
                let resolved = self.resolve_base_class(
                    base_expr,
                    current_module_path,
                    imports,
                    &SemanticModel::new(
                        &[],
                        Path::new(""),
                        SemanticModule {
                            kind: ModuleKind::Module,
                            source: ModuleSource::File(Path::new("")),
                            python_ast: &[],
                            name: None,
                        },
                    ),
                )?;

                match resolved {
                    Some(resolved) => {
                        // Skip init check for known stdlib modules
                        if self.is_stdlib_module_to_skip(&resolved.module_path) {
                            // Assume stdlib modules like unittest.TestCase don't prevent collection
                            continue;
                        }

                        if self.base_class_has_init(&resolved, module_resolver)? {
                            return Ok(true);
                        }
                    }
                    None => {
                        // Skip unresolvable base classes (e.g., Generic[T], Protocol, etc.)
                        // Assume they don't have __init__ that would prevent collection
                        continue;
                    }
                }
            }
        }

        Ok(false)
    }

    /// Check if a base class has __init__ (recursively checking ancestors)
    fn base_class_has_init(
        &mut self,
        resolved: &ResolvedBaseClass,
        module_resolver: &mut ModuleResolver,
    ) -> CollectionResult<bool> {
        let mut visited = HashSet::new();
        self.base_class_has_init_impl(resolved, module_resolver, &mut visited)
    }

    /// Internal implementation of base_class_has_init with cycle detection
    fn base_class_has_init_impl(
        &mut self,
        resolved: &ResolvedBaseClass,
        module_resolver: &mut ModuleResolver,
        visited: &mut HashSet<(Vec<String>, String)>,
    ) -> CollectionResult<bool> {
        // Check for cycles
        let key = (resolved.module_path.clone(), resolved.class_name.clone());
        if visited.contains(&key) {
            // Cycle detected - assume no init to break the cycle
            return Ok(false);
        }
        visited.insert(key.clone());

        if !resolved.is_local {
            // Load the external module if needed
            self.ensure_module_loaded(&resolved.module_path, module_resolver)?;
        }

        // Get the base class info
        let base_info = match self.class_cache.get(&key) {
            Some(info) => info.clone(),
            None => return Ok(false),
        };

        // If this class has __init__, return true
        if base_info.has_init {
            return Ok(true);
        }

        // Otherwise, recursively check all parent classes
        for parent in &base_info.base_classes {
            if self.base_class_has_init_impl(parent, module_resolver, visited)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Get test methods from a base class, including inherited methods
    fn get_base_class_methods(
        &mut self,
        resolved: &ResolvedBaseClass,
        module_resolver: &mut ModuleResolver,
    ) -> CollectionResult<Option<Vec<TestMethodInfo>>> {
        let mut visited = HashSet::new();
        self.get_base_class_methods_impl(resolved, module_resolver, &mut visited)
    }

    /// Internal implementation of get_base_class_methods with cycle detection
    fn get_base_class_methods_impl(
        &mut self,
        resolved: &ResolvedBaseClass,
        module_resolver: &mut ModuleResolver,
        visited: &mut HashSet<(Vec<String>, String)>,
    ) -> CollectionResult<Option<Vec<TestMethodInfo>>> {
        // Check if we've already visited this class (diamond inheritance)
        let key = (resolved.module_path.clone(), resolved.class_name.clone());
        if visited.contains(&key) {
            // Already visited - skip to avoid duplicates (not an error, just diamond inheritance)
            return Ok(None);
        }
        visited.insert(key.clone());

        if !resolved.is_local {
            // Load the external module if needed
            self.ensure_module_loaded(&resolved.module_path, module_resolver)?;
        }

        // Get the base class info
        let base_info = match self.class_cache.get(&key) {
            Some(info) => info.clone(),
            None => {
                return Ok(None);
            }
        };

        let mut all_methods = Vec::new();

        // First, recursively get methods from this class's base classes
        for base_class in &base_info.base_classes {
            if let Some(parent_methods) =
                self.get_base_class_methods_impl(base_class, module_resolver, visited)?
            {
                all_methods.extend(parent_methods);
            }
        }

        // Then add this class's own methods
        all_methods.extend(base_info.test_methods.clone());

        Ok(Some(all_methods))
    }

    /// Ensure a module is loaded and its test classes are cached
    fn ensure_module_loaded(
        &mut self,
        module_path: &[String],
        module_resolver: &mut ModuleResolver,
    ) -> CollectionResult<()> {
        // Check if we've already loaded this module
        let cache_key = (module_path.to_vec(), String::new());
        if self.class_cache.contains_key(&cache_key) {
            return Ok(());
        }

        // Skip built-in/stdlib modules - they can't be analyzed for test collection
        if self.is_stdlib_module_to_skip(module_path) {
            return Ok(());
        }

        // Load the module and collect test classes
        {
            let parsed_module = match module_resolver.resolve_and_load(module_path) {
                Ok(module) => module,
                Err(CollectionError::ImportError(msg)) if msg.contains("built-in module") => {
                    // Skip built-in modules gracefully - they don't contain test code
                    return Ok(());
                }
                Err(e) => return Err(e),
            };

            // Extract test class information without storing the module
            for stmt in &parsed_module.module.body {
                if let Stmt::ClassDef(class_def) = stmt {
                    let class_name = class_def.name.as_str();

                    // Always collect class info for external modules, even if they don't match test patterns
                    // They might be used as base classes
                    let has_init = self.class_has_init(class_def);
                    let test_methods = self.collect_test_methods(class_def);
                    let imports = self.collect_imports_from_module(&parsed_module.module);
                    let base_classes =
                        self.collect_base_class_names(class_def, module_path, &imports)?;

                    let info = TestClassInfo {
                        name: class_name.to_string(),
                        has_init,
                        test_methods,
                        base_classes,
                    };

                    self.class_cache
                        .insert((module_path.to_vec(), class_name.to_string()), info);
                }
            }
        }

        // Mark module as loaded
        self.class_cache.insert(
            cache_key,
            TestClassInfo {
                name: String::new(),
                has_init: false,
                test_methods: vec![],
                base_classes: vec![],
            },
        );

        Ok(())
    }

    /// Resolve a base class expression to module path and class name
    fn resolve_base_class(
        &self,
        base_expr: &Expr,
        current_module_path: &[String],
        imports: &HashMap<String, ImportInfo>,
        _semantic: &SemanticModel,
    ) -> CollectionResult<Option<ResolvedBaseClass>> {
        match base_expr {
            Expr::Name(name_expr) => {
                let name = name_expr.id.as_str();
                // Check if it's an imported class
                if let Some(import_info) = imports.get(name) {
                    // Handle relative imports by resolving them to absolute paths
                    let module_path = if import_info.relative_level > 0 {
                        // Resolve relative import to absolute path
                        Self::resolve_relative_module_path(
                            current_module_path,
                            import_info.relative_level,
                            &import_info.module_path,
                        )?
                    } else {
                        import_info.module_path.clone()
                    };

                    return Ok(Some(ResolvedBaseClass {
                        module_path,
                        class_name: import_info.imported_name.clone(),
                        is_local: false,
                    }));
                }

                // Otherwise, it's a local class
                Ok(Some(ResolvedBaseClass {
                    module_path: current_module_path.to_vec(),
                    class_name: name.to_string(),
                    is_local: true,
                }))
            }
            Expr::Attribute(attr_expr) => {
                // Handle module.Class pattern
                if let Expr::Name(module_name) = &*attr_expr.value {
                    if let Some(import_info) = imports.get(module_name.id.as_str()) {
                        // Handle relative imports
                        let module_path = if import_info.relative_level > 0 {
                            Self::resolve_relative_module_path(
                                current_module_path,
                                import_info.relative_level,
                                &import_info.module_path,
                            )?
                        } else {
                            import_info.module_path.clone()
                        };

                        return Ok(Some(ResolvedBaseClass {
                            module_path,
                            class_name: attr_expr.attr.to_string(),
                            is_local: false,
                        }));
                    }
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// Helper to resolve relative imports to absolute module paths
    fn resolve_relative_module_path(
        current_module_path: &[String],
        relative_level: usize,
        module_parts: &[String],
    ) -> CollectionResult<Vec<String>> {
        if relative_level == 0 {
            // Not a relative import
            return Ok(module_parts.to_vec());
        }

        // For relative imports, go up the module hierarchy
        // Level 1 (.) = current package
        // Level 2 (..) = parent package, etc.

        if current_module_path.len() < relative_level {
            // Can't resolve beyond top-level package
            return Err(CollectionError::ImportError(format!(
                "Attempted relative import beyond top-level package (level {} from depth {})",
                relative_level,
                current_module_path.len()
            )));
        }

        // Go up the hierarchy by relative_level
        let base_path = &current_module_path[..current_module_path.len() - relative_level];

        // Combine with the module parts from the import
        let mut result = base_path.to_vec();
        result.extend_from_slice(module_parts);

        Ok(result)
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

    /// Format a base class expression for error messages
    fn format_base_class_expr(&self, base_expr: &Expr) -> String {
        match base_expr {
            Expr::Name(name_expr) => name_expr.id.to_string(),
            Expr::Attribute(attr_expr) => {
                if let Expr::Name(module_name) = &*attr_expr.value {
                    format!("{}.{}", module_name.id, attr_expr.attr)
                } else {
                    format!("<complex>.{}", attr_expr.attr)
                }
            }
            _ => "<unresolvable>".to_string(),
        }
    }

    /// Check if a module should be skipped for inheritance analysis
    fn is_stdlib_module_to_skip(&self, module_path: &[String]) -> bool {
        if module_path.is_empty() {
            return false;
        }

        let module_name = &module_path[0];
        // Use ruff's stdlib module check for comprehensive stdlib coverage
        use ruff_python_stdlib::sys::is_known_standard_library;
        is_known_standard_library(11, module_name)
    }

    /// Helper to get alias name with fallback to the original name
    fn get_alias_name_or_default(
        alias_name: Option<&ruff_python_ast::Identifier>,
        default: &str,
    ) -> String {
        alias_name
            .map(|n| n.to_string())
            .unwrap_or_else(|| default.to_string())
    }
}
