//! Collection configuration.

use std::path::PathBuf;

/// Configuration for collection
#[derive(Debug, Clone)]
pub struct CollectionConfig {
    pub ignore_patterns: Vec<String>,
    #[allow(dead_code)]
    pub ignore_glob_patterns: Vec<String>,
    pub norecursedirs: Vec<String>,
    pub testpaths: Vec<PathBuf>,
    pub python_files: Vec<String>,
    pub python_classes: Vec<String>,
    pub python_functions: Vec<String>,
}

impl Default for CollectionConfig {
    fn default() -> Self {
        Self {
            ignore_patterns: vec![],
            ignore_glob_patterns: vec![],
            norecursedirs: vec![
                "*.egg".into(),
                ".*".into(),
                "_darcs".into(),
                "build".into(),
                "CVS".into(),
                "dist".into(),
                "node_modules".into(),
                "venv".into(),
                "{arch}".into(),
            ],
            testpaths: vec![],
            python_files: vec!["test_*.py".into(), "*_test.py".into()],
            python_classes: vec!["Test*".into()],
            python_functions: vec!["test*".into()],
        }
    }
}
