//! Semantic analysis for test discovery with import resolution.

use ruff_python_ast::{Expr, ModModule, Stmt, StmtClassDef};
use ruff_python_parser::parse_module;
use ruff_python_stdlib::sys::is_known_standard_library;
use ruff_text_size::Ranged;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::rc::Rc;

use crate::collection::error::{CollectionError, CollectionResult, CollectionWarning};
use crate::python_discovery::{
    cases::{
        combine_and_expand_specs, parse_decorators_for_cases, parse_decorators_to_specs,
        MethodCasesInfo,
    },
    constant_resolver::ConstantResolver,
    discovery::{class_has_init, collect_test_methods, TestDiscoveryConfig, TestInfo},
    module_resolver::ModuleResolver,
};

/// Information about an import
#[derive(Debug, Clone)]
struct ImportInfo {
    module_path: Vec<String>,
    imported_name: Option<String>,
    /// The level of relative import (0 for absolute, 1 for '.', 2 for '..', etc.)
    relative_level: usize,
}

/// Resolved base class information
#[derive(Debug, Clone)]
pub struct ResolvedBaseClass {
    module_path: Vec<String>,
    class_name: String,
    // Local references use the current discovery cache; imported aliases may
    // require loading additional classes from the same module.
    is_local: bool,
}

/// Lightweight test method information for caching.
///
/// Stores method-level specs (not expanded) so they can be combined with
/// child class decorators during inheritance.
#[derive(Debug, Clone)]
pub struct TestMethodInfo {
    pub name: String,
    /// Method-level decorator specs (not yet combined with class decorators)
    pub method_specs: MethodCasesInfo,
}

/// Test class information including methods
#[derive(Debug, Clone)]
pub struct TestClassInfo {
    pub name: String,
    pub has_init: bool,
    pub test_methods: Vec<TestMethodInfo>,
    pub base_classes: Vec<ResolvedBaseClass>,
    /// Class-level decorator specs
    pub class_specs: MethodCasesInfo,
}

/// Enhanced test discovery with semantic analysis
pub struct SemanticTestDiscovery {
    config: TestDiscoveryConfig,
    /// Cache of test classes by module path and class name
    class_cache: HashMap<(Vec<String>, String), Rc<TestClassInfo>>,
    loaded_modules: HashSet<Vec<String>>,
    /// Warnings collected during discovery
    warnings: Vec<CollectionWarning>,
    /// Current file path for warning generation
    current_file_path: String,
}

