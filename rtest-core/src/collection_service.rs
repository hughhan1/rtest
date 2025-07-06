//! Test collection service abstraction
//!
//! This module provides a clean service interface for test collection,
//! separating collection concerns from execution logic.

use crate::collection::{
    CollectionConfig, CollectionError, Collector, Function, Session,
};
use crate::collection_integration::CollectionErrors;
use crate::downcast::CollectorDowncast;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

/// Result type for service operations
pub type ServiceResult<T> = Result<T, ServiceError>;

/// Service-level errors that wrap lower-level errors
#[derive(Debug)]
pub enum ServiceError {
    Collection(CollectionError),
    Configuration(String),
    Io(std::io::Error),
}

impl std::fmt::Display for ServiceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ServiceError::Collection(e) => write!(f, "Collection error: {}", e),
            ServiceError::Configuration(msg) => write!(f, "Configuration error: {}", msg),
            ServiceError::Io(e) => write!(f, "IO error: {}", e),
        }
    }
}

impl std::error::Error for ServiceError {}

impl From<CollectionError> for ServiceError {
    fn from(err: CollectionError) -> Self {
        ServiceError::Collection(err)
    }
}

impl From<std::io::Error> for ServiceError {
    fn from(err: std::io::Error) -> Self {
        ServiceError::Io(err)
    }
}

/// Collected test information
#[derive(Debug, Clone)]
pub struct CollectedTest {
    /// The test nodeid (e.g., "path/to/test.py::TestClass::test_method")
    pub nodeid: String,
    /// The test function object (if available)
    pub function: Option<Function>,
}

/// Collection statistics
#[derive(Debug, Default)]
pub struct CollectionStats {
    pub total_files: usize,
    pub total_tests: usize,
    pub total_errors: usize,
    pub duration_ms: u128,
}

/// Test collection service
pub struct TestCollectionService {
    /// Root path for collection
    rootpath: PathBuf,
    /// Collection configuration
    config: Arc<CollectionConfig>,
}

impl TestCollectionService {
    /// Create a new test collection service
    pub fn new(rootpath: PathBuf) -> Self {
        Self {
            rootpath,
            config: Arc::new(CollectionConfig::default()),
        }
    }

    /// Create with custom configuration
    pub fn with_config(rootpath: PathBuf, config: CollectionConfig) -> Self {
        Self {
            rootpath,
            config: Arc::new(config),
        }
    }

    /// Update configuration
    pub fn set_config(&mut self, config: CollectionConfig) {
        self.config = Arc::new(config);
    }

