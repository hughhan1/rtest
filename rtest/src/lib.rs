//! Rustic core library for Python test collection and execution.

pub mod cli;
pub mod collection;
pub mod collection_integration;
pub mod config;
pub mod pytest_executor;
pub mod python_discovery;
pub mod runner;
pub mod scheduler;
pub mod subproject;
pub mod utils;
pub mod worker;

pub use collection::error::{CollectionError, CollectionResult};
pub use collection_integration::{collect_tests_rust, display_collection_results};
pub use pytest_executor::execute_tests;
pub use runner::{execute_tests_parallel, PytestRunner};
pub use scheduler::{create_scheduler, DistributionMode};
pub use utils::determine_worker_count;
pub use worker::WorkerPool;
