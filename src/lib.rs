//! rtest core library for Python test collection and execution.

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
pub mod subproject;
pub mod utils;
pub mod worker;

pub use collection::error::{CollectionError, CollectionResult};
pub use collection_integration::{collect_tests_rust, display_collection_results};
pub use native_runner::{
    collect_test_files, default_python_files, execute_native, NativeRunnerConfig,
};
#[cfg(feature = "extension-module")]
pub use pyo3::_rtest;
pub use pytest_executor::execute_tests;
pub use runner::{execute_tests_parallel, PytestRunner};
pub use scheduler::{create_scheduler, DistributionMode};
pub use utils::determine_worker_count;
pub use worker::WorkerPool;
