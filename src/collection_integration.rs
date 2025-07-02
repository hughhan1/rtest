//! Integration between Rust collection and pytest execution.

use crate::collection::{collect_one_node, CollectionError, Collector, Session};
use std::path::PathBuf;

/// Holds errors encountered during collection
#[derive(Debug)]
pub struct CollectionErrors {
    pub errors: Vec<(String, CollectionError)>,
}

/// Run the Rust-based collection and return test node IDs
pub fn collect_tests_rust(
    rootpath: PathBuf,
    args: &[String],
) -> Result<(Vec<String>, CollectionErrors), CollectionError> {
    let mut session = Session::new(rootpath);
    let mut collection_errors = CollectionErrors { errors: Vec::new() };

    match session.perform_collect(args) {
        Ok(collectors) => {
            let mut test_nodes = Vec::new();

            for collector in collectors {
                collect_items_recursive(
                    collector.as_ref(),
                    &mut test_nodes,
                    &mut collection_errors,
                );
            }

            Ok((test_nodes, collection_errors))
        }
        Err(e) => Err(e),
    }
}

/// Recursively collect all test items
fn collect_items_recursive(
    collector: &dyn Collector,
    test_nodes: &mut Vec<String>,
    collection_errors: &mut CollectionErrors,
) {
    if collector.is_item() {
        test_nodes.push(collector.nodeid().to_string());
    } else {
        let report = collect_one_node(collector);
        match report.outcome {
            crate::collection::CollectionOutcome::Passed => {
                for child in report.result {
                    collect_items_recursive(child.as_ref(), test_nodes, collection_errors);
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

/// Display collection results in a format similar to pytest
pub fn display_collection_results(test_nodes: &[String], errors: &CollectionErrors) {
    // ANSI color codes
    const RED: &str = "\x1b[31m";
    const BOLD_RED: &str = "\x1b[1;31m";
    const RESET: &str = "\x1b[0m";

    if !errors.errors.is_empty() {
        println!(
            "===================================== ERRORS ======================================"
        );
        for (nodeid, error) in &errors.errors {
            println!("{BOLD_RED}_ ERROR collecting {nodeid} _{RESET}");
            match error {
                CollectionError::ParseError(msg) => {
                    println!("{RED}E   {msg}{RESET}");
                }
                CollectionError::ImportError(msg) => {
                    println!("{RED}E   ImportError: {msg}{RESET}");
                }
                CollectionError::IoError(e) => {
                    println!("{RED}E   IO Error: {e}{RESET}");
                }
                CollectionError::SkipError(msg) => {
                    println!("{RED}E   Skipped: {msg}{RESET}");
                }
            }
        }
        println!(
            "!!!!!!!!!!!!!!!!!!!!! Warning: {} errors during collection !!!!!!!!!!!!!!!!!!!!!",
            errors.errors.len()
        );
    }

    let item_count = test_nodes.len();
    let error_count = errors.errors.len();

    if item_count == 0 && error_count == 0 {
        println!("No tests collected.");
    } else {
        let mut summary_parts = Vec::new();

        if item_count > 0 {
            summary_parts.push(format!(
                "collected {} item{}",
                item_count,
                if item_count == 1 { "" } else { "s" }
            ));
        }

        if error_count > 0 {
            summary_parts.push(format!(
                "{} error{}",
                error_count,
                if error_count == 1 { "" } else { "s" }
            ));
        }

        if !summary_parts.is_empty() {
            println!("{}", summary_parts.join(" / "));
        }

        if !test_nodes.is_empty() {
            println!();
            for node in test_nodes {
                println!("  {node}");
            }
        }
    }
}
