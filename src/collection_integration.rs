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
}

/// Run the Rust-based collection and return test node IDs
pub fn collect_tests_rust(
    rootpath: PathBuf,
    args: &[String],
) -> Result<(Vec<String>, CollectionErrors), CollectionError> {
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
        test_nodes.push(collector.nodeid().into());
    } else {
        let report = collect_one_node(collector);
        collection_errors.warnings.extend(report.warnings);
        match report.outcome {
            CollectionOutcome::Passed => {
                for child in report.result {
                    collect_items_recursive(child.as_ref(), test_nodes, collection_errors);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn tempdir_root() -> (tempfile::TempDir, PathBuf) {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir");
        // Use a normal-named subdirectory as the collection root. The tempdir
        // itself is named ".tmpXXXX", which matches the ".*" norecursedirs
        // pattern and would be ignored during directory traversal.
        let root = temp_dir
            .path()
            .canonicalize()
            .expect("failed to canonicalize temp dir")
            .join("project");
        fs::create_dir(&root).expect("create project root");
        (temp_dir, root)
    }

    #[test]
    fn test_collect_tests_rust_collects_functions() {
        let (_temp, root) = tempdir_root();
        fs::write(
            root.join("test_sample.py"),
            "def test_one():\n    pass\n\ndef test_two():\n    pass\n",
        )
        .expect("write test file");

        let (nodes, errors) =
            collect_tests_rust(root.clone(), &["test_sample.py".to_string()]).expect("collect ok");

        assert_eq!(nodes.len(), 2, "should collect two test functions");
        assert!(nodes.iter().any(|n| n.ends_with("::test_one")));
        assert!(nodes.iter().any(|n| n.ends_with("::test_two")));
        assert!(errors.errors.is_empty());
    }

    #[test]
    fn test_collect_tests_rust_directory_recursion() {
        let (_temp, root) = tempdir_root();
        fs::write(root.join("test_a.py"), "def test_a():\n    pass\n").expect("write a");
        let subdir = root.join("sub");
        fs::create_dir(&subdir).expect("create subdir");
        fs::write(subdir.join("test_b.py"), "def test_b():\n    pass\n").expect("write b");

        // No args -> collect the whole root directory recursively.
        let (nodes, errors) = collect_tests_rust(root.clone(), &[]).expect("collect ok");

        assert_eq!(nodes.len(), 2, "should collect across nested directories");
        assert!(nodes.iter().any(|n| n.ends_with("::test_a")));
        assert!(nodes.iter().any(|n| n.ends_with("::test_b")));
        assert!(errors.errors.is_empty());
    }

    #[test]
    fn test_collect_tests_rust_nonexistent_path_errors() {
        let (_temp, root) = tempdir_root();
        let result = collect_tests_rust(root, &["does_not_exist.py".to_string()]);
        assert!(matches!(result, Err(CollectionError::FileNotFound(_))));
    }

    #[test]
    fn test_collect_tests_rust_reports_parse_errors() {
        let (_temp, root) = tempdir_root();
        // Invalid Python should surface as a collection error rather than panicking.
        fs::write(root.join("test_broken.py"), "def test_x(:\n    pass\n").expect("write broken");

        let (nodes, errors) =
            collect_tests_rust(root.clone(), &["test_broken.py".to_string()]).expect("collect ok");

        assert!(nodes.is_empty(), "broken file yields no test nodes");
        assert_eq!(errors.errors.len(), 1, "broken file yields one error");
    }

    #[test]
    fn test_collect_tests_rust_empty_directory() {
        let (_temp, root) = tempdir_root();
        let (nodes, errors) = collect_tests_rust(root, &[]).expect("collect ok");
        assert!(nodes.is_empty());
        assert!(errors.errors.is_empty());
    }

    #[test]
    fn test_display_collection_results_smoke() {
        // display_collection_results only prints; these calls exercise the
        // formatting branches without panicking.
        let empty = CollectionErrors {
            errors: Vec::new(),
            warnings: Vec::new(),
        };
        display_collection_results(&[], &empty);

        let nodes = vec![
            "tests/test_a.py::test_one".to_string(),
            "tests/test_a.py::test_two".to_string(),
        ];
        let with_warnings = CollectionErrors {
            errors: vec![(
                "tests/test_bad.py".to_string(),
                CollectionError::ParseError("unexpected token".into()),
            )],
            warnings: vec![CollectionWarning {
                file_path: "tests/test_a.py".into(),
                line: 3,
                message: "cannot expand cases".into(),
            }],
        };
        display_collection_results(&nodes, &with_warnings);

        // Exercise the singular-count branches and every error variant.
        let single = vec!["tests/test_a.py::only".to_string()];
        let all_errors = CollectionErrors {
            errors: vec![
                ("a".to_string(), CollectionError::ImportError("m".into())),
                (
                    "b".to_string(),
                    CollectionError::IoError(std::io::Error::other("io")),
                ),
                ("c".to_string(), CollectionError::SkipError("s".into())),
                (
                    "d".to_string(),
                    CollectionError::FileNotFound(PathBuf::from("x.py")),
                ),
            ],
            warnings: Vec::new(),
        };
        display_collection_results(&single, &all_errors);
    }

    #[test]
    fn test_collection_errors_debug() {
        let errors = CollectionErrors {
            errors: Vec::new(),
            warnings: Vec::new(),
        };
        assert!(format!("{errors:?}").contains("CollectionErrors"));
    }
}
