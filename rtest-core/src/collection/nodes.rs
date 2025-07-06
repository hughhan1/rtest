//! Collection node implementations with parallel support.
//!
//! This module implements pytest's collection logic, including:
//! - Parallel file system traversal
//! - Python file parsing
//! - Test discovery
//! - Collection reporting

use super::config::CollectionConfig;
use super::error::{CollectionError, CollectionOutcome, CollectionResult};
use super::types::{Collector, Location};
use super::utils::glob_match;
use crate::python_discovery::{discover_tests, test_info_to_function, TestDiscoveryConfig};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

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

    pub fn perform_collect(&self, args: &[String]) -> (Vec<Collector>, Vec<(PathBuf, CollectionError)>) {
        let paths = self.resolve_paths(args);
        
        // Collect all results, preserving both successes and errors
        let results: Vec<(PathBuf, CollectionResult<Vec<Collector>>)> = paths
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

    fn collect_path(&self, path: &Path) -> CollectionResult<Vec<Collector>> {
        if self.should_ignore_path(path)? {
            return Ok(vec![]);
        }

        if path.is_dir() {
            let dir = Directory::new(path, &self.rootpath);
            Ok(vec![Collector::Directory(dir)])
        } else if path.is_file() && self.is_python_file(path) {
            let module = Module::new(path, &self.rootpath);
            Ok(vec![Collector::Module(module)])
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

    fn collect(&self, session: &Session) -> CollectionResult<Vec<Collector>> {
        let dir_entries = match std::fs::read_dir(&self.path) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                return Ok(vec![]);
            }
            Err(err) => return Err(err.into()),
        };

        // Use par_bridge for parallel processing without collecting into Vec first
        let results: Vec<_> = dir_entries
            .par_bridge()
            .filter_map(|entry_result| {
                let entry = match entry_result {
                    Ok(entry) => entry,
                    Err(_) => return None,
                };

                let path = entry.path();
                
                if session.should_ignore_path(&path).unwrap_or(true) {
                    return None;
                }

                if path.is_dir() {
                    let dir = Directory::new(&path, &session.rootpath);
                    Some(Collector::Directory(dir))
                } else if path.is_file() && session.is_python_file(&path) {
                    let module = Module::new(&path, &session.rootpath);
                    Some(Collector::Module(module))
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

    fn collect(&self, session: &Session) -> CollectionResult<Vec<Collector>> {
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
                Collector::Function(Function {
                    name: function.name.clone(),
                    nodeid: function.nodeid,
                    location: function.location,
                })
            })
            .collect())
    }
}

/// Collect a single node and return a report
pub fn collect_one_node(node: &Collector, session: &Session) -> CollectReport {
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