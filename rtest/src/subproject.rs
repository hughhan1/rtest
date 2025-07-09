//! Subproject detection and test grouping functionality.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// Splits a test node ID into file path and test parts
fn split_test_node(node: &str) -> (&str, Option<&str>) {
    match node.split_once("::") {
        Some((path, test)) => (path, Some(test)),
        None => (node, None),
    }
}

/// Groups test nodes by their subproject
pub fn group_tests_by_subproject(
    rootpath: &Path,
    test_nodes: &[String],
) -> HashMap<PathBuf, Vec<String>> {
    let mut groups: HashMap<PathBuf, Vec<String>> = HashMap::new();
    let mut subproject_roots: HashSet<PathBuf> = HashSet::new();

    for test_node in test_nodes {
        let subproject_root = find_subproject_root(rootpath, test_node);
        let is_subproject = subproject_root != *rootpath;

        if is_subproject {
            subproject_roots.insert(subproject_root.clone());
        }

        groups
            .entry(subproject_root)
            .or_default()
            .push(test_node.clone());
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

    if let Some(parent) = test_path.parent() {
        let mut current = parent;

        while current.starts_with(rootpath) && current != rootpath {
            if current.join("pyproject.toml").exists() {
                return current.to_path_buf();
            }

            match current.parent() {
                Some(parent) => current = parent,
                None => break,
            }
        }
    }

    rootpath.to_path_buf()
}

pub fn make_test_paths_relative(
    test_nodes: &[String],
    old_base: &Path,
    new_base: &Path,
) -> Vec<String> {
    test_nodes
        .iter()
        .map(|node| {
            let (file_path, test_part) = split_test_node(node);

            let absolute_path = old_base.join(file_path);
            match absolute_path.strip_prefix(new_base) {
                Ok(relative_path) => {
                    let mut new_node = relative_path.to_string_lossy().into_owned();
                    if let Some(test) = test_part {
                        new_node.push_str("::");
                        new_node.push_str(test);
                    }
                    new_node
                }
                Err(_) => node.clone(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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
