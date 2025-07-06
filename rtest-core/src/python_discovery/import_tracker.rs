//! Tracks Python imports to resolve decorators correctly

use std::collections::HashMap;

/// Tracks imports and their aliases in a Python module
#[derive(Debug, Default)]
pub struct ImportTracker {
    /// Maps local names to their full module paths
    /// e.g., "pt" -> "pytest", "mark" -> "pytest.mark"
    imports: HashMap<String, String>,
}

impl ImportTracker {
    pub fn new() -> Self {
        Self {
            imports: HashMap::new(),
        }
    }

    /// Record an import statement
    pub fn add_import(&mut self, alias: &str, module_path: &str) {
        self.imports.insert(alias.to_string(), module_path.to_string());
    }

    /// Resolve a name to its full module path
    pub fn resolve(&self, name: &str) -> Option<&str> {
        self.imports.get(name).map(|s| s.as_str())
    }

    /// Check if a name chain resolves to pytest
    pub fn is_pytest(&self, name: &str) -> bool {
        if name == "pytest" {
            return true;
        }
        if let Some(resolved) = self.resolve(name) {
            return resolved == "pytest";
        }
        false
    }

    /// Check if a name chain resolves to pytest.mark
    pub fn is_pytest_mark(&self, name: &str) -> bool {
        if name == "mark" {
            // Could be from "from pytest import mark"
            if let Some(resolved) = self.resolve(name) {
                return resolved == "pytest.mark";
            }
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_import_tracking() {
        let mut tracker = ImportTracker::new();
        
        // import pytest as pt
        tracker.add_import("pt", "pytest");
        assert_eq!(tracker.resolve("pt"), Some("pytest"));
        assert!(tracker.is_pytest("pt"));
        
        // from pytest import mark
        tracker.add_import("mark", "pytest.mark");
        assert_eq!(tracker.resolve("mark"), Some("pytest.mark"));
        assert!(tracker.is_pytest_mark("mark"));
    }

    #[test]
    fn test_no_alias_resolution() {
        let tracker = ImportTracker::new();
        assert!(tracker.is_pytest("pytest"));
        assert!(!tracker.is_pytest("unknown"));
        assert_eq!(tracker.resolve("unknown"), None);
    }
}