//! Rustic core library for Python test collection and execution.

pub mod cli;
pub mod collection;
pub mod collection_integration;
pub mod collection_service;
pub mod downcast;
pub mod error;
pub mod pytest_executor;
pub mod python_discovery;
pub mod runner;
pub mod scheduler;
pub mod string_interner;
#[cfg(test)]
mod string_interner_tests;
pub mod utils;
pub mod worker;

pub use collection::{CollectionError, CollectionResult};
pub use collection_integration::{collect_functions_rust, collect_tests_rust, display_collection_results};
pub use collection_service::{TestCollectionService, ServiceError, CollectedTest, CollectionStats};
pub use pytest_executor::execute_tests;
pub use runner::PytestRunner;
pub use scheduler::{create_scheduler_string, create_scheduler_function, DistributionMode, LoadGroupScheduler};
pub use utils::determine_worker_count;
pub use worker::WorkerPool;
