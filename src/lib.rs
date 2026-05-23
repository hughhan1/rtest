//! rtest core library for Python test collection and execution.

pub mod cache;
pub mod cli;
pub mod collection;
pub mod collection_integration;
pub mod config;
pub mod native_runner;
#[cfg(feature = "extension-module")]
mod pyo3;
pub mod pytest_executor;
pub mod python_discovery;
pub mod runner;
pub mod scheduler;
pub mod selection;
pub mod session;
pub mod subproject;
pub mod utils;
pub mod worker;

pub use collection::error::{CollectionError, CollectionResult};
pub use collection_integration::{
    collect_tests_rust, display_collection_results, CollectedItem, CollectionDisplayStats,
    CollectionErrors,
};
pub use native_runner::{
    collect_test_files, default_python_classes, default_python_files, default_python_functions,
    execute_native, files_from_nodeids, NativeExecutionResult, NativeRunnerConfig,
};
#[cfg(feature = "extension-module")]
pub use pyo3::_rtest;
pub use pytest_executor::{execute_tests, execute_tests_exit_code, PytestExecutionResult};
pub use runner::{
    execute_tests_parallel, execute_tests_parallel_with_outcomes, ParallelExecutionConfig,
    PytestRunner,
};
pub use scheduler::{create_scheduler, DistributionMode};
pub use utils::determine_worker_count;
pub use worker::{WorkerPool, WorkerTask};
