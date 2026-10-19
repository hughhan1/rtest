//! Subproject detection and test grouping functionality.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Split a node ID into its file path and suffix, preserving the `::` separator.
pub(crate) fn split_test_node(node: &str) -> (&str, &str) {
    node.split_at(node.find("::").unwrap_or(node.len()))
}

/// Group tests into nonempty subprojects with paths relative to each working directory.
pub fn group_tests_by_subproject(
    rootpath: &Path,
    test_nodes: Vec<String>,
) -> HashMap<PathBuf, Vec<String>> {
    let mut groups: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let mut subproject_roots: HashSet<PathBuf> = HashSet::new();

    for test_node in test_nodes {
        let subproject_root = find_subproject_root(rootpath, &test_node);
        let is_subproject = subproject_root != *rootpath;

        if is_subproject {
            subproject_roots.insert(subproject_root.clone());
        }

        groups.entry(subproject_root).or_default().push(test_node);
    }

    if let Some(root_tests) = groups.get_mut(rootpath) {
        root_tests.retain(|test_node| {
            let (file_path, _) = split_test_node(test_node);
            let test_path = rootpath.join(file_path);

            !subproject_roots
                .iter()
                .any(|subproject| test_path.starts_with(subproject))
        });
    }

    groups.retain(|directory, tests| {
        if directory != rootpath {
            *tests = make_test_paths_relative(tests, rootpath, directory);
        }
        !tests.is_empty()
    });
    groups
}

fn find_subproject_root(rootpath: &Path, test_node: &str) -> PathBuf {
    let (file_path, _) = split_test_node(test_node);
    let test_path = rootpath.join(file_path);

    // Ensure the test path is within the root path
    let test_path = match test_path.canonicalize() {
        Ok(path) if path.starts_with(rootpath) => path,
        _ => return rootpath.to_path_buf(),
    };

    test_path
        .ancestors()
        .skip(1)
        .take_while(|current| current.starts_with(rootpath) && *current != rootpath)
        .find(|current| current.join("pyproject.toml").exists())
        .unwrap_or(rootpath)
        .to_path_buf()
}

