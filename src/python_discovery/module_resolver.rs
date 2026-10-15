//! Module resolver for Python imports.
//!
//! This module provides functionality to resolve Python module imports
//! to actual file paths and load their contents.

use crate::collection::error::{CollectionError, CollectionResult};
use ruff_python_ast::ModModule;
use ruff_python_parser::parse_module;
use ruff_python_stdlib::sys::is_builtin_module;
use std::collections::{hash_map::Entry, HashMap};
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
    pub fn new(project_root: &Path) -> Self {
        let mut search_paths = Vec::new();

        // Add PYTHONPATH from environment first (takes precedence)
        search_paths.extend(get_pythonpath_from_env());

        // Then add the project root
        search_paths.push(project_root.to_path_buf());

        Self {
            search_paths,
            cache: HashMap::new(),
        }
    }

    /// Copy of this resolver's search paths with an empty module cache.
    pub fn with_search_paths(&self) -> Self {
        Self {
            search_paths: self.search_paths.clone(),
            cache: HashMap::new(),
        }
    }

    /// Resolve a module path to a file and load it
    pub fn resolve_and_load(&mut self, module_path: &[String]) -> CollectionResult<&ParsedModule> {
        match self.cache.entry(module_path.to_vec()) {
            Entry::Occupied(entry) => Ok(entry.into_mut()),
            Entry::Vacant(entry) => {
                let path = Self::resolve_with_search_paths(&self.search_paths, module_path)?;
                Ok(entry.insert(Self::load_module(&path)?))
            }
        }
    }

    /// Resolve module using search paths
    fn resolve_with_search_paths(
        search_paths: &[PathBuf],
        module_path: &[String],
    ) -> CollectionResult<PathBuf> {
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

        search_paths
            .iter()
            .flat_map(|root| {
                let base = root.join(module_path.iter().collect::<PathBuf>());
                [base.join("__init__.py"), base.with_extension("py")]
            })
            .find(|path| std::fs::metadata(path).is_ok())
            .ok_or_else(|| {
                CollectionError::ImportError(format!("Could not find module: {}", module_name))
            })
    }

    /// Load and parse a Python module
    fn load_module(path: &Path) -> CollectionResult<ParsedModule> {
        let source = std::fs::read_to_string(path)?;
        let parsed = parse_module(&source).map_err(|e| {
            CollectionError::ParseError(format!("Failed to parse {}: {:?}", path.display(), e))
        })?;

        Ok(ParsedModule {
            path: path.to_path_buf(),
            source,
            module: parsed.into_syntax(),
        })
    }

    /// Clear the module cache
    pub fn clear_cache(&mut self) {
        self.cache.clear();
    }
}

/// Resolve a relative import to an absolute module path.
///
/// `relative_level` is 0 for absolute imports, 1 for `.`, 2 for `..`, etc.
pub(crate) fn resolve_relative_import(
    current_module_path: &[String],
    relative_level: usize,
    module_parts: &[String],
) -> Option<Vec<String>> {
    if relative_level == 0 {
        return Some(module_parts.to_vec());
    }
    if current_module_path.len() < relative_level {
        return None;
    }
    let mut result = current_module_path[..current_module_path.len() - relative_level].to_vec();
    result.extend_from_slice(module_parts);
    Some(result)
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

        let mut resolver = ModuleResolver::new(root);

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
}
