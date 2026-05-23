//! Integration between Rust collection and pytest execution.

use crate::collection::error::{CollectionError, CollectionOutcome, CollectionWarning};
use crate::collection::nodes::{collect_one_node, Session};
use crate::collection::types::Collector;
use std::path::PathBuf;
use std::rc::Rc;

/// A collected test item with metadata for post-collection selection.
#[derive(Debug, Clone)]
pub struct CollectedItem {
    pub nodeid: String,
    pub keywords: Vec<String>,
}

impl CollectedItem {
    pub fn nodeids(items: &[Self]) -> Vec<String> {
        items.iter().map(|item| item.nodeid.clone()).collect()
    }
}

/// Summary counts for collection display when selection filters items.
#[derive(Debug, Clone, Copy, Default)]
pub struct CollectionDisplayStats {
    pub deselected: usize,
}

/// Holds errors and warnings encountered during collection
#[derive(Debug)]
pub struct CollectionErrors {
    pub errors: Vec<(String, CollectionError)>,
    pub warnings: Vec<CollectionWarning>,
}

/// Run the Rust-based collection and return test items
pub fn collect_tests_rust(
    rootpath: PathBuf,
    args: &[String],
) -> Result<(Vec<CollectedItem>, CollectionErrors), CollectionError> {
    let session = Rc::new(Session::new(rootpath));
    let mut collection_errors = CollectionErrors {
        errors: Vec::new(),
        warnings: Vec::new(),
    };

    match session.perform_collect(args) {
        Ok((collectors, path_errors)) => {
            for (path, error) in path_errors {
                collection_errors.errors.push((path, error));
            }

            let mut test_items = Vec::new();

            for collector in collectors {
                collect_items_recursive(
                    collector.as_ref(),
                    &mut test_items,
                    &mut collection_errors,
                );
            }

            Ok((test_items, collection_errors))
        }
        Err(e) => Err(e),
    }
}

/// Recursively collect all test items
fn collect_items_recursive(
    collector: &dyn Collector,
    test_items: &mut Vec<CollectedItem>,
    collection_errors: &mut CollectionErrors,
) {
    if collector.is_item() {
        test_items.push(CollectedItem {
            nodeid: collector.nodeid().to_string(),
            keywords: collector.keywords().to_vec(),
        });
    } else {
        let report = collect_one_node(collector);
        collection_errors.warnings.extend(report.warnings);
        match report.outcome {
            CollectionOutcome::Passed => {
                for child in report.result {
                    collect_items_recursive(child.as_ref(), test_items, collection_errors);
                }
            }
            CollectionOutcome::Failed => {
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
pub fn display_collection_results(
    test_items: &[CollectedItem],
    errors: &CollectionErrors,
    stats: CollectionDisplayStats,
) {
    // ANSI color codes
    const RED: &str = "\x1b[31m";
    const BOLD_RED: &str = "\x1b[1;31m";
    const YELLOW: &str = "\x1b[33m";
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
                CollectionError::FileNotFound(path) => {
                    println!(
                        "{RED}E   file or directory not found: {}{RESET}",
                        path.display()
                    );
                }
            }
        }
        println!(
            "!!!!!!!!!!!!!!!!!!!!! Warning: {} errors during collection !!!!!!!!!!!!!!!!!!!!!",
            errors.errors.len()
        );
    }

    let item_count = test_items.len();
    let error_count = errors.errors.len();
    let warning_count = errors.warnings.len();

    if item_count == 0 && error_count == 0 && stats.deselected == 0 {
        println!("No tests collected.");
    } else {
        let mut summary_parts = Vec::new();

        if item_count > 0 || stats.deselected > 0 {
            summary_parts.push(format!(
                "collected {} item{}",
                item_count + stats.deselected,
                if item_count + stats.deselected == 1 {
                    ""
                } else {
                    "s"
                }
            ));
        }

        if stats.deselected > 0 {
            summary_parts.push(format!("{} deselected", stats.deselected));
        }

        if error_count > 0 {
            summary_parts.push(format!(
                "{} error{}",
                error_count,
                if error_count == 1 { "" } else { "s" }
            ));
        }

        if warning_count > 0 {
            summary_parts.push(format!(
                "{} warning{}",
                warning_count,
                if warning_count == 1 { "" } else { "s" }
            ));
        }

        if !summary_parts.is_empty() {
            println!("{}", summary_parts.join(" / "));
        }

        if !test_items.is_empty() {
            println!();
            for item in test_items {
                println!("  {}", item.nodeid);
            }
        }
    }

    // Display warnings after the test list
    if !errors.warnings.is_empty() {
        println!();
        println!(
            "=============================== warnings summary ==============================="
        );
        for warning in &errors.warnings {
            println!("{YELLOW}{warning}{RESET}");
        }
        println!("-- Docs: https://docs.pytest.org/en/stable/how-to/capture-warnings.html");
    }
}