pub fn make_test_paths_relative(
    test_nodes: &[String],
    old_base: &Path,
    new_base: &Path,
) -> Vec<String> {
    test_nodes
        .iter()
        .map(|node| {
            let (file_path, suffix) = split_test_node(node);

            let absolute_path = old_base.join(file_path);
            match absolute_path.strip_prefix(new_base) {
                Ok(relative_path) => format!("{}{suffix}", relative_path.display()),
                Err(_) => node.clone(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_split_test_node_with_test_part() {
        let (path, test) = split_test_node("tests/test_a.py::test_x");
        assert_eq!(path, "tests/test_a.py");
        assert_eq!(test, Some("test_x"));
    }

    #[test]
    fn test_split_test_node_with_class_and_test() {
        // split_once stops at the first "::", leaving the rest as the test part.
        let (path, test) = split_test_node("tests/test_a.py::TestA::test_x");
        assert_eq!(path, "tests/test_a.py");
        assert_eq!(test, Some("TestA::test_x"));
    }

    #[test]
    fn test_split_test_node_without_test_part() {
        let (path, test) = split_test_node("tests/test_a.py");
        assert_eq!(path, "tests/test_a.py");
        assert_eq!(test, None);
    }

    #[test]
    fn test_find_subproject_root_no_pyproject_returns_root() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path().canonicalize().expect("canonicalize");
        let tests_dir = root.join("tests");
        fs::create_dir_all(&tests_dir).expect("create tests dir");
        fs::write(tests_dir.join("test_a.py"), "def test_a():\n    pass\n").expect("write");

        let result = find_subproject_root(&root, "tests/test_a.py::test_a");
        assert_eq!(result, root);
    }

    #[test]
    fn test_find_subproject_root_detects_nested_pyproject() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path().canonicalize().expect("canonicalize");
        let subproject = root.join("packages").join("pkg_a");
        fs::create_dir_all(&subproject).expect("create subproject dir");
        fs::write(
            subproject.join("pyproject.toml"),
            "[project]\nname='pkg_a'\n",
        )
        .expect("write toml");
        fs::write(subproject.join("test_a.py"), "def test_a():\n    pass\n").expect("write test");

        let result = find_subproject_root(&root, "packages/pkg_a/test_a.py::test_a");
        assert_eq!(result, subproject);
    }

    #[test]
    fn test_find_subproject_root_nonexistent_path_returns_root() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path().canonicalize().expect("canonicalize");
        // Path cannot be canonicalized, so it falls back to the root path.
        let result = find_subproject_root(&root, "missing/test_x.py::test_x");
        assert_eq!(result, root);
    }

    #[test]
    fn test_group_tests_by_subproject_splits_root_and_subproject() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path().canonicalize().expect("canonicalize");

        // Root-level test.
        let root_tests = root.join("tests");
        fs::create_dir_all(&root_tests).expect("create root tests");
        fs::write(root_tests.join("test_root.py"), "def test_r():\n    pass\n").expect("write");

        // Subproject with its own pyproject.toml.
        let subproject = root.join("sub");
        fs::create_dir_all(&subproject).expect("create sub");
        fs::write(subproject.join("pyproject.toml"), "[project]\nname='sub'\n")
            .expect("write toml");
        fs::write(subproject.join("test_sub.py"), "def test_s():\n    pass\n").expect("write");

        let nodes = vec![
            "tests/test_root.py::test_r".to_string(),
            "sub/test_sub.py::test_s".to_string(),
        ];

        let groups = group_tests_by_subproject(&root, &nodes);

        assert_eq!(groups.len(), 2, "expected root and subproject groups");
        assert_eq!(
            groups.get(&root).map(Vec::len),
            Some(1),
            "root group should hold only the root test"
        );
        assert_eq!(
            groups.get(&subproject).map(Vec::len),
            Some(1),
            "subproject group should hold the subproject test"
        );
        assert_eq!(groups[&root][0], "tests/test_root.py::test_r");
        assert_eq!(groups[&subproject][0], "sub/test_sub.py::test_s");
    }

    #[test]
    fn test_group_tests_by_subproject_all_root() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let root = temp_dir.path().canonicalize().expect("canonicalize");
        fs::write(root.join("test_a.py"), "def test_a():\n    pass\n").expect("write a");
        fs::write(root.join("test_b.py"), "def test_b():\n    pass\n").expect("write b");

        let nodes = vec![
            "test_a.py::test_a".to_string(),
            "test_b.py::test_b".to_string(),
        ];

        let groups = group_tests_by_subproject(&root, &nodes);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[&root].len(), 2);
    }

    #[test]
    fn test_make_test_paths_relative_unrelated_base_is_unchanged() {
        let test_nodes = vec!["tests/test_a.py::test_x".to_string()];
        let old_base = Path::new("/Users/test/project");
        // new_base is not a prefix of the absolute path, so nodes pass through.
        let new_base = Path::new("/somewhere/else");
        let result = make_test_paths_relative(&test_nodes, old_base, new_base);
        assert_eq!(result[0], "tests/test_a.py::test_x");
    }

    #[test]
    fn test_make_test_paths_relative_without_test_part() {
        let test_nodes = vec!["examples/tutorial/tests/test_auth.py".to_string()];
        let old_base = Path::new("/Users/test/flask");
        let new_base = Path::new("/Users/test/flask/examples/tutorial");
        let result = make_test_paths_relative(&test_nodes, old_base, new_base);
        assert_eq!(result[0], "tests/test_auth.py");
    }

    #[test]
    fn groups_nonempty_projects_with_paths_relative_to_their_working_directory() {
        let project = tempfile::tempdir().expect("temporary project");
        let root = project.path().canonicalize().expect("canonical root");
        let nested = root.join("nested");
        std::fs::create_dir(&nested).expect("nested project");
        for path in [
            root.join("test_root.py"),
            nested.join("test_child.py"),
            nested.join("pyproject.toml"),
        ] {
            std::fs::write(path, "").expect("project file");
        }
        let groups = group_tests_by_subproject(
            &root,
            vec![
                "test_root.py::test_root".into(),
                "nested/test_child.py::TestChild::test_method[x::y]".into(),
                // Existing behavior excludes unresolved nodes under a discovered subproject.
                "nested/test_missing.py::test_missing".into(),
            ],
        );
        assert_eq!(
            groups,
            HashMap::from([
                (root.clone(), vec!["test_root.py::test_root".into()]),
                (
                    nested,
                    vec!["test_child.py::TestChild::test_method[x::y]".into()]
                ),
            ])
        );
        assert!(group_tests_by_subproject(&root, vec![]).is_empty());
    }

    #[test]
    fn test_make_test_paths_relative() {
        let test_nodes = vec![
            "examples/tutorial/tests/test_auth.py::test_register".to_string(),
            "examples/tutorial/tests/test_auth.py::TestAuth::test_login".to_string(),
            "tests/test_basic.py::test_simple".to_string(),
        ];

        let old_base = Path::new("/Users/test/flask");
        let new_base = Path::new("/Users/test/flask/examples/tutorial");

        let result = make_test_paths_relative(&test_nodes, old_base, new_base);

        assert_eq!(result[0], "tests/test_auth.py::test_register");
        assert_eq!(result[1], "tests/test_auth.py::TestAuth::test_login");
        assert_eq!(result[2], "tests/test_basic.py::test_simple");
    }
}
