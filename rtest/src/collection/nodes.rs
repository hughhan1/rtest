//! Collection node implementations.

use super::config::CollectionConfig;
use super::error::{CollectionError, CollectionOutcome, CollectionResult};
use super::types::{Collector, Location};
use super::utils::glob_match;
use crate::python_discovery::{
    discover_tests_with_inheritance, test_info_to_function, TestDiscoveryConfig,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Root of the collection tree
#[derive(Debug)]
pub struct Session {
    pub rootpath: PathBuf,
    pub nodeid: String,
    pub config: CollectionConfig,
    #[allow(dead_code)]
    cache: HashMap<PathBuf, Vec<Box<dyn Collector>>>,
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
        self: Rc<Self>,
        args: &[String],
    ) -> CollectionResult<Vec<Box<dyn Collector>>> {
        let paths = if args.is_empty() {
            let pytest_config = crate::config::read_pytest_config(&self.rootpath);

            if !pytest_config.testpaths.is_empty() {
                pytest_config
                    .testpaths
                    .iter()
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
        };

        Ok(paths
            .into_iter()
            .filter_map(|path| self.collect_path(&path).ok())
            .flatten()
            .collect())
    }

    fn collect_path(self: &Rc<Self>, path: &Path) -> CollectionResult<Vec<Box<dyn Collector>>> {
        if self.should_ignore_path(path)? {
            return Ok(vec![]);
        }

        if path.is_dir() {
            let dir = Directory::new(path.to_path_buf(), Rc::clone(self));
            Ok(vec![Box::new(dir)])
        } else if path.is_file() && self.is_python_file(path) {
            let module = Module::new(path.to_path_buf(), Rc::clone(self));
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
    parent_session: Rc<Session>,
}

impl Directory {
    fn new(path: PathBuf, session: Rc<Session>) -> Self {
        let nodeid = path
            .strip_prefix(&session.rootpath)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        Self {
            path,
            nodeid,
            parent_session: session,
        }
    }

    fn session(&self) -> &Session {
        &self.parent_session
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
        let read_dir_result = std::fs::read_dir(&self.path);
        let dir_entries = match read_dir_result {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                return Ok(vec![]);
            }
            Err(err) => return Err(err.into()),
        };

        // Process entries, filtering out unnecessary Vec allocations
        let mut items = Vec::new();

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
                let dir = Directory::new(path, Rc::clone(&self.parent_session));
                items.push(Box::new(dir) as Box<dyn Collector>);
            } else if path.is_file() && self.session().is_python_file(&path) {
                let module = Module::new(path, Rc::clone(&self.parent_session));
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
    parent_session: Rc<Session>,
}

impl Module {
    fn new(path: PathBuf, session: Rc<Session>) -> Self {
        let nodeid = path
            .strip_prefix(&session.rootpath)
            .unwrap_or(&path)
            .to_string_lossy()
            .into_owned();

        Self {
            path,
            nodeid,
            parent_session: session,
        }
    }

    fn session(&self) -> &Session {
        &self.parent_session
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

        // Use the session's root path for module resolution
        let root_path = &self.session().rootpath;

        let (tests, warnings) =
            discover_tests_with_inheritance(&self.path, &source, &discovery_config, root_path)?;

        // Print warnings directly (following pytest's approach)
        for warning in &warnings {
            println!("{}", warning);
        }

        // Use iterator to transform tests without intermediate allocations
        Ok(tests
            .into_iter()
            .map(|test| {
                let function = test_info_to_function(&test, &self.path, &self.nodeid);
                Box::new(function) as Box<dyn Collector>
            })
            .collect())
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

/// Collect a single node and return a report
pub fn collect_one_node(collector: &dyn Collector) -> CollectReport {
    match collector.collect() {
        Ok(result) => CollectReport::new(
            collector.nodeid().into(),
            CollectionOutcome::Passed,
            None,
            None,
            result,
        ),
        Err(e) => CollectReport::new(
            collector.nodeid().into(),
            CollectionOutcome::Failed,
            Some(e.to_string()),
            Some(e),
            vec![],
        ),
    }
}