impl SemanticTestDiscovery {
    pub fn new(config: TestDiscoveryConfig) -> Self {
        Self {
            config,
            class_cache: HashMap::new(),
            loaded_modules: HashSet::new(),
            warnings: Vec::new(),
            current_file_path: String::new(),
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
        self.current_file_path = path.to_string_lossy().into_owned();

        // Parse the module
        let parsed = parse_module(source).map_err(|e| {
            CollectionError::ParseError(format!("Failed to parse {}: {:?}", path.display(), e))
        })?;

        let ast_module = parsed.into_syntax();

        // Build constant resolver for this module
        let resolver = ConstantResolver::from_module_with_imports(
            &ast_module,
            &module_path,
            module_resolver.with_search_paths(),
            path,
        );

        // First pass: collect imports
        let imports = self.collect_imports(&ast_module);

        // Second pass: collect test classes
        let classes = self.collect_test_classes(&ast_module, &module_path, &imports, &resolver)?;

        // Third pass: resolve inheritance and collect all tests
        let mut all_tests = Vec::new();

        // Collect module-level test functions
        for stmt in &ast_module.body {
            if let Stmt::FunctionDef(func) = stmt {
                if self.config.is_test_function(func.name.as_str()) {
                    let cases_expansion =
                        parse_decorators_for_cases(&func.decorator_list, &resolver, None);
                    all_tests.push(TestInfo {
                        name: func.name.to_string(),
                        class_name: None,
                        cases_expansion,
                    });
                }
            }
        }

        // Collect test methods from classes
        for (class_def, info) in classes {
            all_tests.extend(self.collect_class_tests(class_def, &info, module_resolver)?);
        }

        Ok((all_tests, std::mem::take(&mut self.warnings)))
    }

    /// Collect imports from the module.
    fn collect_imports(&self, module: &ModModule) -> HashMap<String, ImportInfo> {
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
                                imported_name: Some(imported_name),
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
                                imported_name: None,
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
    fn collect_test_classes<'a>(
        &mut self,
        module: &'a ModModule,
        module_path: &[String],
        imports: &HashMap<String, ImportInfo>,
        resolver: &ConstantResolver,
    ) -> CollectionResult<Vec<(&'a StmtClassDef, Rc<TestClassInfo>)>> {
        let mut classes = Vec::new();
        for stmt in &module.body {
            if let Stmt::ClassDef(class_def) = stmt {
                let class_name = class_def.name.as_str();

                if self.config.is_test_class(class_name) {
                    let info =
                        Rc::new(self.class_info(class_def, module_path, imports, resolver)?);
                    self.class_cache.insert(
                        (module_path.to_vec(), class_name.to_string()),
                        Rc::clone(&info),
                    );
                    classes.push((class_def, info));
                }
            }
        }

        Ok(classes)
    }

    fn class_info(
        &self,
        class_def: &StmtClassDef,
        module_path: &[String],
        imports: &HashMap<String, ImportInfo>,
        resolver: &ConstantResolver,
    ) -> CollectionResult<TestClassInfo> {
        Ok(TestClassInfo {
            name: class_def.name.to_string(),
            has_init: class_has_init(class_def),
            test_methods: collect_test_methods(class_def, &self.config, resolver),
            class_specs: parse_decorators_to_specs(
                &class_def.decorator_list,
                resolver,
                Some(class_def.name.as_str()),
            ),
            base_classes: self.collect_base_class_names(class_def, module_path, imports)?,
        })
    }

    /// Collect base class names from a class definition
    fn collect_base_class_names(
        &self,
        class_def: &StmtClassDef,
        current_module_path: &[String],
        imports: &HashMap<String, ImportInfo>,
    ) -> CollectionResult<Vec<ResolvedBaseClass>> {
        let mut base_classes = Vec::new();

        for base in class_def.arguments.iter().flat_map(|args| args.args.iter()) {
            base_classes.extend(self.resolve_base_class(base, current_module_path, imports)?);
        }

        Ok(base_classes)
    }

    /// Collect all tests from a class, including inherited ones
    fn collect_class_tests(
        &mut self,
        class_def: &StmtClassDef,
        info: &TestClassInfo,
        module_resolver: &mut ModuleResolver,
    ) -> CollectionResult<Vec<TestInfo>> {
        let class_name = class_def.name.as_str();

        // Check if this class or any parent has __init__
        if self.should_skip_class(info, module_resolver, &mut HashSet::new())? {
            // Add warning about skipped class
            let warning = CollectionWarning {
                file_path: self.current_file_path.clone(),
                line: class_def.range().start().to_u32() as usize + 1, // Convert 0-based to 1-based
                message: format!(
                    "cannot collect test class '{}' because it has a __init__ constructor",
                    class_name
                ),
            };
            self.warnings.push(warning);
            return Ok(vec![]);
        }

        let class_specs = &info.class_specs;
        let own_methods = &info.test_methods;
        let own_method_names: HashSet<&str> = own_methods
            .iter()
            .map(|method| method.name.as_str())
            .collect();

        let mut inherited_methods = Vec::new();

        // Collect inherited methods
        for base in &info.base_classes {
            inherited_methods.extend(
                self.get_base_class_methods(base, module_resolver, &mut HashSet::new())?
                    .into_iter()
                    .filter(|method| !own_method_names.contains(method.name.as_str())),
            );
        }

        Ok(inherited_methods
            .into_iter()
            .chain(own_methods.iter().cloned())
            .map(|method| {
                let cases_expansion = combine_and_expand_specs(class_specs, &method.method_specs);
                TestInfo {
                    name: method.name,
                    class_name: Some(class_name.to_string()),
                    cases_expansion,
                }
            })
            .collect())
    }

    /// Check if a class should be skipped (has __init__ or inherits from class with __init__)
    fn should_skip_class(
        &mut self,
        info: &TestClassInfo,
        module_resolver: &mut ModuleResolver,
        visited: &mut HashSet<(Vec<String>, String)>,
    ) -> CollectionResult<bool> {
        // Check if this class has __init__
        if info.has_init {
            return Ok(true);
        }

        // Check parent classes
        for base in &info.base_classes {
            if self.base_class_has_init(base, module_resolver, visited)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Check if a base class has __init__ (recursively checking ancestors)
    fn base_class_has_init(
        &mut self,
        resolved: &ResolvedBaseClass,
        module_resolver: &mut ModuleResolver,
        visited: &mut HashSet<(Vec<String>, String)>,
    ) -> CollectionResult<bool> {
        if self.is_stdlib_module_to_skip(&resolved.module_path) {
            return Ok(false);
        }

        // Check for cycles
        let key = (resolved.module_path.clone(), resolved.class_name.clone());
        if !visited.insert(key.clone()) {
            // Cycle detected - assume no init to break the cycle
            return Ok(false);
        }

        let has_init = match self.load_base_class(resolved, module_resolver)? {
            Some(info) => self.should_skip_class(&info, module_resolver, visited)?,
            None => false,
        };

        // Remove from visited after processing to allow sibling branches in diamond
        // inheritance patterns to revisit this class without false cycle detection
        visited.remove(&key);
        Ok(has_init)
    }

    fn load_base_class(
        &mut self,
        resolved: &ResolvedBaseClass,
        module_resolver: &mut ModuleResolver,
    ) -> CollectionResult<Option<Rc<TestClassInfo>>> {
        if !resolved.is_local {
            self.ensure_module_loaded(&resolved.module_path, module_resolver)?;
        }
        Ok(self
            .class_cache
            .get(&(resolved.module_path.clone(), resolved.class_name.clone()))
            .cloned())
    }

    /// Get inherited test methods, detecting cycles along the current ancestry path.
    fn get_base_class_methods(
        &mut self,
        resolved: &ResolvedBaseClass,
        module_resolver: &mut ModuleResolver,
        visited: &mut HashSet<(Vec<String>, String)>,
    ) -> CollectionResult<Vec<TestMethodInfo>> {
        if self.is_stdlib_module_to_skip(&resolved.module_path) {
            return Ok(vec![]);
        }

        // Check for cycles
        let key = (resolved.module_path.clone(), resolved.class_name.clone());
        if !visited.insert(key.clone()) {
            // Cycle detected - this is an error condition
            return Err(CollectionError::ParseError(format!(
                "Circular inheritance detected in class hierarchy involving '{}'",
                resolved.class_name
            )));
        }

        let mut all_methods = Vec::new();
        if let Some(base_info) = self.load_base_class(resolved, module_resolver)? {
            // First collect inherited methods, then this class's own methods.
            for base_class in &base_info.base_classes {
                all_methods.extend(self.get_base_class_methods(
                    base_class,
                    module_resolver,
                    visited,
                )?);
            }
            all_methods.extend(base_info.test_methods.clone());
        }

        // Remove from visited after processing to allow sibling branches in diamond
        // inheritance patterns to revisit this class without false cycle detection
        visited.remove(&key);

        Ok(all_methods)
    }

    /// Ensure a module is loaded and its test classes are cached
    fn ensure_module_loaded(
        &mut self,
        module_path: &[String],
        module_resolver: &mut ModuleResolver,
    ) -> CollectionResult<()> {
        // Check if we've already loaded this module
        if self.loaded_modules.contains(module_path) {
            return Ok(());
        }

        let constant_loader = module_resolver.with_search_paths();
        match module_resolver.resolve_and_load(module_path) {
            Ok(parsed_module) => {
                // Build resolver for constants in this external module
                let external_resolver = ConstantResolver::from_module_with_imports(
                    &parsed_module.module,
                    module_path,
                    constant_loader,
                    &parsed_module.path,
                );
                let imports = self.collect_imports(&parsed_module.module);

                // Extract test class information without storing the module
                for stmt in &parsed_module.module.body {
                    if let Stmt::ClassDef(class_def) = stmt {
                        let class_name = class_def.name.as_str();

                        // Always collect class info for external modules, even if they don't match test patterns
                        // They might be used as base classes
                        let info =
                            self.class_info(class_def, module_path, &imports, &external_resolver)?;

                        self.class_cache.insert(
                            (module_path.to_vec(), class_name.to_string()),
                            Rc::new(info),
                        );
                    }
                }
            }
            // External or unresolvable modules are marked as attempted below.
            Err(CollectionError::ImportError(_)) => {}
            Err(e) => return Err(e),
        }

        // Mark module as loaded
        self.loaded_modules.insert(module_path.to_vec());

        Ok(())
    }

    /// Resolve a base class expression to module path and class name
    fn resolve_base_class(
        &self,
        base_expr: &Expr,
        current_module_path: &[String],
        imports: &HashMap<String, ImportInfo>,
    ) -> CollectionResult<Option<ResolvedBaseClass>> {
        let (import_info, class_name) = match base_expr {
            Expr::Name(name_expr) => {
                let name = name_expr.id.as_str();
                let Some(import_info) = imports.get(name) else {
                    return Ok(Some(ResolvedBaseClass {
                        module_path: current_module_path.to_vec(),
                        class_name: name.to_string(),
                        is_local: true,
                    }));
                };
                let Some(class_name) = &import_info.imported_name else {
                    return Ok(None);
                };
                (import_info, class_name.clone())
            }
            Expr::Attribute(attr_expr) => {
                let Expr::Name(module_name) = &*attr_expr.value else {
                    return Ok(None);
                };
                let Some(import_info) = imports.get(module_name.id.as_str()) else {
                    return Ok(None);
                };
                (import_info, attr_expr.attr.to_string())
            }
            Expr::Subscript(subscript_expr) => {
                return self.resolve_base_class(
                    &subscript_expr.value,
                    current_module_path,
                    imports,
                );
            }
            _ => return Ok(None),
        };

        Ok(Some(ResolvedBaseClass {
            module_path: Self::resolve_relative_module_path(
                current_module_path,
                import_info.relative_level,
                &import_info.module_path,
            )?,
            class_name,
            is_local: false,
        }))
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

    /// Check if a module should be skipped for inheritance analysis.
    ///
    /// Uses ruff's comprehensive stdlib detection to skip all standard library modules,
    /// since the module resolver can only resolve project-local modules.
    fn is_stdlib_module_to_skip(&self, module_path: &[String]) -> bool {
        module_path
            .first()
            .is_some_and(|name| is_known_standard_library(11, name))
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
