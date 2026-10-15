//! Collection node implementations.

use super::config::CollectionConfig;
use super::error::{CollectionError, CollectionResult, CollectionWarning};
use super::types::{CollectionBranch, CollectionNode};
use super::utils::glob_match_any;
use crate::python_discovery::{
    discover_tests_with_inheritance, format_cannot_expand_warning, CasesExpansion,
    TestDiscoveryConfig,
};
use std::path::{Path, PathBuf};

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

    fn resolve_args(&self, args: &[String]) -> Vec<PathBuf> {
        args.iter().map(|arg| self.rootpath.join(arg)).collect()
    }

    pub fn perform_collect(&self, args: &[String]) -> CollectionResult<Vec<CollectionNode>> {
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
            let resolved = self.resolve_args(args);
            for path in &resolved {
                if !path.exists() {
                    return Err(CollectionError::FileNotFound(path.clone()));
                }
            }
            resolved
        };

        Ok(paths
            .iter()
            .filter_map(|path| self.collect_path(path))
            .collect())
    }

    fn collect_path(&self, path: &Path) -> Option<CollectionNode> {
        if self.should_ignore_path(path) {
            return None;
        }
        let branch = if path.is_dir() {
            CollectionBranch::Directory(path.to_path_buf())
        } else if path.is_file() && self.is_python_file(path) {
            CollectionBranch::Module(path.to_path_buf())
        } else {
            return None;
        };
        Some(CollectionNode::Branch(branch))
    }

    pub fn should_ignore_path(&self, path: &Path) -> bool {
        // Check __pycache__
        if path.file_name() == Some(std::ffi::OsStr::new("__pycache__")) {
            return true;
        }

        // Check ignore patterns
        let path_str = path.to_string_lossy();
        for pattern in &self.config.ignore_patterns {
            if path_str.contains(pattern) {
                return true;
            }
        }

        // Check directory recursion patterns
        if path.is_dir() {
            let dir_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

            if glob_match_any(&self.config.norecursedirs, dir_name) {
                return true;
            }
        }

        false
    }

    pub fn is_python_file(&self, path: &Path) -> bool {
        let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        glob_match_any(&self.config.python_files, filename)
    }
}

impl CollectionBranch {
    fn path(&self) -> &Path {
        match self {
            Self::Directory(path) | Self::Module(path) => path,
        }
    }

    pub(crate) fn nodeid(&self, session: &Session) -> String {
        let path = self.path();
        path.strip_prefix(&session.rootpath)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned()
    }

    pub(crate) fn collect(
        &self,
        session: &Session,
    ) -> CollectionResult<(Vec<CollectionNode>, Vec<CollectionWarning>)> {
        match self {
            Self::Directory(path) => Self::collect_directory(path, session),
            Self::Module(path) => self.collect_module(path, session),
        }
    }

    fn collect_directory(
        path: &Path,
        session: &Session,
    ) -> CollectionResult<(Vec<CollectionNode>, Vec<CollectionWarning>)> {
        let read_dir_result = std::fs::read_dir(path);
        let dir_entries = match read_dir_result {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
                return Ok((vec![], vec![]));
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

            items.extend(session.collect_path(&entry.path()));
        }

        Ok((items, vec![]))
    }

    fn collect_module(
        &self,
        path: &Path,
        session: &Session,
    ) -> CollectionResult<(Vec<CollectionNode>, Vec<CollectionWarning>)> {
        let nodeid = self.nodeid(session);
        // Read the Python file
        let source = std::fs::read_to_string(path)?;

        // Configure test discovery
        let discovery_config = TestDiscoveryConfig {
            python_classes: session.config.python_classes.clone(),
            python_functions: session.config.python_functions.clone(),
        };

        // Use the session's root path for module resolution
        let root_path = &session.rootpath;

        let (tests, discovery_warnings) =
            discover_tests_with_inheritance(path, &source, &discovery_config, root_path)?;

        let mut all_warnings: Vec<CollectionWarning> = discovery_warnings;

        let nodes: Vec<CollectionNode> = tests
            .into_iter()
            .flat_map(|test| {
                if let CasesExpansion::CannotExpand(reason) = &test.cases_expansion {
                    let nodeid = test.base_nodeid(&nodeid);
                    let warning_str = format_cannot_expand_warning(&nodeid, reason);
                    all_warnings.push(CollectionWarning {
                        file_path: path.to_string_lossy().into_owned(),
                        line: 0,
                        message: warning_str,
                    });
                }
                test.nodeids(&nodeid)
            })
            .map(CollectionNode::Test)
            .collect();

        Ok((nodes, all_warnings))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perform_collect_explicit_nonexistent_path_returns_error() {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        let root = temp_dir.path().canonicalize().expect("canonicalize");

        let session = Session::new(root);

        // Explicit args with non-existent path should return Err (FileNotFound)
        let result = session.perform_collect(&["nonexistent.py".to_string()]);
        assert!(matches!(result, Err(CollectionError::FileNotFound(_))));
    }
}
