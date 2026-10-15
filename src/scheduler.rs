use std::collections::BTreeMap;
use std::fmt;
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DistributionMode {
    Load,
    LoadScope,
    LoadFile,
    WorkSteal,
    No,
}

impl fmt::Display for DistributionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DistributionMode::Load => write!(f, "load"),
            DistributionMode::LoadScope => write!(f, "loadscope"),
            DistributionMode::LoadFile => write!(f, "loadfile"),
            DistributionMode::WorkSteal => write!(f, "worksteal"),
            DistributionMode::No => write!(f, "no"),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ParseDistributionModeError {
    UnknownMode(String),
}

impl fmt::Display for ParseDistributionModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseDistributionModeError::UnknownMode(mode) => write!(
                f,
                "Unsupported distribution mode: '{mode}'. Supported modes: load, loadscope, loadfile, worksteal, no"
            ),
        }
    }
}

impl std::error::Error for ParseDistributionModeError {}

impl std::str::FromStr for DistributionMode {
    type Err = ParseDistributionModeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "load" => Ok(DistributionMode::Load),
            "loadscope" => Ok(DistributionMode::LoadScope),
            "loadfile" => Ok(DistributionMode::LoadFile),
            "worksteal" => Ok(DistributionMode::WorkSteal),
            "no" => Ok(DistributionMode::No),
            other => Err(ParseDistributionModeError::UnknownMode(other.to_string())),
        }
    }
}

impl DistributionMode {
    /// Assign tests to nonempty batches for this distribution mode.
    pub fn distribute_tests(
        self,
        tests: Vec<String>,
        num_workers: NonZeroUsize,
    ) -> Vec<Vec<String>> {
        match self {
            // Work stealing currently uses static round-robin assignment.
            Self::Load | Self::WorkSteal => distribute_round_robin(tests, num_workers),
            Self::LoadScope => distribute_by_key(tests, num_workers, extract_scope),
            Self::LoadFile => distribute_by_key(tests, num_workers, extract_file),
            Self::No => {
                if tests.is_empty() {
                    vec![]
                } else {
                    vec![tests]
                }
            }
        }
    }
}

/// Distribute items in input order, returning only nonempty workers.
pub(crate) fn distribute_round_robin<T>(items: Vec<T>, num_workers: NonZeroUsize) -> Vec<Vec<T>> {
    let actual_workers = num_workers.get().min(items.len());
    let mut workers: Vec<Vec<T>> = (0..actual_workers).map(|_| Vec::new()).collect();
    for (worker, item) in (0..actual_workers).cycle().zip(items) {
        workers[worker].push(item);
    }
    workers
}

fn distribute_by_key(
    tests: Vec<String>,
    num_workers: NonZeroUsize,
    key_extractor: impl Fn(&str) -> String,
) -> Vec<Vec<String>> {
    // A single worker preserves input order rather than sorting by group key.
    if num_workers == NonZeroUsize::MIN {
        return distribute_round_robin(tests, num_workers);
    }

    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for test in tests {
        groups.entry(key_extractor(&test)).or_default().push(test);
    }

    distribute_round_robin(groups.into_values().collect(), num_workers)
        .into_iter()
        .map(|groups| groups.into_iter().flatten().collect())
        .collect()
}

fn extract_scope(test_path: &str) -> String {
    // Extract module/class scope from test path
    // Format: path/to/file.py::TestClass::test_method or path/to/file.py::test_function
    let parts: Vec<&str> = test_path.split("::").collect();
    match parts.len() {
        1 | 2 => parts[0].to_string(), // Just file path or File::function
        _ => format!("{}::{}", parts[0], parts[1]), // File::Class::method
    }
}

