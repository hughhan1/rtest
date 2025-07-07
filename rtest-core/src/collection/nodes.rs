//! Collection node implementations with parallel support.
//!
//! This module implements pytest's collection logic, including:
//! - Parallel file system traversal
//! - Python file parsing
//! - Test discovery
//! - Collection reporting

use super::config::CollectionConfig;
use super::error::{CollectionError, CollectionOutcome, CollectionResult};
use super::types::Location;
use super::utils::glob_match;
use crate::python_discovery::{discover_tests, test_info_to_function, TestDiscoveryConfig};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

/// Directory collector
#[derive(Debug, Clone)]
pub struct Directory {
    pub path: PathBuf,
    pub nodeid: String,
}

/// Module collector
#[derive(Debug, Clone)]
pub struct Module {
    pub path: PathBuf,
    pub nodeid: String,
}

/// Function collector
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub nodeid: String,
    pub location: Location,
}

/// Concrete collector types as an enum
#[derive(Debug, Clone)]
pub enum CollectorNode {
    Directory(Directory),
    Module(Module),
    Function(Function),
}

impl CollectorNode {
    pub fn nodeid(&self) -> &str {
        match self {
            CollectorNode::Directory(d) => &d.nodeid,
            CollectorNode::Module(m) => &m.nodeid,
            CollectorNode::Function(f) => &f.nodeid,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            CollectorNode::Directory(d) => &d.path,
            CollectorNode::Module(m) => &m.path,
            CollectorNode::Function(f) => &f.location.path,
        }
    }

    pub fn is_item(&self) -> bool {
        matches!(self, CollectorNode::Function(_))
    }

    pub fn collect(&self, session: &Session) -> CollectionResult<Vec<CollectorNode>> {
        match self {
            CollectorNode::Directory(d) => d.collect(session),
            CollectorNode::Module(m) => m.collect(session),
            CollectorNode::Function(_) => Ok(vec![]), // Functions are leaf nodes
        }
    }
}

/// Root of the collection tree
#[derive(Debug)]
pub struct Session {
    pub rootpath: PathBuf,
    pub config: CollectionConfig,
}

impl Session {
    pub fn new(rootpath: PathBuf) -> Self {
        Self {
            rootpath,
            config: CollectionConfig::default(),
        }
    }

    pub fn perform_collect(&self, args: &[String]) -> (Vec<CollectorNode>, Vec<(PathBuf, CollectionError)>) {
        let paths = self.resolve_paths(args);
        
        // Collect all results, preserving both successes and errors
        let results: Vec<(PathBuf, CollectionResult<Vec<CollectorNode>>)> = paths
            .par_iter()
            .map(|path| (path.clone(), self.collect_path(path)))
            .collect();
        
        let mut collectors = Vec::new();
        let mut errors = Vec::new();
        
        for (path, result) in results {
            match result {
                Ok(mut path_collectors) => collectors.append(&mut path_collectors),
                Err(e) => errors.push((path, e)),
            }
        }
        
        (collectors, errors)
    }

    fn resolve_paths(&self, args: &[String]) -> Vec<PathBuf> {
        if args.is_empty() {
            let pytest_config = crate::config::read_pytest_config(&self.rootpath);
            
            if !pytest_config.testpaths.is_empty() {
                pytest_config.testpaths.iter()
                    .map(|p| self.rootpath.join(p))
                    .collect()
            } else if self.config.testpaths.is_empty() {
                vec![self.rootpath.clone()]
            } else {
                self.config.testpaths.clone()
            }
        } else {
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
        }
    }

    fn collect_path(&self, path: &Path) -> CollectionResult<Vec<CollectorNode>> {
        if self.should_ignore_path(path)? {
            return Ok(vec![]);
        }

        if path.is_dir() {
            let dir = Directory::new(path, &self.rootpath);
            let mut collectors = vec![CollectorNode::Directory(dir.clone())];
            // Recursively collect directory contents
            collectors.extend(dir.collect(self)?);
            Ok(collectors)
        } else if path.is_file() && self.is_python_file(path) {
            let module = Module::new(path, &self.rootpath);
            Ok(vec![CollectorNode::Module(module)])
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

impl Directory {
    fn new(path: &Path, rootpath: &Path) -> Self {
        let nodeid = path
            .strip_prefix(rootpath)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();

        Self {
            path: path.to_path_buf(),
            nodeid,
        }
    }

    fn collect(&self, session: &Session) -> CollectionResult<Vec<CollectorNode>> {
        let dir_entries = match std::fs::read_dir(&self.path) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                return Ok(vec![]);
            }
            Err(err) => return Err(err.into()),
        };

        // Use par_bridge for parallel processing
        let results: Vec<_> = dir_entries
            .par_bridge()
            .filter_map(|entry_result| {
                let entry = match entry_result {
                    Ok(entry) => entry,
                    Err(_) => return None,
                };

                let entry_path = entry.path();
                
                if session.should_ignore_path(&entry_path).unwrap_or(true) {
                    return None;
                }

                if entry_path.is_dir() {
                    let dir = Directory::new(&entry_path, &session.rootpath);
                    Some(CollectorNode::Directory(dir))
                } else if entry_path.is_file() && session.is_python_file(&entry_path) {
                    let module = Module::new(&entry_path, &session.rootpath);
                    Some(CollectorNode::Module(module))
                } else {
                    None
                }
            })
            .collect();
        
        Ok(results)
    }
}

impl Module {
    fn new(path: &Path, rootpath: &Path) -> Self {
        let nodeid = path
            .strip_prefix(rootpath)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();

        Self {
            path: path.to_path_buf(),
            nodeid,
        }
    }

    fn collect(&self, session: &Session) -> CollectionResult<Vec<CollectorNode>> {
        // Read the Python file
        let source = std::fs::read_to_string(&self.path)?;

        // Configure test discovery
        let discovery_config = TestDiscoveryConfig {
            python_classes: session.config.python_classes.clone(),
            python_functions: session.config.python_functions.clone(),
        };

        let tests = discover_tests(&self.path, &source, &discovery_config)?;

        // Convert test info to function nodes
        Ok(tests
            .into_iter()
            .map(|test| {
                let function = test_info_to_function(&test, &self.path, &self.nodeid);
                CollectorNode::Function(Function {
                    name: function.name.clone(),
                    nodeid: function.nodeid,
                    location: function.location,
                })
            })
            .collect())
    }
}

/// Collection report
#[derive(Debug)]
pub struct CollectReport {
    pub nodeid: String,
    pub outcome: CollectionOutcome,
    pub longrepr: Option<String>,
    pub error_type: Option<CollectionError>,
    pub result: Vec<CollectorNode>,
}

impl CollectReport {
    pub fn new(
        nodeid: String,
        outcome: CollectionOutcome,
        longrepr: Option<String>,
        error_type: Option<CollectionError>,
        result: Vec<CollectorNode>,
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

/// Collect a single node and return a report
pub fn collect_one_node(node: &CollectorNode, session: &Session) -> CollectReport {
    match node.collect(session) {
        Ok(result) => CollectReport::new(
            node.nodeid().into(),
            CollectionOutcome::Passed,
            None,
            None,
            result,
        ),
        Err(e) => CollectReport::new(
            node.nodeid().into(),
            CollectionOutcome::Failed,
            Some(e.to_string()),
            Some(e),
            vec![],
        ),
    }
}