    /// Collect tests as nodeids (strings)
    pub fn collect_nodeids(&self, args: &[String]) -> ServiceResult<(Vec<String>, CollectionErrors)> {
        let start = std::time::Instant::now();
        
        let session = self.create_session();
        let mut collection_errors = CollectionErrors { errors: Vec::new() };

        match session.perform_collect(args) {
            Ok(collectors) => {
                let mut test_nodes = Vec::new();

                for collector in collectors {
                    self.collect_items_recursive(
                        collector.as_ref(),
                        &mut test_nodes,
                        &mut collection_errors,
                    );
                }

                let _duration = start.elapsed();
                Ok((test_nodes, collection_errors))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Collect tests as Function objects
    pub fn collect_functions(&self, args: &[String]) -> ServiceResult<(Vec<Function>, CollectionErrors)> {
        let start = std::time::Instant::now();
        
        let session = self.create_session();
        let mut collection_errors = CollectionErrors { errors: Vec::new() };

        match session.perform_collect(args) {
            Ok(collectors) => {
                let mut test_functions = Vec::new();

                for collector in collectors {
                    self.collect_functions_recursive(
                        collector.as_ref(),
                        &mut test_functions,
                        &mut collection_errors,
                    );
                }

                let _duration = start.elapsed();
                Ok((test_functions, collection_errors))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Collect tests with full information
    pub fn collect_tests(&self, args: &[String]) -> ServiceResult<(Vec<CollectedTest>, CollectionErrors, CollectionStats)> {
        let start = std::time::Instant::now();
        
        let session = self.create_session();
        let mut collection_errors = CollectionErrors { errors: Vec::new() };
        let mut collected_tests = Vec::new();
        let mut file_count = 0;

        match session.perform_collect(args) {
            Ok(collectors) => {
                for collector in collectors {
                    self.collect_tests_recursive(
                        collector.as_ref(),
                        &mut collected_tests,
                        &mut collection_errors,
                        &mut file_count,
                    );
                }

                let duration = start.elapsed();
                let stats = CollectionStats {
                    total_files: file_count,
                    total_tests: collected_tests.len(),
                    total_errors: collection_errors.errors.len(),
                    duration_ms: duration.as_millis(),
                };

                Ok((collected_tests, collection_errors, stats))
            }
            Err(e) => Err(e.into()),
        }
    }

    /// Validate test paths before collection
    pub fn validate_paths(&self, paths: &[String]) -> ServiceResult<Vec<PathBuf>> {
        let mut validated_paths = Vec::with_capacity(paths.len());

        for path_str in paths {
            let path = PathBuf::from(path_str);
            let absolute_path = if path.is_absolute() {
                path
            } else {
                self.rootpath.join(&path)
            };

            if !absolute_path.exists() {
                return Err(ServiceError::Configuration(
                    format!("Path does not exist: {}", path_str)
                ));
            }

            validated_paths.push(absolute_path);
        }

        Ok(validated_paths)
    }

    /// Create a new session with current configuration
    fn create_session(&self) -> Rc<Session> {
        let mut session = Session::new(self.rootpath.clone());
        session.config = (*self.config).clone();
        Rc::new(session)
    }

    /// Recursively collect test nodeids
    fn collect_items_recursive(
        &self,
        collector: &dyn Collector,
        test_nodes: &mut Vec<String>,
        collection_errors: &mut CollectionErrors,
    ) {
        if collector.is_item() {
            test_nodes.push(collector.nodeid().to_string());
        } else {
            let report = crate::collection::collect_one_node(collector);
            match report.outcome {
                crate::collection::CollectionOutcome::Passed => {
                    for child in report.result {
                        self.collect_items_recursive(child.as_ref(), test_nodes, collection_errors);
                    }
                }
                crate::collection::CollectionOutcome::Failed => {
                    if let Some(error) = report.error_type {
                        collection_errors
                            .errors
                            .push((report.nodeid.clone(), error));
                    }
                }
                _ => {}
            }
        }
    }

    /// Recursively collect Function objects
    fn collect_functions_recursive(
        &self,
        collector: &dyn Collector,
        test_functions: &mut Vec<Function>,
        collection_errors: &mut CollectionErrors,
    ) {
        if collector.is_item() {
            match collector.try_as_function() {
                Some(function) => test_functions.push(function.clone()),
                None => {
                    // Log error about unexpected item type  
                    collection_errors.errors.push((
                        collector.nodeid().to_string(),
                        crate::collection::CollectionError::ParseError(
                            format!("Expected Function item but got different type")
                        ),
                    ));
                }
            }
        } else {
            let report = crate::collection::collect_one_node(collector);
            match report.outcome {
                crate::collection::CollectionOutcome::Passed => {
                    for child in report.result {
                        self.collect_functions_recursive(
                            child.as_ref(),
                            test_functions,
                            collection_errors,
                        );
                    }
                }
                crate::collection::CollectionOutcome::Failed => {
                    if let Some(error) = report.error_type {
                        collection_errors
                            .errors
                            .push((report.nodeid.clone(), error));
                    }
                }
                _ => {}
            }
        }
    }

    /// Recursively collect full test information
    fn collect_tests_recursive(
        &self,
        collector: &dyn Collector,
        collected_tests: &mut Vec<CollectedTest>,
        collection_errors: &mut CollectionErrors,
        file_count: &mut usize,
    ) {
        // Count files
        if collector.as_any().downcast_ref::<crate::collection::Module>().is_some() {
            *file_count += 1;
        }

        if collector.is_item() {
            let nodeid = collector.nodeid().to_string();
            let function = collector.try_as_function().cloned();
            
            collected_tests.push(CollectedTest {
                nodeid,
                function,
            });
        } else {
            let report = crate::collection::collect_one_node(collector);
            match report.outcome {
                crate::collection::CollectionOutcome::Passed => {
                    for child in report.result {
                        self.collect_tests_recursive(
                            child.as_ref(),
                            collected_tests,
                            collection_errors,
                            file_count,
                        );
                    }
                }
                crate::collection::CollectionOutcome::Failed => {
                    if let Some(error) = report.error_type {
                        collection_errors
                            .errors
                            .push((report.nodeid.clone(), error));
                    }
                }
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_creation() {
        let service = TestCollectionService::new(PathBuf::from("/tmp"));
        assert_eq!(service.rootpath, PathBuf::from("/tmp"));
    }

    #[test]
    fn test_validate_paths() {
        let temp_dir = std::env::temp_dir();
        let service = TestCollectionService::new(temp_dir.clone());
        
        // Test with existing path
        let result = service.validate_paths(&[temp_dir.to_string_lossy().to_string()]);
        assert!(result.is_ok());
        
        // Test with non-existing path
        let result = service.validate_paths(&["/non/existent/path".to_string()]);
        assert!(result.is_err());
    }
}