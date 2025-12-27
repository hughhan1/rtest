use std::collections::BTreeMap;
use std::fmt;

#[derive(Debug, Clone)]
pub enum SchedulerError {
    InvalidTestPath(String),
    InvalidWorkerCount(usize),
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchedulerError::InvalidTestPath(path) => {
                write!(f, "Invalid test path format: {path}")
            }
            SchedulerError::InvalidWorkerCount(count) => {
                write!(f, "Invalid worker count: {count}")
            }
        }
    }
}

impl std::error::Error for SchedulerError {}

#[derive(Debug, Clone)]
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

pub trait Scheduler {
    fn distribute_tests(&self, tests: Vec<String>, num_workers: usize) -> Vec<Vec<String>>;
}

// Common utility functions to eliminate code duplication
fn validate_and_handle_edge_cases(
    tests: &[String],
    num_workers: usize,
) -> Option<Vec<Vec<String>>> {
    if num_workers == 0 || tests.is_empty() {
        return Some(vec![]);
    }

    if num_workers == 1 {
        return Some(vec![tests.to_vec()]);
    }

    None // Continue with normal processing
}

fn distribute_groups_to_workers<T>(groups: Vec<Vec<T>>, num_workers: usize) -> Vec<Vec<T>> {
    let mut workers: Vec<Vec<T>> = (0..num_workers).map(|_| Vec::new()).collect();

    for (i, group) in groups.into_iter().enumerate() {
        workers[i % num_workers].extend(group);
    }

    workers.into_iter().filter(|w| !w.is_empty()).collect()
}

fn group_tests_by_key<F>(tests: Vec<String>, key_extractor: F) -> Vec<Vec<String>>
where
    F: Fn(&str) -> String,
{
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for test in tests {
        let key = key_extractor(&test);
        groups.entry(key).or_default().push(test);
    }

    groups.into_values().collect()
}

pub struct LoadScheduler;

impl Scheduler for LoadScheduler {
    fn distribute_tests(&self, tests: Vec<String>, num_workers: usize) -> Vec<Vec<String>> {
        if let Some(result) = validate_and_handle_edge_cases(&tests, num_workers) {
            return result;
        }

        let mut workers: Vec<Vec<String>> = vec![Vec::new(); num_workers];

        for (i, test) in tests.into_iter().enumerate() {
            workers[i % num_workers].push(test);
        }

        workers.into_iter().filter(|w| !w.is_empty()).collect()
    }
}

pub struct LoadScopeScheduler;

impl Scheduler for LoadScopeScheduler {
    fn distribute_tests(&self, tests: Vec<String>, num_workers: usize) -> Vec<Vec<String>> {
        if let Some(result) = validate_and_handle_edge_cases(&tests, num_workers) {
            return result;
        }

        let groups = group_tests_by_key(tests, extract_scope);
        distribute_groups_to_workers(groups, num_workers)
    }
}

pub struct LoadFileScheduler;

impl Scheduler for LoadFileScheduler {
    fn distribute_tests(&self, tests: Vec<String>, num_workers: usize) -> Vec<Vec<String>> {
        if let Some(result) = validate_and_handle_edge_cases(&tests, num_workers) {
            return result;
        }

        let groups = group_tests_by_key(tests, extract_file);
        distribute_groups_to_workers(groups, num_workers)
    }
}

/// WorkStealScheduler implements a round-robin distribution that's optimized for
/// work stealing scenarios. While true work stealing requires runtime coordination
/// between workers, this scheduler provides better load balancing by:
/// 1. Using round-robin assignment (avoiding clustering of slow tests)
/// 2. Interleaving tests across workers to maximize stealing opportunities
/// 3. Ensuring each worker gets a good mix of tests from different parts of the suite
pub struct WorkStealScheduler;

impl Scheduler for WorkStealScheduler {
    fn distribute_tests(&self, tests: Vec<String>, num_workers: usize) -> Vec<Vec<String>> {
        if let Some(result) = validate_and_handle_edge_cases(&tests, num_workers) {
            return result;
        }

        let mut workers: Vec<Vec<String>> = (0..num_workers).map(|_| Vec::new()).collect();

        // Round-robin distribution - this gives better work-stealing characteristics
        // because it interleaves tests across workers, making it more likely that
        // when one worker finishes early, there are still tests available for stealing
        for (i, test) in tests.into_iter().enumerate() {
            workers[i % num_workers].push(test);
        }

        workers.into_iter().filter(|w| !w.is_empty()).collect()
    }
}

pub struct NoScheduler;

impl Scheduler for NoScheduler {
    fn distribute_tests(&self, tests: Vec<String>, _num_workers: usize) -> Vec<Vec<String>> {
        if tests.is_empty() {
            vec![]
        } else {
            vec![tests]
        }
    }
}

fn extract_scope(test_path: &str) -> String {
    // Extract module/class scope from test path
    // Format: path/to/file.py::TestClass::test_method or path/to/file.py::test_function
    let parts: Vec<&str> = test_path.split("::").collect();
    match parts.len() {
        0 => test_path.to_string(), // Shouldn't happen, but handle gracefully
        1 => parts[0].to_string(),  // Just file path
        2 => parts[0].to_string(),  // File::function
        _ => format!("{}::{}", parts[0], parts[1]), // File::Class::method
    }
}

