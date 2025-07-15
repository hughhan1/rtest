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
    /// Root directory to search for modules
    root_path: PathBuf,
    /// Cache of already loaded modules
    cache: HashMap<Vec<String>, ParsedModule>,
}

impl ModuleResolver {
    pub fn new(root_path: PathBuf) -> Self {
        Self {
            root_path,
            cache: HashMap::new(),
        }
    }

    /// Resolve a module path to a file and load it
    pub fn resolve_and_load(&mut self, module_path: &[String]) -> CollectionResult<&ParsedModule> {
        // Check cache first
        if self.cache.contains_key(module_path) {
            return Ok(self.cache.get(module_path).unwrap());
        }

        // Convert module path to file path
        let file_path = self.module_path_to_file_path(module_path)?;

        // Load and parse the module
        let parsed = self.load_module(&file_path)?;

        // Cache and return
        self.cache.insert(module_path.to_vec(), parsed);
        // Safe to unwrap here since we just inserted the value
        Ok(self
            .cache
            .get(module_path)
            .expect("Just inserted value should exist"))
    }

    /// Convert a module path like ["tests", "test_example"] to a file path
    fn module_path_to_file_path(&self, module_path: &[String]) -> CollectionResult<PathBuf> {
        if module_path.is_empty() {
            return Err(CollectionError::ImportError("Empty module path".into()));
        }

        let module_name = &module_path[0];

        // Check if this is a built-in module (compiled into interpreter)
        // Use Python 3.11 as a reasonable default version
        if is_builtin_module(11, module_name) {
            return Err(CollectionError::ImportError(format!(
                "Cannot resolve built-in module '{}' - inheritance from built-in modules is not supported",
                module_path.join(".")
            )));
        }

        // Try different possible file paths by attempting to read them
        let possible_paths = self.get_possible_paths(module_path);
        for path in &possible_paths {
            // Try to read the file directly instead of checking existence first
            // This avoids TOCTOU issues and is more efficient
            if std::fs::metadata(&path).is_ok() {
                return Ok(path.clone());
            }
        }

        Err(CollectionError::ImportError(format!(
            "Could not find module: {}",
            module_path.join(".")
        )))
    }

    /// Get all possible file paths for a module following Python import semantics
    fn get_possible_paths(&self, module_path: &[String]) -> Vec<PathBuf> {
        if module_path.is_empty() {
            return Vec::new();
        }

        let mut paths = Vec::new();

        // Build the base directory path from all but the last component
        let mut base_dir = self.root_path.clone();
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

        let resolver = ModuleResolver::new(root.to_path_buf());

        // Test simple module
        let path = resolver
            .module_path_to_file_path(&["tests".into(), "test_example".into()])
            .unwrap();
        assert_eq!(path, root.join("tests/test_example.py"));

        // Test package with __init__.py
        let path = resolver
            .module_path_to_file_path(&["package".into()])
            .unwrap();
        assert_eq!(path, root.join("package/__init__.py"));

        // Test module in package
        let path = resolver
            .module_path_to_file_path(&["package".into(), "module".into()])
            .unwrap();
        assert_eq!(path, root.join("package/module.py"));

        // Test non-existent module
        assert!(resolver
            .module_path_to_file_path(&["nonexistent".into()])
            .is_err());
    }
}
