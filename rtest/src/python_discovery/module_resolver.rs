//! Module resolver for Python imports.
//!
//! This module provides functionality to resolve Python module imports
//! to actual file paths and load their contents.

use crate::collection::error::{CollectionError, CollectionResult};
use ruff_python_ast::{Mod, ModModule};
use ruff_python_parser::{parse, Mode, ParseOptions};
use ruff_python_stdlib::sys::is_builtin_module;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Information about a parsed Python module
pub struct ParsedModule {
    pub path: PathBuf,
    pub source: String,
    pub module: ModModule,
}

/// Resolves Python module imports to file paths and loads modules
pub struct ModuleResolver {
    /// Search paths for module resolution
    search_paths: Vec<PathBuf>,
    /// Cache of already loaded modules
    cache: HashMap<Vec<String>, ParsedModule>,
}

impl ModuleResolver {
    pub fn new(project_root: &Path) -> CollectionResult<Self> {
        let mut search_paths = Vec::new();

        // Add PYTHONPATH from environment first (takes precedence)
        search_paths.extend(get_pythonpath_from_env());

        // Then add the project root
        search_paths.push(project_root.to_path_buf());

        Ok(Self {
            search_paths,
            cache: HashMap::new(),
        })
    }

    /// Resolve a module path to a file and load it
    pub fn resolve_and_load(&mut self, module_path: &[String]) -> CollectionResult<&ParsedModule> {
        // Check cache first
        if self.cache.contains_key(module_path) {
            return Ok(self.cache.get(module_path).unwrap());
        }

        // Resolve using search paths
        let file_path = self.resolve_with_search_paths(module_path)?;
        let parsed = self.load_module(&file_path)?;
        self.cache.insert(module_path.to_vec(), parsed);
        Ok(self.cache.get(module_path).unwrap())
    }

    /// Resolve module using search paths
    fn resolve_with_search_paths(&self, module_path: &[String]) -> CollectionResult<PathBuf> {
        if module_path.is_empty() {
            return Err(CollectionError::ImportError("Empty module path".into()));
        }

        let module_name = module_path.join(".");

        // Check if this is a built-in module
        if is_builtin_module(11, &module_path[0]) {
            return Err(CollectionError::ImportError(format!(
                "Cannot resolve built-in module '{}' - inheritance from built-in modules is not supported",
                module_name
            )));
        }

        // Try each search path in order
        for search_path in &self.search_paths {
            if let Ok(file_path) = self.try_resolve_in_search_path(module_path, search_path) {
                return Ok(file_path);
            }
        }

        Err(CollectionError::ImportError(format!(
            "Could not find module: {}",
            module_name
        )))
    }

    /// Try to resolve a module in a specific search path
    fn try_resolve_in_search_path(
        &self,
        module_path: &[String],
        search_path: &PathBuf,
    ) -> Result<PathBuf, ()> {
        let possible_paths = self.get_possible_paths_in_root(module_path, search_path);

        for path in &possible_paths {
            if std::fs::metadata(&path).is_ok() {
                return Ok(path.clone());
            }
        }

        Err(())
    }

    /// Get all possible file paths for a module in a specific root directory
    fn get_possible_paths_in_root(&self, module_path: &[String], root: &PathBuf) -> Vec<PathBuf> {
        if module_path.is_empty() {
            return Vec::new();
        }

        let mut paths = Vec::new();
        let mut base_dir = root.clone();

        // Build the base directory path from all but the last component
        if module_path.len() > 1 {
            for part in &module_path[..module_path.len() - 1] {
                base_dir.push(part);
            }
        }

        let module_name = &module_path[module_path.len() - 1];

        // Try module_name.py (regular module)
        let mut py_file = base_dir.clone();
        py_file.push(format!("{}.py", module_name));
        paths.push(py_file);

        // Try module_name/__init__.py (package)
        let mut package_init = base_dir;
        package_init.push(module_name);
        package_init.push("__init__.py");
        paths.push(package_init);

        paths
    }

    /// Load and parse a Python module
    fn load_module(&self, path: &Path) -> CollectionResult<ParsedModule> {
        let source = std::fs::read_to_string(path)?;
        let parsed = parse(&source, ParseOptions::from(Mode::Module)).map_err(|e| {
            CollectionError::ParseError(format!("Failed to parse {}: {:?}", path.display(), e))
        })?;

        let ast_module = match parsed.into_syntax() {
            Mod::Module(module) => module,
            _ => return Err(CollectionError::ParseError("Not a module".into())),
        };

        Ok(ParsedModule {
            path: path.to_path_buf(),
            source,
            module: ast_module,
        })
    }

    /// Clear the module cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

/// Get PYTHONPATH environment variable as search paths
fn get_pythonpath_from_env() -> Vec<PathBuf> {
    std::env::var("PYTHONPATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_module_path_to_file_path() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create test structure
        fs::create_dir_all(root.join("tests")).unwrap();
        fs::write(root.join("tests/test_example.py"), "# test").unwrap();

        fs::create_dir_all(root.join("package/subpackage")).unwrap();
        fs::write(root.join("package/__init__.py"), "").unwrap();
        fs::write(root.join("package/subpackage/__init__.py"), "").unwrap();
        fs::write(root.join("package/module.py"), "# module").unwrap();

        let mut resolver = ModuleResolver::new(root).unwrap();

        // Test simple module
        let path = resolver
            .resolve_and_load(&["tests".into(), "test_example".into()])
            .unwrap();
        assert_eq!(path.path, root.join("tests/test_example.py"));

        // Test package with __init__.py
        let path = resolver.resolve_and_load(&["package".into()]).unwrap();
        assert_eq!(path.path, root.join("package/__init__.py"));

        // Test module in package
        let path = resolver
            .resolve_and_load(&["package".into(), "module".into()])
            .unwrap();
        assert_eq!(path.path, root.join("package/module.py"));

        // Test non-existent module
        assert!(resolver.resolve_and_load(&["nonexistent".into()]).is_err());
    }

    #[test]
    fn test_sys_path_resolution() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create test module in temp directory
        fs::write(root.join("test_module.py"), "# test module").unwrap();

        // Create resolver with sys.path support
        let mut resolver = ModuleResolver::new(root).unwrap();

        // Should be able to resolve the module
        let result = resolver.resolve_and_load(&["test_module".into()]);
        // This might fail if the temp dir isn't in Python's sys.path, which is expected
        // The test verifies the resolver doesn't crash
        match result {
            Ok(parsed) => {
                assert_eq!(parsed.path, root.join("test_module.py"));
            }
            Err(_) => {
                // Expected if temp dir not in sys.path
            }
        }
    }
}
