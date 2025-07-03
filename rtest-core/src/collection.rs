//! Pytest collection implementation in Rust.
//!
//! This module implements pytest's collection logic, including:
//! - File system traversal
//! - Python file parsing
//! - Test discovery
//! - Collection reporting

use crate::python_discovery::{discover_tests, test_info_to_function, TestDiscoveryConfig};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

/// Result type for collection operations
pub type CollectionResult<T> = Result<T, CollectionError>;

/// Collection-specific errors
#[derive(Debug)]
#[allow(dead_code, clippy::enum_variant_names)]
pub enum CollectionError {
    IoError(std::io::Error),
    ParseError(String),
    ImportError(String),
    SkipError(String),
}

impl fmt::Display for CollectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {e}"),
            Self::ParseError(e) => write!(f, "Parse error: {e}"),
            Self::ImportError(e) => write!(f, "Import error: {e}"),
            Self::SkipError(e) => write!(f, "Skip: {e}"),
        }
    }
}

impl std::error::Error for CollectionError {}

impl From<std::io::Error> for CollectionError {
    fn from(err: std::io::Error) -> Self {
        CollectionError::IoError(err)
    }
}

/// Outcome of a collection operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CollectionOutcome {
    Passed,
    Failed,
    Skipped,
}

/// Location information for a test item
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Location {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub name: String,
}

/// Base trait for all collectible nodes
pub trait Collector: std::fmt::Debug {
    /// Unique identifier for this node
    fn nodeid(&self) -> &str;

    /// Parent collector, if any
    #[allow(dead_code)]
    fn parent(&self) -> Option<&dyn Collector>;

    /// Collect child nodes
    fn collect(&self) -> CollectionResult<Vec<Box<dyn Collector>>>;

    /// Get the path associated with this collector
    #[allow(dead_code)]
    fn path(&self) -> &Path;

    /// Check if this is a test item (leaf node)
    fn is_item(&self) -> bool {
        false
    }
}

/// Root of the collection tree
#[derive(Debug)]
pub struct Session {
    pub rootpath: PathBuf,
    pub nodeid: String,
    pub config: CollectionConfig,
    #[allow(dead_code)]
    cache: HashMap<PathBuf, Vec<Box<dyn Collector>>>,
}

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
                "*.egg".to_string(),
                ".*".to_string(),
                "_darcs".to_string(),
                "build".to_string(),
                "CVS".to_string(),
                "dist".to_string(),
                "node_modules".to_string(),
                "venv".to_string(),
                "{arch}".to_string(),
            ],
            testpaths: vec![],
            python_files: vec!["test_*.py".to_string(), "*_test.py".to_string()],
            python_classes: vec!["Test*".to_string()],
            python_functions: vec!["test*".to_string()],
        }
    }
}

impl Session {
    pub fn new(rootpath: PathBuf) -> Self {
        Self {
            nodeid: String::new(),
            rootpath,
            config: CollectionConfig::default(),
            cache: HashMap::new(),
        }
    }

    pub fn perform_collect(
        &mut self,
        args: &[String],
    ) -> CollectionResult<Vec<Box<dyn Collector>>> {
        let paths = if args.is_empty() {
            // Use testpaths from config or current directory
            if self.config.testpaths.is_empty() {
                vec![self.rootpath.clone()]
            } else {
                self.config.testpaths.clone()
            }
        } else {
            // Parse arguments into paths
            args.iter()
                .map(|arg| {
                    let path = PathBuf::from(arg);
                    if path.is_absolute() {
                        path
                    } else {
                        self.rootpath.join(arg)
                    }
                })
                .collect()
        };

        let mut items = Vec::new();
        for path in paths {
            if let Ok(collected) = self.collect_path(&path) {
                items.extend(collected);
            }
        }

        Ok(items)
    }

    fn collect_path(&self, path: &Path) -> CollectionResult<Vec<Box<dyn Collector>>> {
        if self.should_ignore_path(path)? {
            return Ok(vec![]);
        }

        if path.is_dir() {
            let dir = Directory::new(path.to_path_buf(), self);
            Ok(vec![Box::new(dir)])
        } else if path.is_file() && self.is_python_file(path) {
            let module = Module::new(path.to_path_buf(), self);
            Ok(vec![Box::new(module)])
        } else {
            Ok(vec![])
        }
    }