fn extract_file(test_path: &str) -> String {
    // Extract file path from test path
    // Format: path/to/file.py::TestClass::test_method or path/to/file.py::test_function
    crate::subproject::split_test_node(test_path).0.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_distribution_mode_from_str() {
        assert!(matches!(
            "load".parse::<DistributionMode>(),
            Ok(DistributionMode::Load)
        ));
        assert!(matches!(
            "loadscope".parse::<DistributionMode>(),
            Ok(DistributionMode::LoadScope)
        ));
        assert!(matches!(
            "loadfile".parse::<DistributionMode>(),
            Ok(DistributionMode::LoadFile)
        ));
        assert!(matches!(
            "worksteal".parse::<DistributionMode>(),
            Ok(DistributionMode::WorkSteal)
        ));
        assert!(matches!(
            "no".parse::<DistributionMode>(),
            Ok(DistributionMode::No)
        ));
        assert!("invalid".parse::<DistributionMode>().is_err());
        assert!("loadgroup".parse::<DistributionMode>().is_err()); // No longer supported
    }

    #[test]
    fn test_load_scheduler_empty_tests() {
        let scheduler = DistributionMode::Load;
        let result = scheduler.distribute_tests(vec![], NonZeroUsize::new(4).expect("positive"));
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_scheduler_single_worker() {
        let scheduler = DistributionMode::Load;
        let tests = vec!["test1".into(), "test2".into(), "test3".into()];
        let result =
            scheduler.distribute_tests(tests.clone(), NonZeroUsize::new(1).expect("positive"));
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], tests);
    }

    #[test]
    fn test_load_scheduler_round_robin() {
        let scheduler = DistributionMode::Load;
        let tests = vec![
            "test1".into(),
            "test2".into(),
            "test3".into(),
            "test4".into(),
            "test5".into(),
        ];
        let result = scheduler.distribute_tests(tests, NonZeroUsize::new(3).expect("positive"));

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], vec!["test1", "test4"]);
        assert_eq!(result[1], vec!["test2", "test5"]);
        assert_eq!(result[2], vec!["test3"]);
    }

    #[test]
    fn test_load_scheduler_more_workers_than_tests() {
        let scheduler = DistributionMode::Load;
        let tests = vec!["test1".into(), "test2".into()];
        let result = scheduler.distribute_tests(tests, NonZeroUsize::new(5).expect("positive"));

        assert_eq!(result.len(), 2); // Only non-empty workers
        assert_eq!(result[0], vec!["test1"]);
        assert_eq!(result[1], vec!["test2"]);
    }

    #[test]
    fn test_distribution_mode_display() {
        assert_eq!(format!("{}", DistributionMode::Load), "load");
        assert_eq!(format!("{}", DistributionMode::LoadScope), "loadscope");
        assert_eq!(format!("{}", DistributionMode::LoadFile), "loadfile");
        assert_eq!(format!("{}", DistributionMode::WorkSteal), "worksteal");
        assert_eq!(format!("{}", DistributionMode::No), "no");
    }

    #[test]
    fn test_distribution_mode_from_str_error_message() {
        let error = "invalid".parse::<DistributionMode>().unwrap_err();
        let error_string = error.to_string();
        assert!(error_string.contains("Unsupported distribution mode: 'invalid'"));
        assert!(error_string.contains("Supported modes: load, loadscope, loadfile, worksteal, no"));
    }

    // LoadScope scheduler tests
    #[test]
    fn test_loadscope_scheduler_groups_by_scope() {
        let scheduler = DistributionMode::LoadScope;
        let tests = vec![
            "tests/test_file1.py::TestClass1::test_method1".into(),
            "tests/test_file1.py::TestClass1::test_method2".into(),
            "tests/test_file1.py::test_function1".into(),
            "tests/test_file2.py::TestClass2::test_method1".into(),
            "tests/test_file2.py::test_function2".into(),
        ];
        let result = scheduler.distribute_tests(tests, NonZeroUsize::new(4).expect("positive"));

        // Should have 4 groups: file1::TestClass1, file1, file2::TestClass2, file2
        assert_eq!(result.len(), 4);

        // Verify that tests with same scope are grouped together
        let mut scope_to_worker: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for (worker_idx, worker_tests) in result.iter().enumerate() {
            for test in worker_tests {
                let scope = extract_scope(test);
                if let Some(&existing_worker) = scope_to_worker.get(&scope) {
                    assert_eq!(
                        existing_worker, worker_idx,
                        "Test {test} should be in same worker as other tests from scope {scope}"
                    );
                } else {
                    scope_to_worker.insert(scope, worker_idx);
                }
            }
        }

        // Verify all tests are distributed
        let total_tests: usize = result.iter().map(|w| w.len()).sum();
        assert_eq!(total_tests, 5);

        // Verify the class methods are grouped together
        assert_eq!(scope_to_worker.len(), 4); // Should have 4 different scopes
    }

    #[test]
    fn grouped_modes_preserve_input_order_with_one_worker() {
        let tests = vec![
            "z_file.py::TestClass::test_method".into(),
            "a_file.py::test_function".into(),
        ];
        for mode in [DistributionMode::LoadScope, DistributionMode::LoadFile] {
            assert_eq!(
                mode.distribute_tests(tests.clone(), NonZeroUsize::MIN),
                vec![tests.clone()]
            );
        }
    }

    // LoadFile scheduler tests
    #[test]
    fn test_loadfile_scheduler_groups_by_file() {
        let scheduler = DistributionMode::LoadFile;
        let tests = vec![
            "tests/test_file1.py::TestClass1::test_method1".into(),
            "tests/test_file1.py::TestClass1::test_method2".into(),
            "tests/test_file1.py::test_function1".into(),
            "tests/test_file2.py::TestClass2::test_method1".into(),
            "tests/test_file2.py::test_function2".into(),
            "tests/test_file3.py::test_function3".into(),
        ];
        let result = scheduler.distribute_tests(tests, NonZeroUsize::new(2).expect("positive"));

        assert_eq!(result.len(), 2);

        // Verify that each file's tests are kept together
        let mut file_to_worker: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();

        for (worker_idx, worker_tests) in result.iter().enumerate() {
            for test in worker_tests {
                let file = extract_file(test);
                if let Some(&existing_worker) = file_to_worker.get(&file) {
                    assert_eq!(
                        existing_worker, worker_idx,
                        "Test {test} should be in same worker as other tests from {file}"
                    );
                } else {
                    file_to_worker.insert(file, worker_idx);
                }
            }
        }

        // Verify all tests are distributed
        let total_tests: usize = result.iter().map(|w| w.len()).sum();
        assert_eq!(total_tests, 6);
    }

    // WorkSteal scheduler tests
    #[test]
    fn test_worksteal_scheduler_round_robin() {
        let scheduler = DistributionMode::WorkSteal;
        let tests = vec![
            "test1".into(),
            "test2".into(),
            "test3".into(),
            "test4".into(),
            "test5".into(),
            "test6".into(),
        ];
        let result = scheduler.distribute_tests(tests, NonZeroUsize::new(3).expect("positive"));

        assert_eq!(result.len(), 3);
        // Round-robin: test1->worker0, test2->worker1, test3->worker2, test4->worker0, etc.
        assert_eq!(result[0], vec!["test1", "test4"]);
        assert_eq!(result[1], vec!["test2", "test5"]);
        assert_eq!(result[2], vec!["test3", "test6"]);
    }

    // No scheduler tests
    #[test]
    fn test_no_scheduler_single_group() {
        let scheduler = DistributionMode::No;
        let tests = vec!["test1".into(), "test2".into(), "test3".into()];
        let result =
            scheduler.distribute_tests(tests.clone(), NonZeroUsize::new(5).expect("positive"));

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], tests);
    }

    #[test]
    fn test_no_scheduler_empty_tests() {
        let scheduler = DistributionMode::No;
        let result = scheduler.distribute_tests(vec![], NonZeroUsize::new(3).expect("positive"));
        assert!(result.is_empty());
    }

    // Test deterministic ordering
    #[test]
    fn test_deterministic_distribution() {
        let scheduler = DistributionMode::LoadFile;
        let tests = vec![
            "z_file.py::test1".into(),
            "a_file.py::test2".into(),
            "m_file.py::test3".into(),
            "z_file.py::test4".into(),
            "a_file.py::test5".into(),
        ];

        // Run multiple times to ensure deterministic behavior
        let result1 =
            scheduler.distribute_tests(tests.clone(), NonZeroUsize::new(2).expect("positive"));
        let result2 =
            scheduler.distribute_tests(tests.clone(), NonZeroUsize::new(2).expect("positive"));
        let result3 =
            scheduler.distribute_tests(tests.clone(), NonZeroUsize::new(2).expect("positive"));

        assert_eq!(result1, result2);
        assert_eq!(result2, result3);

        // Verify that the same files end up together across runs
        // This tests the key benefit: deterministic grouping
        for (worker_idx, worker_tests) in result1.iter().enumerate() {
            let mut files_in_worker: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for test in worker_tests {
                files_in_worker.insert(extract_file(test));
            }

            // Verify the same files are in the same worker in all runs
            let mut files_in_worker2: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for test in &result2[worker_idx] {
                files_in_worker2.insert(extract_file(test));
            }

            assert_eq!(
                files_in_worker, files_in_worker2,
                "Worker {worker_idx} should have the same files across runs"
            );
        }
    }
}
