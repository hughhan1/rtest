//! rtest core library for Python test collection and execution.

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

use std::path::PathBuf;

pub use collection::error::{CollectionError, CollectionResult};
pub use collection_integration::{collect_tests_rust, display_collection_results};
pub use pytest_executor::execute_tests;
pub use runner::{execute_tests_parallel, PytestRunner};
pub use scheduler::{create_scheduler, DistributionMode};
pub use utils::determine_worker_count;
pub use worker::WorkerPool;

/// Resolves test nodes based on the provided arguments.
///
/// This function handles the logic for determining which test nodes to run,
/// including special handling for test-level selection with `::` syntax.
///
/// # Arguments
/// * `files` - The test files/patterns provided by the user
/// * `collect_only` - Whether we're in collect-only mode
/// * `rootpath` - The root path for test collection
///
/// # Returns
/// * A vector of test node strings to be executed
/// * Exits the process if collection fails or if in collect-only mode
pub fn resolve_test_nodes(files: &[String], collect_only: bool, rootpath: PathBuf) -> Vec<String> {
    // Check if any arguments contain :: indicating specific test node IDs
    let has_specific_tests = files.iter().any(|f| f.contains("::"));

    if has_specific_tests {
        // If specific test node IDs are provided, handle them specially
        if collect_only {
            // For collect-only mode with specific tests, we still need to collect from the files
            let (nodes, errors) = match collect_tests_rust(rootpath, files) {
                Ok((nodes, errors)) => (nodes, errors),
                Err(e) => {
                    eprintln!("FATAL: {e}");
                    std::process::exit(1);
                }
            };

            display_collection_results(&nodes, &errors);

            if !errors.errors.is_empty() {
                std::process::exit(1);
            }

            std::process::exit(0);
        } else {
            // For execution mode with specific tests, pass them directly to pytest
            files.to_vec()
        }
    } else {
        // Otherwise, perform normal collection
        let (nodes, errors) = match collect_tests_rust(rootpath, files) {
            Ok((nodes, errors)) => (nodes, errors),
            Err(e) => {
                eprintln!("FATAL: {e}");
                std::process::exit(1);
            }
        };

        display_collection_results(&nodes, &errors);

        // Exit early if there are collection errors to prevent test execution
        if !errors.errors.is_empty() {
            std::process::exit(1);
        }

        if nodes.is_empty() {
            println!("No tests found.");
            std::process::exit(0);
        }

        // Exit after collection if --collect-only flag is set
        if collect_only {
            std::process::exit(0);
        }

        nodes
    }
}
