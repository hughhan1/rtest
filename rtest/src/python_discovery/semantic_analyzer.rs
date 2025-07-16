//! Semantic analysis for test discovery with import resolution.

use ruff_python_ast::{Expr, Mod, ModModule, Stmt, StmtClassDef, StmtImportFrom};
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
    #[allow(dead_code)]
    local_name: String,
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
                    all_tests.push(TestInfo {
                        name: func.name.to_string(),
                        line: func.range().start().to_u32() as usize,
                        is_method: false,
                        class_name: None,
                    });
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
                Stmt::ImportFrom(StmtImportFrom {
                    module: Some(module_name),
                    names,
                    ..
                }) => {
                    let module_path: Vec<String> =
                        module_name.split('.').map(String::from).collect();

                    for alias in names {
                        let imported_name = alias.name.to_string();
                        let local_name =
                            Self::get_alias_name_or_default(alias.asname.as_ref(), &imported_name);

                        imports.insert(
                            local_name.clone(),
                            ImportInfo {
                                module_path: module_path.clone(),
                                imported_name,
                                local_name,
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
                                local_name: alias.name.to_string(),
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
                    methods.push(TestMethodInfo {
                        name: method_name.to_string(),
                        line: func.range().start().to_u32() as usize,
                    });
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
                match self.resolve_base_class(
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
                ) {
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
                match self.resolve_base_class(base_expr, current_module_path, imports, semantic) {
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
                    all_tests.push(TestInfo {
                        name: method_name.to_string(),
                        line: func.range().start().to_u32() as usize,
                        is_method: true,
                        class_name: Some(class_name.to_string()),
                    });
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
                match self.resolve_base_class(
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
                ) {
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
    ) -> Option<ResolvedBaseClass> {
        match base_expr {
            Expr::Name(name_expr) => {
                let name = name_expr.id.as_str();
                // Check if it's an imported class
                if let Some(import_info) = imports.get(name) {
                    return Some(ResolvedBaseClass {
                        module_path: import_info.module_path.clone(),
                        class_name: import_info.imported_name.clone(),
                        is_local: false,
                    });
                }

                // Otherwise, it's a local class
                Some(ResolvedBaseClass {
                    module_path: current_module_path.to_vec(),
                    class_name: name.to_string(),
                    is_local: true,
                })
            }
            Expr::Attribute(attr_expr) => {
                // Handle module.Class pattern
                if let Expr::Name(module_name) = &*attr_expr.value {
                    if let Some(import_info) = imports.get(module_name.id.as_str()) {
                        return Some(ResolvedBaseClass {
                            module_path: import_info.module_path.clone(),
                            class_name: attr_expr.attr.to_string(),
                            is_local: false,
                        });
                    }
                }
                None
            }
            _ => None,
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