fn extract_file(test_path: &str) -> String {
    // Extract file path from test path
    // Format: path/to/file.py::TestClass::test_method or path/to/file.py::test_function
    test_path
        .split("::")
        .next()
        .unwrap_or(test_path)
        .to_string()
}

pub fn create_scheduler(mode: DistributionMode) -> Box<dyn Scheduler> {
    match mode {
        DistributionMode::Load => Box::new(LoadScheduler),
        DistributionMode::LoadScope => Box::new(LoadScopeScheduler),
        DistributionMode::LoadFile => Box::new(LoadFileScheduler),
        DistributionMode::WorkSteal => Box::new(WorkStealScheduler),
        DistributionMode::No => Box::new(NoScheduler),
    }
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
        let scheduler = LoadScheduler;
        let result = scheduler.distribute_tests(vec![], 4);
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_scheduler_zero_workers() {
        let scheduler = LoadScheduler;
        let tests = vec!["test1".into(), "test2".into()];
        let result = scheduler.distribute_tests(tests, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_scheduler_single_worker() {
        let scheduler = LoadScheduler;
        let tests = vec!["test1".into(), "test2".into(), "test3".into()];
        let result = scheduler.distribute_tests(tests.clone(), 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], tests);
    }

    #[test]
    fn test_load_scheduler_round_robin() {
        let scheduler = LoadScheduler;
        let tests = vec![
            "test1".into(),
            "test2".into(),
            "test3".into(),
            "test4".into(),
            "test5".into(),
        ];
        let result = scheduler.distribute_tests(tests, 3);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], vec!["test1", "test4"]);
        assert_eq!(result[1], vec!["test2", "test5"]);
        assert_eq!(result[2], vec!["test3"]);
    }

    #[test]
    fn test_load_scheduler_more_workers_than_tests() {
        let scheduler = LoadScheduler;
        let tests = vec!["test1".into(), "test2".into()];
        let result = scheduler.distribute_tests(tests, 5);

        assert_eq!(result.len(), 2); // Only non-empty workers
        assert_eq!(result[0], vec!["test1"]);
        assert_eq!(result[1], vec!["test2"]);
    }

    #[test]
    fn test_create_scheduler() {
        let scheduler = create_scheduler(DistributionMode::Load);
        let tests = vec!["test1".into(), "test2".into()];
        let result = scheduler.distribute_tests(tests, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_load_scheduler_consistent_distribution() {
        let scheduler = LoadScheduler;
        let tests = vec![
            "test1".into(),
            "test2".into(),
            "test3".into(),
            "test4".into(),
        ];

        // Test the same distribution multiple times - should be consistent
        let result1 = scheduler.distribute_tests(tests.clone(), 2);
        let result2 = scheduler.distribute_tests(tests.clone(), 2);

        assert_eq!(result1, result2);
        assert_eq!(result1[0], vec!["test1", "test3"]);
        assert_eq!(result1[1], vec!["test2", "test4"]);
    }

    #[test]
    fn test_load_scheduler_all_tests_distributed() {
        let scheduler = LoadScheduler;
        let tests = vec![
            "test1".into(),
            "test2".into(),
            "test3".into(),
            "test4".into(),
            "test5".into(),
        ];

        let result = scheduler.distribute_tests(tests.clone(), 3);

        let mut all_distributed_tests: Vec<String> = Vec::new();
        for worker_tests in result {
            all_distributed_tests.extend(worker_tests);
        }

        all_distributed_tests.sort();
        let mut expected_tests = tests.clone();
        expected_tests.sort();

        assert_eq!(all_distributed_tests, expected_tests);
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
        let scheduler = LoadScopeScheduler;
        let tests = vec![
            "tests/test_file1.py::TestClass1::test_method1".into(),
            "tests/test_file1.py::TestClass1::test_method2".into(),
            "tests/test_file1.py::test_function1".into(),
            "tests/test_file2.py::TestClass2::test_method1".into(),
            "tests/test_file2.py::test_function2".into(),
        ];
        let result = scheduler.distribute_tests(tests, 4);

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
    fn test_loadscope_scheduler_single_worker() {
        let scheduler = LoadScopeScheduler;
        let tests = vec![
            "tests/test_file1.py::TestClass1::test_method1".into(),
            "tests/test_file2.py::test_function1".into(),
        ];
        let result = scheduler.distribute_tests(tests.clone(), 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 2);
    }

    // LoadFile scheduler tests
    #[test]
    fn test_loadfile_scheduler_groups_by_file() {
        let scheduler = LoadFileScheduler;
        let tests = vec![
            "tests/test_file1.py::TestClass1::test_method1".into(),
            "tests/test_file1.py::TestClass1::test_method2".into(),
            "tests/test_file1.py::test_function1".into(),
            "tests/test_file2.py::TestClass2::test_method1".into(),
            "tests/test_file2.py::test_function2".into(),
            "tests/test_file3.py::test_function3".into(),
        ];
        let result = scheduler.distribute_tests(tests, 2);

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
        let scheduler = WorkStealScheduler;
        let tests = vec![
            "test1".into(),
            "test2".into(),
            "test3".into(),
            "test4".into(),
            "test5".into(),
            "test6".into(),
        ];
        let result = scheduler.distribute_tests(tests, 3);

        assert_eq!(result.len(), 3);
        // Round-robin: test1->worker0, test2->worker1, test3->worker2, test4->worker0, etc.
        assert_eq!(result[0], vec!["test1", "test4"]);
        assert_eq!(result[1], vec!["test2", "test5"]);
        assert_eq!(result[2], vec!["test3", "test6"]);
    }

    #[test]
    fn test_worksteal_scheduler_uneven_distribution() {
        let scheduler = WorkStealScheduler;
        let tests = vec![
            "test1".into(),
            "test2".into(),
            "test3".into(),
            "test4".into(),
            "test5".into(),
        ];
        let result = scheduler.distribute_tests(tests, 3);

        assert_eq!(result.len(), 3);
        // Round-robin distribution: some workers get one more test
        assert_eq!(result[0], vec!["test1", "test4"]);
        assert_eq!(result[1], vec!["test2", "test5"]);
        assert_eq!(result[2], vec!["test3"]);

        // Verify all tests are distributed
        let total_tests: usize = result.iter().map(|w| w.len()).sum();
        assert_eq!(total_tests, 5);
    }

    #[test]
    fn test_worksteal_scheduler_interleaving() {
        let scheduler = WorkStealScheduler;
        let tests = vec![
            "fast_test1".into(),
            "slow_test1".into(),
            "fast_test2".into(),
            "slow_test2".into(),
        ];
        let result = scheduler.distribute_tests(tests, 2);

        assert_eq!(result.len(), 2);
        // Tests should be interleaved across workers for better work stealing
        assert_eq!(result[0], vec!["fast_test1", "fast_test2"]);
        assert_eq!(result[1], vec!["slow_test1", "slow_test2"]);
    }

    // No scheduler tests
    #[test]
    fn test_no_scheduler_single_group() {
        let scheduler = NoScheduler;
        let tests = vec!["test1".into(), "test2".into(), "test3".into()];
        let result = scheduler.distribute_tests(tests.clone(), 5);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], tests);
    }

    #[test]
    fn test_no_scheduler_empty_tests() {
        let scheduler = NoScheduler;
        let result = scheduler.distribute_tests(vec![], 3);
        assert!(result.is_empty());
    }

    // Utility function tests
    #[test]
    fn test_extract_scope() {
        assert_eq!(
            extract_scope("tests/test_file.py::TestClass::test_method"),
            "tests/test_file.py::TestClass"
        );
        assert_eq!(
            extract_scope("tests/test_file.py::test_function"),
            "tests/test_file.py"
        );
        assert_eq!(extract_scope("tests/test_file.py"), "tests/test_file.py");
    }

    #[test]
    fn test_extract_file() {
        assert_eq!(
            extract_file("tests/test_file.py::TestClass::test_method"),
            "tests/test_file.py"
        );
        assert_eq!(
            extract_file("tests/test_file.py::test_function"),
            "tests/test_file.py"
        );
        assert_eq!(extract_file("tests/test_file.py"), "tests/test_file.py");
    }

    // Create scheduler tests for all modes
    #[test]
    fn test_create_scheduler_all_modes() {
        let load_scheduler = create_scheduler(DistributionMode::Load);
        let loadscope_scheduler = create_scheduler(DistributionMode::LoadScope);
        let loadfile_scheduler = create_scheduler(DistributionMode::LoadFile);
        let worksteal_scheduler = create_scheduler(DistributionMode::WorkSteal);
        let no_scheduler = create_scheduler(DistributionMode::No);

        let tests = vec!["test1".into(), "test2".into()];

        assert_eq!(load_scheduler.distribute_tests(tests.clone(), 2).len(), 2);
        assert_eq!(
            loadscope_scheduler.distribute_tests(tests.clone(), 2).len(),
            2
        );
        assert_eq!(
            loadfile_scheduler.distribute_tests(tests.clone(), 2).len(),
            2
        );
        assert_eq!(
            worksteal_scheduler.distribute_tests(tests.clone(), 2).len(),
            2
        );
        assert_eq!(no_scheduler.distribute_tests(tests.clone(), 2).len(), 1);
    }

    // Test deterministic ordering
    #[test]
    fn test_deterministic_distribution() {
        let scheduler = LoadFileScheduler;
        let tests = vec![
            "z_file.py::test1".into(),
            "a_file.py::test2".into(),
            "m_file.py::test3".into(),
            "z_file.py::test4".into(),
            "a_file.py::test5".into(),
        ];

        // Run multiple times to ensure deterministic behavior
        let result1 = scheduler.distribute_tests(tests.clone(), 2);
        let result2 = scheduler.distribute_tests(tests.clone(), 2);
        let result3 = scheduler.distribute_tests(tests.clone(), 2);

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
