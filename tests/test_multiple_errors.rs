//! Test that multiple syntax errors are collected and reported

use rtest::collection_integration::{collect_tests_rust, display_collection_results};
use rtest::CollectionError;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_collection_multiple_syntax_errors() {
    let temp_dir = TempDir::new().expect("Failed to create temp directory");
    let project_path = temp_dir.path().join("test_project");
    fs::create_dir_all(&project_path).expect("Failed to create project directory");

    let error_files = vec![
        ("test_assert_empty.py", "assert"),
        ("test_if_missing.py", "if : ...\n    pass"),
        ("test_while_missing.py", "while : ...\n    pass"),
    ];

    for (filename, content) in &error_files {
        let file_path = project_path.join(filename);
        fs::write(&file_path, content).expect("Failed to write file");
    }

    let valid_content = r#"
def test_valid():
    assert True
    
class TestValidClass:
    def test_method(self):
        pass
"#;
    fs::write(project_path.join("test_valid.py"), valid_content).expect("Failed to write file");

    println!("Files in project directory:");
    for entry in std::fs::read_dir(&project_path).unwrap() {
        let entry = entry.unwrap();
        println!("  - {:?}", entry.path());
    }

    let result = collect_tests_rust(project_path, &[]);
    assert!(
        result.is_ok(),
        "Should not fail immediately on syntax errors"
    );

    let (test_nodes, errors) = result.unwrap();

    println!("Test nodes found: {test_nodes:?}");
    println!("Errors found: {errors:?}");

    assert_eq!(test_nodes.len(), 2, "Should find tests from valid file");
    assert!(test_nodes
        .iter()
        .any(|n| n.contains("test_valid.py::test_valid")));
    assert!(test_nodes
        .iter()
        .any(|n| n.contains("test_valid.py::TestValidClass::test_method")));

    assert_eq!(errors.errors.len(), 3, "Should collect all 3 syntax errors");

    for (filename, _) in &error_files {
        assert!(
            errors
                .errors
                .iter()
                .any(|(nodeid, _)| nodeid.contains(filename)),
            "Should have error for {filename}"
        );
    }

    for (_, error) in &errors.errors {
        assert!(
            matches!(error, CollectionError::ParseError(_)),
            "All errors should be parse errors"
        );
    }

    println!("\n--- Test output ---");
    display_collection_results(&test_nodes, &errors);
    println!("--- End test output ---\n");
}
