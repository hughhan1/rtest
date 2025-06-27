//! Integration between Rust collection and pytest execution.

use crate::collection::{collect_one_node, Collector, Session};
use std::path::PathBuf;

/// Run the Rust-based collection and return test node IDs
pub fn collect_tests_rust(rootpath: PathBuf, args: &[String]) -> Vec<String> {
    let mut session = Session::new(rootpath);

    match session.perform_collect(args) {
        Ok(collectors) => {
            let mut test_nodes = Vec::new();

            for collector in collectors {
                collect_items_recursive(collector.as_ref(), &mut test_nodes);
            }

            test_nodes
        }
        Err(e) => {
            eprintln!("Collection error: {e}");
            vec![]
        }
    }
}

/// Recursively collect all test items
fn collect_items_recursive(collector: &dyn Collector, test_nodes: &mut Vec<String>) {
    if collector.is_item() {
        test_nodes.push(collector.nodeid().to_string());
    } else {
        let report = collect_one_node(collector);
        if matches!(report.outcome, crate::collection::CollectionOutcome::Passed) {
            for child in report.result {
                collect_items_recursive(child.as_ref(), test_nodes);
            }
        }
    }
}

/// Display collection results in a format similar to pytest
pub fn display_collection_results(test_nodes: &[String]) {
    if test_nodes.is_empty() {
        println!("No tests collected.");
    } else {
        println!("Collected {} items:", test_nodes.len());
        for node in test_nodes {
            println!("  {node}");
        }
    }
}
