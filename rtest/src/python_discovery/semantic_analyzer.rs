//! Semantic analysis for test discovery with import resolution.

use ruff_python_ast::{
    Expr, ExprAttribute, ExprCall, Mod, ModModule, Stmt, StmtClassDef, StmtFunctionDef,
};
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
    /// Parameter names (e.g., ["x", "y"] or ["value"])
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
    if let Expr::Call(ExprCall {
        func, arguments, ..
    }) = expr
    {
        if is_parametrize_call(func) {
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
            // Handle string format: "x,y,expected"
            let param_str = s.value.to_str();
            Some(param_str.split(',').map(|p| p.trim().to_string()).collect())
        }
        Expr::Tuple(tuple) => {
            // Handle tuple format: ("x", "y", "expected")
            let mut param_names = Vec::new();
            for elem in &tuple.elts {
                if let Expr::StringLiteral(s) = elem {
                    param_names.push(s.value.to_str().to_string());
                } else {
                    // If any element is not a string literal, fail
                    return None;
                }
            }
            Some(param_names)
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
                match elem {
                    Expr::Tuple(tuple) => {
                        let param_values: Vec<String> =
                            tuple.elts.iter().map(format_param_value).collect();
                        all_values.push(param_values);
                    }
                    _ => {
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
        Expr::NumberLiteral(num) => match &num.value {
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
        },
        Expr::StringLiteral(s) => s.value.to_str().to_string(),
        Expr::BooleanLiteral(b) => {
            if b.value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Expr::NoneLiteral(_) => "None".to_string(),
        Expr::Tuple(tuple) => {
            let elements: Vec<String> = tuple.elts.iter().map(format_param_value).collect();
            elements.join("-")
        }
        Expr::List(list) => {
            let elements: Vec<String> = list.elts.iter().map(format_param_value).collect();
            format!("[{}]", elements.join("-"))
        }
        Expr::UnaryOp(unary) => {
            if let ruff_python_ast::UnaryOp::USub = unary.op {
                format!("-{}", format_param_value(&unary.operand))
            } else {
                format!("{:?}", expr).chars().take(20).collect()
            }
        }
        _ => format!("{:?}", expr).chars().take(20).collect(),
    }
}

/// Generate all parameter combinations from multiple parametrize decorators
fn generate_param_combinations(parametrize_infos: &[ParametrizeInfo]) -> Vec<String> {
    if parametrize_infos.is_empty() {
        return vec![];
    }

    let mut combinations = parametrize_infos[0]
        .values
        .iter()
        .map(|v| v.join("-"))
        .collect::<Vec<_>>();

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

/// Extract the base method name from a parametrized test name
/// For example, "test_example[1-2-3]" returns "test_example"
fn extract_base_method_name(name: &str) -> &str {
    name.split('[').next().unwrap_or(name)
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
                    let parametrize_infos = extract_parametrize_decorators(func);

                    if parametrize_infos.is_empty() {
                        // Non-parametrized test
                        all_tests.push(TestInfo {
                            name: func.name.to_string(),
                            line: func.range().start().to_u32() as usize,
                            is_method: false,
                            class_name: None,
                        });
                    } else {
                        // Generate parametrized test instances
                        let combinations = generate_param_combinations(&parametrize_infos);
                        for combo in combinations {
                            let parametrized_name = format!("{}[{}]", func.name, combo);
                            all_tests.push(TestInfo {
                                name: parametrized_name,
                                line: func.range().start().to_u32() as usize,
                                is_method: false,
                                class_name: None,
                            });
                        }
                    }
                }
            }
        }

        // Collect test methods from classes defined in this module
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

        // Collect test methods from imported test classes
        // This matches pytest's behavior where imported test classes are collected
        for (_import_name, import_info) in &imports {
            // Only process imports that import a specific name (not entire modules)
            if !import_info.imported_name.is_empty() {
                // Check if the imported name is a test class
                if self.is_test_class(&import_info.imported_name) {
                    // Resolve the import to get the module path
                    let resolved_module = if import_info.relative_level > 0 {
                        Self::resolve_relative_module_path(
                            &module_path,
                            import_info.relative_level,
                            &import_info.module_path,
                        )?
                    } else {
                        import_info.module_path.clone()
                    };

                    // Load the module and collect the class
                    self.ensure_module_loaded(&resolved_module, module_resolver)?;

                    // Create a resolved base class reference
                    let resolved = ResolvedBaseClass {
                        module_path: resolved_module,
                        class_name: import_info.imported_name.clone(),
                        is_local: false,
                    };

                    // Check if class has __init__ (including inherited)
                    if self.base_class_has_init(&resolved, module_resolver)? {
                        continue;
                    }

                    // Get all methods from this imported class (including inherited)
                    if let Some(methods) =
                        self.get_base_class_methods(&resolved, module_resolver)?
                    {
                        for method in methods {
                            all_tests.push(TestInfo {
                                name: method.name.clone(),
                                line: method.line,
                                is_method: true,
                                class_name: Some(import_info.imported_name.clone()),
                            });
                        }
                    }
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
                    let parametrize_infos = extract_parametrize_decorators(func);

                    if parametrize_infos.is_empty() {
                        // Non-parametrized method
                        methods.push(TestMethodInfo {
                            name: method_name.to_string(),
                            line: func.range().start().to_u32() as usize,
                        });
                    } else {
                        // Generate parametrized test instances
                        let combinations = generate_param_combinations(&parametrize_infos);
                        for combo in combinations {
                            let parametrized_name = format!("{}[{}]", method_name, combo);
                            methods.push(TestMethodInfo {
                                name: parametrized_name,
                                line: func.range().start().to_u32() as usize,
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
                        return Err(CollectionError::ImportError(format!(
                            "Could not resolve base class '{}' for class '{}'",
                            self.format_base_class_expr(base),
                            class_def.name
                        )));
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
                                all_tests.push(TestInfo {
                                    name: method.name.clone(),
                                    line: method.line,
                                    is_method: true,
                                    class_name: Some(class_name.to_string()),
                                });
                            }
                        }
                    }
                    None => {
                        return Err(CollectionError::ImportError(format!(
                            "Could not resolve base class '{}' for inheritance in class '{}'",
                            self.format_base_class_expr(base_expr),
                            class_name
                        )));
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
                    own_method_names.insert(method_name.to_string());

                    let parametrize_infos = extract_parametrize_decorators(func);

                    if parametrize_infos.is_empty() {
                        // Non-parametrized method
                        all_tests.push(TestInfo {
                            name: method_name.to_string(),
                            line: func.range().start().to_u32() as usize,
                            is_method: true,
                            class_name: Some(class_name.to_string()),
                        });
                    } else {
                        // Generate parametrized test instances
                        let combinations = generate_param_combinations(&parametrize_infos);
                        for combo in combinations {
                            let parametrized_name = format!("{}[{}]", method_name, combo);
                            all_tests.push(TestInfo {
                                name: parametrized_name,
                                line: func.range().start().to_u32() as usize,
                                is_method: true,
                                class_name: Some(class_name.to_string()),
                            });
                        }
                    }
                }
            }
        }

        // Remove inherited methods that are overridden by methods defined in this class
        all_tests.retain(|test| {
            // Keep the test if it's defined in this class OR if it's not overridden
            // For parametrized tests, extract the base name before checking
            (test.class_name.as_ref() == Some(&class_name.to_string())
                && test.line >= class_def.range().start().to_u32() as usize)
                || !own_method_names.contains(extract_base_method_name(&test.name))
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
                        return Err(CollectionError::ImportError(format!(
                            "Could not resolve base class '{}' for __init__ check in class '{}'",
                            self.format_base_class_expr(base_expr),
                            class_def.name
                        )));
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
        // Check for cycles
        let key = (resolved.module_path.clone(), resolved.class_name.clone());
        if visited.contains(&key) {
            // Cycle detected - this is an error condition
            return Err(CollectionError::ParseError(format!(
                "Circular inheritance detected in class hierarchy involving '{}'",
                resolved.class_name
            )));
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

        // Load the module and collect test classes
        {
            let parsed_module = module_resolver.resolve_and_load(module_path)?;

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
        matches!(
            module_name.as_str(),
            "unittest"    // unittest.TestCase and related
            | "abc"       // abc.ABC for abstract base classes  
            | "typing" // typing.Protocol, typing.Generic, etc.
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::python_discovery::TestDiscoveryConfig;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_extract_base_method_name() {
        assert_eq!(extract_base_method_name("test_example"), "test_example");
        assert_eq!(extract_base_method_name("test_example[1]"), "test_example");
        assert_eq!(
            extract_base_method_name("test_example[1-2-3]"),
            "test_example"
        );
        assert_eq!(
            extract_base_method_name("test_method[alice]"),
            "test_method"
        );
        assert_eq!(
            extract_base_method_name("test_stacked[10-1]"),
            "test_stacked"
        );
    }

    #[test]
    fn test_parametrize_single_param() {
        let source = r#"
import pytest

@pytest.mark.parametrize("value", [1, 2, 3])
def test_simple(value):
    assert value > 0
"#;

        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path().join("test_param.py");
        fs::write(&test_path, source).unwrap();

        let config = TestDiscoveryConfig::default();
        let mut module_resolver =
            crate::python_discovery::ModuleResolver::new(temp_dir.path()).unwrap();
        let module_path = vec!["test_param".to_string()];
        let mut discovery = SemanticTestDiscovery::new(config);

        let (tests, _warnings) = discovery
            .discover_tests(&test_path, source, module_path, &mut module_resolver)
            .unwrap();

        assert_eq!(tests.len(), 3);
        assert_eq!(tests[0].name, "test_simple[1]");
        assert_eq!(tests[1].name, "test_simple[2]");
        assert_eq!(tests[2].name, "test_simple[3]");
    }

    #[test]
    fn test_parametrize_multiple_params() {
        let source = r#"
import pytest

@pytest.mark.parametrize("x,y,expected", [
    (1, 2, 3),
    (5, 5, 10),
    (10, -5, 5),
])
def test_add(x, y, expected):
    assert x + y == expected
"#;

        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path().join("test_param.py");
        fs::write(&test_path, source).unwrap();

        let config = TestDiscoveryConfig::default();
        let mut module_resolver =
            crate::python_discovery::ModuleResolver::new(temp_dir.path()).unwrap();
        let module_path = vec!["test_param".to_string()];
        let mut discovery = SemanticTestDiscovery::new(config);

        let (tests, _warnings) = discovery
            .discover_tests(&test_path, source, module_path, &mut module_resolver)
            .unwrap();

        assert_eq!(tests.len(), 3);
        assert_eq!(tests[0].name, "test_add[1-2-3]");
        assert_eq!(tests[1].name, "test_add[5-5-10]");
        assert_eq!(tests[2].name, "test_add[10--5-5]");
    }

    #[test]
    fn test_parametrize_tuple_format() {
        let source = r#"
import pytest

@pytest.mark.parametrize(("x", "y", "expected"), [
    (1, 2, 3),
    (5, 5, 10),
    (10, -5, 5),
])
def test_add(x, y, expected):
    assert x + y == expected
"#;

        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path().join("test_param.py");
        fs::write(&test_path, source).unwrap();

        let config = TestDiscoveryConfig::default();
        let mut module_resolver =
            crate::python_discovery::ModuleResolver::new(temp_dir.path()).unwrap();
        let module_path = vec!["test_param".to_string()];
        let mut discovery = SemanticTestDiscovery::new(config);

        let (tests, _warnings) = discovery
            .discover_tests(&test_path, source, module_path, &mut module_resolver)
            .unwrap();

        assert_eq!(tests.len(), 3);
        assert_eq!(tests[0].name, "test_add[1-2-3]");
        assert_eq!(tests[1].name, "test_add[5-5-10]");
        assert_eq!(tests[2].name, "test_add[10--5-5]");
    }

    #[test]
    fn test_parametrize_stacked() {
        let source = r#"
import pytest

@pytest.mark.parametrize("value", [1, 2])
@pytest.mark.parametrize("multiplier", [10, 20])
def test_stacked(value, multiplier):
    assert value * multiplier > 0
"#;

        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path().join("test_param.py");
        fs::write(&test_path, source).unwrap();

        let config = TestDiscoveryConfig::default();
        let mut module_resolver =
            crate::python_discovery::ModuleResolver::new(temp_dir.path()).unwrap();
        let module_path = vec!["test_param".to_string()];
        let mut discovery = SemanticTestDiscovery::new(config);

        let (tests, _warnings) = discovery
            .discover_tests(&test_path, source, module_path, &mut module_resolver)
            .unwrap();

        // With stacked decorators: 2 values × 2 multipliers = 4 combinations
        assert_eq!(tests.len(), 4);
        assert!(tests.iter().any(|t| t.name == "test_stacked[10-1]"));
        assert!(tests.iter().any(|t| t.name == "test_stacked[10-2]"));
        assert!(tests.iter().any(|t| t.name == "test_stacked[20-1]"));
        assert!(tests.iter().any(|t| t.name == "test_stacked[20-2]"));
    }

    #[test]
    fn test_parametrize_in_class() {
        let source = r#"
import pytest

class TestClass:
    @pytest.mark.parametrize("name", ["alice", "bob"])
    def test_names(self, name):
        assert len(name) > 0
"#;

        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path().join("test_param.py");
        fs::write(&test_path, source).unwrap();

        let config = TestDiscoveryConfig::default();
        let mut module_resolver =
            crate::python_discovery::ModuleResolver::new(temp_dir.path()).unwrap();
        let module_path = vec!["test_param".to_string()];
        let mut discovery = SemanticTestDiscovery::new(config);

        let (tests, _warnings) = discovery
            .discover_tests(&test_path, source, module_path, &mut module_resolver)
            .unwrap();

        assert_eq!(tests.len(), 2);
        assert_eq!(tests[0].name, "test_names[alice]");
        assert_eq!(tests[1].name, "test_names[bob]");
        assert_eq!(tests[0].class_name, Some("TestClass".to_string()));
        assert_eq!(tests[1].class_name, Some("TestClass".to_string()));
    }

    #[test]
    fn test_parametrize_inheritance_override() {
        let temp_dir = TempDir::new().unwrap();
        let tests_dir = temp_dir.path().join("tests");
        fs::create_dir(&tests_dir).unwrap();

        // Create base class with parametrized test
        let base_source = r#"
import pytest

class TestBase:
    @pytest.mark.parametrize("value", [1, 2, 3])
    def test_param(self, value):
        assert value > 0
"#;
        fs::write(tests_dir.join("test_base.py"), base_source).unwrap();

        // Create derived class that inherits the parametrized test
        let derived_source = r#"
from tests.test_base import TestBase

class TestDerived(TestBase):
    def test_own_method(self):
        pass
"#;
        let derived_path = tests_dir.join("test_derived.py");
        fs::write(&derived_path, derived_source).unwrap();

        let config = TestDiscoveryConfig::default();
        let mut module_resolver =
            crate::python_discovery::ModuleResolver::new(temp_dir.path()).unwrap();
        let module_path = vec!["tests".to_string(), "test_derived".to_string()];
        let mut discovery = SemanticTestDiscovery::new(config);

        let (tests, _warnings) = discovery
            .discover_tests(
                &derived_path,
                derived_source,
                module_path,
                &mut module_resolver,
            )
            .unwrap();

        // Should have (matching pytest behavior):
        // - TestBase (imported): 3 parametrized tests
        // - TestDerived: 3 inherited parametrized tests + 1 own method = 4
        // Total: 7 tests
        assert_eq!(tests.len(), 7);

        let param_tests: Vec<_> = tests
            .iter()
            .filter(|t| t.name.starts_with("test_param"))
            .collect();
        assert_eq!(param_tests.len(), 6); // 3 from TestBase + 3 from TestDerived
        assert!(param_tests.iter().any(|t| t.name == "test_param[1]"));
        assert!(param_tests.iter().any(|t| t.name == "test_param[2]"));
        assert!(param_tests.iter().any(|t| t.name == "test_param[3]"));
    }

    #[test]
    fn test_parametrize_inheritance_with_override() {
        let temp_dir = TempDir::new().unwrap();
        let tests_dir = temp_dir.path().join("tests");
        fs::create_dir(&tests_dir).unwrap();

        // Create base class with parametrized test
        let base_source = r#"
import pytest

class TestBase:
    @pytest.mark.parametrize("value", [1, 2])
    def test_param(self, value):
        assert value > 0
"#;
        fs::write(tests_dir.join("test_base.py"), base_source).unwrap();

        // Create derived class that OVERRIDES the parametrized test with different params
        let derived_source = r#"
import pytest
from tests.test_base import TestBase

class TestDerived(TestBase):
    @pytest.mark.parametrize("value", [10, 20, 30])
    def test_param(self, value):
        assert value > 5
"#;
        let derived_path = tests_dir.join("test_derived.py");
        fs::write(&derived_path, derived_source).unwrap();

        let config = TestDiscoveryConfig::default();
        let mut module_resolver =
            crate::python_discovery::ModuleResolver::new(temp_dir.path()).unwrap();
        let module_path = vec!["tests".to_string(), "test_derived".to_string()];
        let mut discovery = SemanticTestDiscovery::new(config);

        let (tests, _warnings) = discovery
            .discover_tests(
                &derived_path,
                derived_source,
                module_path,
                &mut module_resolver,
            )
            .unwrap();

        // Should have (matching pytest behavior):
        // - TestBase (imported): 2 parametrized tests with [1], [2]
        // - TestDerived: 3 overridden parametrized tests with [10], [20], [30]
        // Total: 5 tests
        assert_eq!(tests.len(), 5);

        // TestDerived should have the overridden parameters
        let derived_tests: Vec<_> = tests
            .iter()
            .filter(|t| t.class_name.as_ref().map_or(false, |c| c == "TestDerived"))
            .collect();
        assert_eq!(derived_tests.len(), 3);
        assert!(tests.iter().any(|t| t.name == "test_param[10]"));
        assert!(tests.iter().any(|t| t.name == "test_param[20]"));
        assert!(tests.iter().any(|t| t.name == "test_param[30]"));

        // TestBase (imported) should have its original parameters
        let base_tests: Vec<_> = tests
            .iter()
            .filter(|t| t.class_name.as_ref().map_or(false, |c| c == "TestBase"))
            .collect();
        assert_eq!(base_tests.len(), 2);
        assert!(tests.iter().any(|t| t.name == "test_param[1]"));
        assert!(tests.iter().any(|t| t.name == "test_param[2]"));
    }

    #[test]
    fn test_mixed_parametrized_and_regular_methods() {
        let source = r#"
import pytest

class TestMixed:
    def test_regular(self):
        pass
    
    @pytest.mark.parametrize("value", [1, 2])
    def test_param(self, value):
        assert value > 0
    
    def test_another_regular(self):
        pass
"#;

        let temp_dir = TempDir::new().unwrap();
        let test_path = temp_dir.path().join("test_mixed.py");
        fs::write(&test_path, source).unwrap();

        let config = TestDiscoveryConfig::default();
        let mut module_resolver =
            crate::python_discovery::ModuleResolver::new(temp_dir.path()).unwrap();
        let module_path = vec!["test_mixed".to_string()];
        let mut discovery = SemanticTestDiscovery::new(config);

        let (tests, _warnings) = discovery
            .discover_tests(&test_path, source, module_path, &mut module_resolver)
            .unwrap();

        assert_eq!(tests.len(), 4); // 2 regular + 2 parametrized
        assert!(tests.iter().any(|t| t.name == "test_regular"));
        assert!(tests.iter().any(|t| t.name == "test_param[1]"));
        assert!(tests.iter().any(|t| t.name == "test_param[2]"));
        assert!(tests.iter().any(|t| t.name == "test_another_regular"));
    }
}
