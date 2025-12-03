//! Integration between Rust collection and pytest execution.

use crate::collection::error::{CollectionError, CollectionOutcome, CollectionWarning};
use crate::collection::nodes::{collect_one_node, Session};
use crate::collection::types::Collector;
use std::path::PathBuf;
use std::rc::Rc;

/// Holds errors and warnings encountered during collection
#[derive(Debug)]
pub struct CollectionErrors {
    pub errors: Vec<(String, CollectionError)>,
    pub warnings: Vec<CollectionWarning>,
    /// Files with parametrized tests that may differ from pytest's collection
    pub uncertain_files: Vec<String>,
}

/// Options for test collection
#[derive(Debug, Default)]
pub struct CollectionOptions {
    /// Mark ALL files with parametrized tests as uncertain (not just complex ones)
    pub all_parametrized_uncertain: bool,
}

/// Run the Rust-based collection and return test node IDs
pub fn collect_tests_rust(
    rootpath: PathBuf,
    args: &[String],
) -> Result<(Vec<String>, CollectionErrors), CollectionError> {
    collect_tests_rust_with_options(rootpath, args, &CollectionOptions::default())
}

/// Run the Rust-based collection with options
pub fn collect_tests_rust_with_options(
    rootpath: PathBuf,
    args: &[String],
    options: &CollectionOptions,
) -> Result<(Vec<String>, CollectionErrors), CollectionError> {
    let session = Rc::new(Session::new(rootpath));
    let mut collection_errors = CollectionErrors {
        errors: Vec::new(),
        warnings: Vec::new(),
        uncertain_files: Vec::new(),
    };

    match session.perform_collect(args) {
        Ok(collectors) => {
            let mut test_nodes = Vec::new();
            let mut files_with_parametrized: std::collections::HashSet<String> = std::collections::HashSet::new();

            for collector in collectors {
                collect_items_recursive(
                    collector.as_ref(),
                    &mut test_nodes,
                    &mut collection_errors,
                    options.all_parametrized_uncertain,
                    &mut files_with_parametrized,
                );
            }

            // Convert set to vec for the output
            collection_errors.uncertain_files = files_with_parametrized.into_iter().collect();
            collection_errors.uncertain_files.sort();

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
    all_parametrized_uncertain: bool,
    files_with_parametrized: &mut std::collections::HashSet<String>,
) {
    if collector.is_item() {
        let nodeid = collector.nodeid();
        test_nodes.push(nodeid.into());

        // Track files with parametrized tests that need uncertain handling
        if collector.is_parametrized() {
            // Extract file path from nodeid (format: "path/to/file.py::test_name" or "path/to/file.py::Class::test_name")
            if let Some(file_path) = nodeid.split("::").next() {
                // Mark files as uncertain when:
                // 1. all_parametrized_uncertain flag is set (mark ALL parametrized files)
                // 2. parametrize couldn't be expanded (is_parametrized=true but nodeid doesn't contain '[')
                // 3. parametrize has uncertain values (e.g., attribute accesses like Enum.VALUE)
                let was_expanded = nodeid.contains('[');
                if all_parametrized_uncertain || !was_expanded || collector.has_uncertain_params() {
                    files_with_parametrized.insert(file_path.to_string());
                }
            }
        }
    } else {
        let report = collect_one_node(collector);
        match report.outcome {
            CollectionOutcome::Passed => {
                for child in report.result {
                    collect_items_recursive(
                        child.as_ref(),
                        test_nodes,
                        collection_errors,
                        all_parametrized_uncertain,
                        files_with_parametrized,
                    );
                }
            }
            CollectionOutcome::Failed => {
                // Add failed files to uncertain list so hybrid approach falls back to pytest
                // Extract file path from nodeid (nodeid for a file is just the path)
                let file_path = report.nodeid.split("::").next().unwrap_or(&report.nodeid);
                if file_path.ends_with(".py") {
                    files_with_parametrized.insert(file_path.to_string());
                }

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
            }
        }
        println!(
            "!!!!!!!!!!!!!!!!!!!!! Warning: {} errors during collection !!!!!!!!!!!!!!!!!!!!!",
            errors.errors.len()
        );
    }

    let item_count = test_nodes.len();
    let error_count = errors.errors.len();
    let warning_count = errors.warnings.len();

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

        if !test_nodes.is_empty() {
            println!();
            for node in test_nodes {
                println!("  {node}");
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