    pub fn should_ignore_path(&self, path: &Path) -> CollectionResult<bool> {
        // Check __pycache__
        if path.file_name() == Some(std::ffi::OsStr::new("__pycache__")) {
            return Ok(true);
        }

        // Check ignore patterns
        let path_str = path.to_string_lossy();
        for pattern in &self.config.ignore_patterns {
            if path_str.contains(pattern) {
                return Ok(true);
            }
        }

        // Check directory recursion patterns
        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            for pattern in &self.config.norecursedirs {
                if glob_match(pattern, dir_name) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub fn is_python_file(&self, path: &Path) -> bool {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        for pattern in &self.config.python_files {
            if glob_match(pattern, filename) {
                return true;
            }
        }

        false
    }
}

impl Collector for Session {
    fn nodeid(&self) -> &str {
        &self.nodeid
    }

    fn parent(&self) -> Option<&dyn Collector> {
        None
    }

    fn collect(&self) -> CollectionResult<Vec<Box<dyn Collector>>> {
        // Session collection is handled by perform_collect
        Ok(vec![])
    }

    fn path(&self) -> &Path {
        &self.rootpath
    }
}

/// Directory collector
#[derive(Debug)]
pub struct Directory {
    pub path: PathBuf,
    pub nodeid: String,
    parent_session: *const Session,
}

impl Directory {
    fn new(path: PathBuf, session: &Session) -> Self {
        let nodeid = path
            .strip_prefix(&session.rootpath)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        Self {
            path,
            nodeid,
            parent_session: session as *const Session,
        }
    }

    fn session(&self) -> &Session {
        unsafe { &*self.parent_session }
    }
}

impl Collector for Directory {
    fn nodeid(&self) -> &str {
        &self.nodeid
    }

    fn parent(&self) -> Option<&dyn Collector> {
        Some(self.session() as &dyn Collector)
    }

    fn collect(&self) -> CollectionResult<Vec<Box<dyn Collector>>> {
        let mut items = Vec::new();

        let read_dir_result = std::fs::read_dir(&self.path);
        let dir_entries = match read_dir_result {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                return Ok(items);
            }
            Err(err) => return Err(err.into()),
        };

        for entry_result in dir_entries {
            let entry = match entry_result {
                Ok(entry) => entry,
                Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => continue,
                Err(err) => return Err(err.into()),
            };

            let path = entry.path();

            if self.session().should_ignore_path(&path)? {
                continue;
            }

            if path.is_dir() {
                let dir = Directory::new(path, self.session());
                items.push(Box::new(dir) as Box<dyn Collector>);
            } else if path.is_file() && self.session().is_python_file(&path) {
                let module = Module::new(path, self.session());
                items.push(Box::new(module) as Box<dyn Collector>);
            }
        }

        Ok(items)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

/// Python module collector
#[derive(Debug)]
pub struct Module {
    pub path: PathBuf,
    pub nodeid: String,
    parent_session: *const Session,
}

impl Module {
    fn new(path: PathBuf, session: &Session) -> Self {
        let nodeid = path
            .strip_prefix(&session.rootpath)
            .unwrap_or(&path)
            .to_string_lossy()
            .to_string();

        Self {
            path,
            nodeid,
            parent_session: session as *const Session,
        }
    }

    fn session(&self) -> &Session {
        unsafe { &*self.parent_session }
    }
}

impl Collector for Module {
    fn nodeid(&self) -> &str {
        &self.nodeid
    }

    fn parent(&self) -> Option<&dyn Collector> {
        Some(self.session() as &dyn Collector)
    }

    fn collect(&self) -> CollectionResult<Vec<Box<dyn Collector>>> {
        // Read the Python file
        let source = std::fs::read_to_string(&self.path)?;

        // Configure test discovery
        let discovery_config = TestDiscoveryConfig {
            python_classes: self.session().config.python_classes.clone(),
            python_functions: self.session().config.python_functions.clone(),
        };

        let tests = match discover_tests(&self.path, &source, &discovery_config) {
            Ok(tests) => tests,
            Err(e) => {
                return Err(e);
            }
        };

        let mut items: Vec<Box<dyn Collector>> = Vec::new();
        for test in tests {
            let function = test_info_to_function(&test, &self.path, &self.nodeid);
            items.push(Box::new(function));
        }

        Ok(items)
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

/// Test function item
#[derive(Debug)]
pub struct Function {
    #[allow(dead_code)]
    pub name: String,
    pub nodeid: String,
    pub location: Location,
}

impl Collector for Function {
    fn nodeid(&self) -> &str {
        &self.nodeid
    }

    fn parent(&self) -> Option<&dyn Collector> {
        None // TODO: Store parent reference
    }

    fn collect(&self) -> CollectionResult<Vec<Box<dyn Collector>>> {
        // Functions are leaf nodes, they don't collect
        Ok(vec![])
    }

    fn path(&self) -> &Path {
        &self.location.path
    }

    fn is_item(&self) -> bool {
        true
    }
}

/// Collection report
#[derive(Debug)]
#[allow(dead_code)]
pub struct CollectReport {
    pub nodeid: String,
    pub outcome: CollectionOutcome,
    pub longrepr: Option<String>,
    pub error_type: Option<CollectionError>,
    pub result: Vec<Box<dyn Collector>>,
}

impl CollectReport {
    pub fn new(
        nodeid: String,
        outcome: CollectionOutcome,
        longrepr: Option<String>,
        error_type: Option<CollectionError>,
        result: Vec<Box<dyn Collector>>,
    ) -> Self {
        Self {
            nodeid,
            outcome,
            longrepr,
            error_type,
            result,
        }
    }
}

/// Simple glob pattern matching
fn glob_match(pattern: &str, text: &str) -> bool {
    use glob::Pattern;

    // Try to use the glob crate for more accurate matching
    if let Ok(glob_pattern) = Pattern::new(pattern) {
        glob_pattern.matches(text)
    } else {
        // Fallback to simple matching
        if pattern.starts_with('*') && pattern.ends_with('*') {
            let middle = &pattern[1..pattern.len() - 1];
            text.contains(middle)
        } else if let Some(suffix) = pattern.strip_prefix('*') {
            text.ends_with(suffix)
        } else if let Some(prefix) = pattern.strip_suffix('*') {
            text.starts_with(prefix)
        } else {
            pattern == text
        }
    }
}

/// Collect a single node and return a report
pub fn collect_one_node(collector: &dyn Collector) -> CollectReport {
    match collector.collect() {
        Ok(result) => CollectReport::new(
            collector.nodeid().to_string(),
            CollectionOutcome::Passed,
            None,
            None,
            result,
        ),
        Err(e) => CollectReport::new(
            collector.nodeid().to_string(),
            CollectionOutcome::Failed,
            Some(e.to_string()),
            Some(e),
            vec![],
        ),
    }
}
