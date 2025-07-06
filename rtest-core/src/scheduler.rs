use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use crate::collection::Function;
use crate::string_interner::intern;

// Re-export the new error type
pub use crate::error::SchedulerError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DistributionMode {
    Load,
    LoadScope,
    LoadFile,
    LoadGroup,
    WorkSteal,
    No,
}

impl fmt::Display for DistributionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl DistributionMode {
    pub const ALL: &'static [Self] = &[
        Self::Load,
        Self::LoadScope,
        Self::LoadFile,
        Self::LoadGroup,
        Self::WorkSteal,
        Self::No,
    ];
    
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Load => "load",
            Self::LoadScope => "loadscope",
            Self::LoadFile => "loadfile",
            Self::LoadGroup => "loadgroup",
            Self::WorkSteal => "worksteal",
            Self::No => "no",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseDistributionModeError {
    UnknownMode(String),
}

impl fmt::Display for ParseDistributionModeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseDistributionModeError::UnknownMode(mode) => {
                write!(f, "Unsupported distribution mode: '{}'. Supported modes: ", mode)?;
                for (i, mode) in DistributionMode::ALL.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", mode)?;
                }
                Ok(())
            }
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
            "loadgroup" => Ok(DistributionMode::LoadGroup),
            "worksteal" => Ok(DistributionMode::WorkSteal),
            "no" => Ok(DistributionMode::No),
            other => Err(ParseDistributionModeError::UnknownMode(other.to_string())),
        }
    }
}

/// Generic scheduler trait that can work with any distributeable item
pub trait Scheduler<T> {
    /// Distribute items across workers
    /// 
    /// # Arguments
    /// * `items` - Items to distribute (consumed for zero-copy when possible)
    /// * `num_workers` - Number of workers to distribute across
    /// 
    /// # Returns
    /// Vector of work batches, one per worker. Empty workers are filtered out.
    fn distribute(&self, items: Vec<T>, num_workers: usize) -> Result<Vec<Vec<T>>, SchedulerError>;
    
    /// Get the minimum number of workers this scheduler can effectively use
    fn min_workers(&self) -> usize {
        1
    }
    
    /// Get the recommended number of workers for a given item count
    fn recommend_workers(&self, item_count: usize) -> usize {
        std::cmp::min(item_count, num_cpus::get())
    }
}

// Macro to validate worker count
macro_rules! validate_worker_count {
    ($num_workers:expr) => {
        if $num_workers == 0 {
            return Err(SchedulerError::InvalidWorkerCount {
                requested: $num_workers,
                min: 1,
                max: usize::MAX,
            });
        }
    };
}

// Common utility functions to eliminate code duplication
#[inline]
#[allow(dead_code)]
fn validate_inputs<T>(items: &[T], num_workers: usize) -> Result<(), SchedulerError> {
    if num_workers == 0 {
        return Err(SchedulerError::InvalidWorkerCount {
            requested: num_workers,
            min: 1,
            max: usize::MAX,
        });
    }
    if items.is_empty() {
        return Err(SchedulerError::EmptyInput);
    }
    Ok(())
}

#[inline]
#[allow(dead_code)]
fn create_workers<T>(num_workers: usize) -> Vec<Vec<T>> {
    let mut workers = Vec::with_capacity(num_workers);
    for _ in 0..num_workers {
        workers.push(Vec::new());
    }
    workers
}

fn distribute_groups_to_workers<T>(groups: Vec<Vec<T>>, num_workers: usize) -> Vec<Vec<T>> {
    if groups.is_empty() || num_workers == 0 {
        return vec![];
    }
    
    let mut workers: Vec<Vec<T>> = Vec::with_capacity(num_workers);
    for _ in 0..num_workers {
        workers.push(Vec::new());
    }
    
    // Use round-robin with pre-calculated capacities for better memory allocation
    let total_items: usize = groups.iter().map(|g| g.len()).sum();
    let items_per_worker = (total_items + num_workers - 1) / num_workers;
    
    for worker in &mut workers {
        worker.reserve(items_per_worker);
    }
    
    for (i, group) in groups.into_iter().enumerate() {
        workers[i % num_workers].extend(group);
    }
    
    // Remove empty workers
    workers.retain(|w| !w.is_empty());
    workers
}

fn group_items_by_key<T, K, F>(items: Vec<T>, key_extractor: F) -> Vec<Vec<T>>
where 
    K: Ord,
    F: Fn(&T) -> K,
{
    if items.is_empty() {
        return vec![];
    }
    
    let mut groups: BTreeMap<K, Vec<T>> = BTreeMap::new();
    
    // Pre-allocate based on expected group sizes
    let _estimated_groups = std::cmp::min(items.len(), 100); // Reasonable upper bound
    
    for item in items {
        let key = key_extractor(&item);
        groups.entry(key).or_insert_with(|| Vec::with_capacity(1)).push(item);
    }
    
    groups.into_values().collect()
}

pub struct LoadScheduler;

impl<T> Scheduler<T> for LoadScheduler {
    fn distribute(&self, items: Vec<T>, num_workers: usize) -> Result<Vec<Vec<T>>, SchedulerError> {
        validate_worker_count!(num_workers);
        
        if items.is_empty() {
            return Ok(vec![]);
        }

        if num_workers == 1 {
            return Ok(vec![items]);
        }

        let items_per_worker = (items.len() + num_workers - 1) / num_workers;
        let mut workers: Vec<Vec<T>> = Vec::with_capacity(num_workers);
        
        for _ in 0..num_workers {
            workers.push(Vec::with_capacity(items_per_worker));
        }

        for (i, item) in items.into_iter().enumerate() {
            workers[i % num_workers].push(item);
        }

        workers.retain(|w| !w.is_empty());
        Ok(workers)
    }
}

pub struct LoadScopeScheduler;

impl Scheduler<String> for LoadScopeScheduler {
    fn distribute(&self, items: Vec<String>, num_workers: usize) -> Result<Vec<Vec<String>>, SchedulerError> {
        validate_worker_count!(num_workers);
        
        if items.is_empty() {
            return Ok(vec![]);
        }

        if num_workers == 1 {
            return Ok(vec![items]);
        }

        let groups = group_items_by_key(items, |s| extract_scope(s));
        Ok(distribute_groups_to_workers(groups, num_workers))
    }
}

pub struct LoadFileScheduler;

pub struct LoadGroupScheduler;

impl Scheduler<Function> for LoadGroupScheduler {
    fn distribute(&self, items: Vec<Function>, num_workers: usize) -> Result<Vec<Vec<Function>>, SchedulerError> {
        validate_worker_count!(num_workers);
        
        if items.is_empty() {
            return Ok(vec![]);
        }

        if num_workers == 1 {
            return Ok(vec![items]);
        }

        // Group functions by their xdist_group
        // Use string interning to avoid repeated allocations
        let groups = group_items_by_key(items, |func| {
            match &func.xdist_group {
                Some(group) => intern(group),
                None => {
                    // For ungrouped items, create a unique but interned key
                    // This avoids allocating a new string for each ungrouped function
                    intern(&func.nodeid)
                }
            }
        });
        
        Ok(distribute_groups_to_workers(groups, num_workers))
    }
}

impl Scheduler<String> for LoadGroupScheduler {
    fn distribute(&self, items: Vec<String>, num_workers: usize) -> Result<Vec<Vec<String>>, SchedulerError> {
        // For string-based distribution, we fall back to load balancing
        // since we don't have access to Function objects with xdist_group info
        LoadScheduler.distribute(items, num_workers)
    }
}

impl Scheduler<String> for LoadFileScheduler {
    fn distribute(&self, items: Vec<String>, num_workers: usize) -> Result<Vec<Vec<String>>, SchedulerError> {
        validate_worker_count!(num_workers);
        
        if items.is_empty() {
            return Ok(vec![]);
        }

        if num_workers == 1 {
            return Ok(vec![items]);
        }

        let groups = group_items_by_key(items, |s| extract_file(s));
        Ok(distribute_groups_to_workers(groups, num_workers))
    }
}


/// WorkStealScheduler implements a round-robin distribution that's optimized for 
/// work stealing scenarios. While true work stealing requires runtime coordination
/// between workers, this scheduler provides better load balancing by:
/// 1. Using round-robin assignment (avoiding clustering of slow tests)
/// 2. Interleaving tests across workers to maximize stealing opportunities
/// 3. Ensuring each worker gets a good mix of tests from different parts of the suite
pub struct WorkStealScheduler;

impl<T> Scheduler<T> for WorkStealScheduler {
    fn distribute(&self, items: Vec<T>, num_workers: usize) -> Result<Vec<Vec<T>>, SchedulerError> {
        validate_worker_count!(num_workers);
        
        if items.is_empty() {
            return Ok(vec![]);
        }

        if num_workers == 1 {
            return Ok(vec![items]);
        }

        // Pre-allocate with capacity for better performance
        let items_per_worker = (items.len() + num_workers - 1) / num_workers;
        let mut workers: Vec<Vec<T>> = Vec::with_capacity(num_workers);
        for _ in 0..num_workers {
            workers.push(Vec::with_capacity(items_per_worker));
        }
        
        // Round-robin distribution - this gives better work-stealing characteristics
        // because it interleaves tests across workers, making it more likely that
        // when one worker finishes early, there are still items available for stealing
        for (i, item) in items.into_iter().enumerate() {
            workers[i % num_workers].push(item);
        }

        workers.retain(|w| !w.is_empty());
        Ok(workers)
    }
}

pub struct NoScheduler;

impl<T> Scheduler<T> for NoScheduler {
    fn distribute(&self, items: Vec<T>, _num_workers: usize) -> Result<Vec<Vec<T>>, SchedulerError> {
        if items.is_empty() {
            Ok(vec![])
        } else {
            Ok(vec![items])
        }
    }
    
    fn min_workers(&self) -> usize {
        1
    }
    
    fn recommend_workers(&self, _item_count: usize) -> usize {
        1 // No parallelism
    }
}

/// Extract scope from test path without allocation when possible
fn extract_scope(test_path: &str) -> Arc<str> {
    // Extract module/class scope from test path
    // Format: path/to/file.py::TestClass::test_method or path/to/file.py::test_function
    match test_path.find("::") {
        None => Arc::from(test_path), // Just file path
        Some(first_sep) => {
            // Check if there's a second ::
            if let Some(second_sep) = test_path[first_sep + 2..].find("::") {
                // File::Class::method - return File::Class
                let end = first_sep + 2 + second_sep;
                Arc::from(&test_path[..end])
            } else {
                // File::function - return just File
                Arc::from(&test_path[..first_sep])
            }
        }
    }
}

/// Extract file from test path without allocation
fn extract_file(test_path: &str) -> Arc<str> {
    // Extract file path from test path
    // Format: path/to/file.py::TestClass::test_method or path/to/file.py::test_function
    match test_path.find("::") {
        None => Arc::from(test_path),
        Some(sep) => Arc::from(&test_path[..sep]),
    }
}

pub fn create_scheduler_string(mode: DistributionMode) -> Box<dyn Scheduler<String>> {
    match mode {
        DistributionMode::Load => Box::new(LoadScheduler),
        DistributionMode::LoadScope => Box::new(LoadScopeScheduler),
        DistributionMode::LoadFile => Box::new(LoadFileScheduler),
        DistributionMode::LoadGroup => Box::new(LoadGroupScheduler),
        DistributionMode::WorkSteal => Box::new(WorkStealScheduler),
        DistributionMode::No => Box::new(NoScheduler),
    }
}

pub fn create_scheduler_function(mode: DistributionMode) -> Box<dyn Scheduler<Function>> {
    match mode {
        DistributionMode::Load => Box::new(LoadScheduler),
        DistributionMode::LoadScope => Box::new(LoadScheduler), // Functions don't have scope concept
        DistributionMode::LoadFile => Box::new(LoadScheduler), // Functions don't have file concept
        DistributionMode::LoadGroup => Box::new(LoadGroupScheduler),
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
            "loadgroup".parse::<DistributionMode>(),
            Ok(DistributionMode::LoadGroup)
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
    }

    #[test]
    fn test_load_scheduler_empty_tests() {
        let scheduler = LoadScheduler;
        let result: Vec<Vec<String>> = scheduler.distribute(vec![], 4);
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_scheduler_zero_workers() {
        let scheduler = LoadScheduler;
        let tests = vec!["test1".to_string(), "test2".to_string()];
        let result = scheduler.distribute(tests, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_load_scheduler_single_worker() {
        let scheduler = LoadScheduler;
        let tests = vec![
            "test1".to_string(),
            "test2".to_string(),
            "test3".to_string(),
        ];
        let result = scheduler.distribute(tests.clone(), 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], tests);
    }

    #[test]
    fn test_load_scheduler_round_robin() {
        let scheduler = LoadScheduler;
        let tests = vec![
            "test1".to_string(),
            "test2".to_string(),
            "test3".to_string(),
            "test4".into(),
            "test5".into(),
        ];
        let result = scheduler.distribute(tests, 3);

        assert_eq!(result.len(), 3);
        assert_eq!(result[0], vec!["test1", "test4"]);
        assert_eq!(result[1], vec!["test2", "test5"]);
        assert_eq!(result[2], vec!["test3"]);
    }

    #[test]
    fn test_load_scheduler_more_workers_than_tests() {
        let scheduler = LoadScheduler;
        let tests = vec!["test1".to_string(), "test2".to_string()];
        let result = scheduler.distribute(tests, 5);

        assert_eq!(result.len(), 2); // Only non-empty workers
        assert_eq!(result[0], vec!["test1"]);
        assert_eq!(result[1], vec!["test2"]);
    }

    #[test]
    fn test_create_scheduler() {
        let scheduler = create_scheduler_string(DistributionMode::Load);
        let tests = vec!["test1".to_string(), "test2".to_string()];
        let result = scheduler.distribute(tests, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_load_scheduler_consistent_distribution() {
        let scheduler = LoadScheduler;
        let tests = vec![
            "test1".to_string(),
            "test2".to_string(),
            "test3".to_string(),
            "test4".into(),
        ];

        // Test the same distribution multiple times - should be consistent
        let result1 = scheduler.distribute(tests.clone(), 2);
        let result2 = scheduler.distribute(tests.clone(), 2);

        assert_eq!(result1, result2);
        assert_eq!(result1[0], vec!["test1", "test3"]);
        assert_eq!(result1[1], vec!["test2", "test4"]);
    }

    #[test]
    fn test_load_scheduler_all_tests_distributed() {
        let scheduler = LoadScheduler;
        let tests = vec![
            "test1".to_string(),
            "test2".to_string(),
            "test3".to_string(),
            "test4".into(),
            "test5".into(),
        ];

        let result = scheduler.distribute(tests.clone(), 3);

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
        assert_eq!(format!("{}", DistributionMode::LoadGroup), "loadgroup");
        assert_eq!(format!("{}", DistributionMode::WorkSteal), "worksteal");
        assert_eq!(format!("{}", DistributionMode::No), "no");
    }

    #[test]
    fn test_distribution_mode_from_str_error_message() {
        let error = "invalid".parse::<DistributionMode>().unwrap_err();
        let error_string = error.to_string();
        assert!(error_string.contains("Unsupported distribution mode: 'invalid'"));
        assert!(error_string.contains("Supported modes: load, loadscope, loadfile, loadgroup, worksteal, no"));
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
        let result = scheduler.distribute(tests, 4);

        // Should have 4 groups: file1::TestClass1, file1, file2::TestClass2, file2
        assert_eq!(result.len(), 4);
        
        // Verify that tests with same scope are grouped together
        let mut scope_to_worker: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        
        for (worker_idx, worker_tests) in result.iter().enumerate() {
            for test in worker_tests {
                let scope = extract_scope(test);
                if let Some(&existing_worker) = scope_to_worker.get(&scope) {
                    assert_eq!(existing_worker, worker_idx, 
                        "Test {} should be in same worker as other tests from scope {}", test, scope);
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
        let result = scheduler.distribute(tests.clone(), 1);
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
        let result = scheduler.distribute(tests, 2);

        assert_eq!(result.len(), 2);
        
        // Verify that each file's tests are kept together
        let mut file_to_worker: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        
        for (worker_idx, worker_tests) in result.iter().enumerate() {
            for test in worker_tests {
                let file = extract_file(test);
                if let Some(&existing_worker) = file_to_worker.get(&file) {
                    assert_eq!(existing_worker, worker_idx, 
                        "Test {} should be in same worker as other tests from {}", test, file);
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
            "test1".to_string(),
            "test2".to_string(),
            "test3".to_string(),
            "test4".into(),
            "test5".into(),
            "test6".into(),
        ];
        let result = scheduler.distribute(tests, 3);

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
            "test1".to_string(),
            "test2".to_string(),
            "test3".to_string(),
            "test4".into(),
            "test5".into(),
        ];
        let result = scheduler.distribute(tests, 3);

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
        let result = scheduler.distribute(tests, 2);

        assert_eq!(result.len(), 2);
        // Tests should be interleaved across workers for better work stealing
        assert_eq!(result[0], vec!["fast_test1", "fast_test2"]);
        assert_eq!(result[1], vec!["slow_test1", "slow_test2"]);
    }

    // No scheduler tests
    #[test]
    fn test_no_scheduler_single_group() {
        let scheduler = NoScheduler;
        let tests = vec![
            "test1".to_string(),
            "test2".to_string(),
            "test3".to_string(),
        ];
        let result = scheduler.distribute_tests(tests.clone(), 5);

        assert_eq!(result.len(), 1);
        assert_eq!(result[0], tests);
    }

    #[test]
    fn test_no_scheduler_empty_tests() {
        let scheduler = NoScheduler;
        let result = scheduler.distribute(vec![], 3);
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
        assert_eq!(
            extract_scope("tests/test_file.py"),
            "tests/test_file.py"
        );
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
        assert_eq!(
            extract_file("tests/test_file.py"),
            "tests/test_file.py"
        );
    }

    // Create scheduler tests for all modes
    #[test]
    fn test_create_scheduler_all_modes() {
        let load_scheduler = create_scheduler_string(DistributionMode::Load);
        let loadscope_scheduler = create_scheduler_string(DistributionMode::LoadScope);
        let loadfile_scheduler = create_scheduler_string(DistributionMode::LoadFile);
        let loadgroup_scheduler = create_scheduler_string(DistributionMode::LoadGroup);
        let worksteal_scheduler = create_scheduler_string(DistributionMode::WorkSteal);
        let no_scheduler = create_scheduler_string(DistributionMode::No);

        let tests = vec!["test1".to_string(), "test2".to_string()];
        
        assert_eq!(load_scheduler.distribute(tests.clone(), 2).len(), 2);
        assert_eq!(loadscope_scheduler.distribute(tests.clone(), 2).len(), 2);
        assert_eq!(loadfile_scheduler.distribute(tests.clone(), 2).len(), 2);
        assert_eq!(loadgroup_scheduler.distribute(tests.clone(), 2).len(), 2);
        assert_eq!(worksteal_scheduler.distribute(tests.clone(), 2).len(), 2);
        assert_eq!(no_scheduler.distribute(tests.clone(), 2).len(), 1);
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
        let result1 = scheduler.distribute(tests.clone(), 2);
        let result2 = scheduler.distribute(tests.clone(), 2);
        let result3 = scheduler.distribute_tests(tests.clone(), 2);
        
        assert_eq!(result1, result2);
        assert_eq!(result2, result3);
        
        // Verify that the same files end up together across runs
        // This tests the key benefit: deterministic grouping
        for (worker_idx, worker_tests) in result1.iter().enumerate() {
            let mut files_in_worker: std::collections::HashSet<String> = std::collections::HashSet::new();
            for test in worker_tests {
                files_in_worker.insert(extract_file(test));
            }
            
            // Verify the same files are in the same worker in all runs
            let mut files_in_worker2: std::collections::HashSet<String> = std::collections::HashSet::new();
            for test in &result2[worker_idx] {
                files_in_worker2.insert(extract_file(test));
            }
            
            assert_eq!(files_in_worker, files_in_worker2, 
                "Worker {} should have the same files across runs", worker_idx);
        }
    }

    // LoadGroup scheduler tests
    #[test]
    fn test_loadgroup_scheduler_groups_by_xdist_group() {
        use crate::collection::{Function, Location};
        use std::path::PathBuf;

        let scheduler = LoadGroupScheduler;
        
        let functions = vec![
            Function {
                name: "test1".into(),
                nodeid: "file1.py::test1".into(),
                location: Location { path: PathBuf::from("file1.py"), line: Some(1), name: "test1".into() },
                xdist_group: Some("database".into()),
            },
            Function {
                name: "test2".into(),
                nodeid: "file1.py::test2".into(),
                location: Location { path: PathBuf::from("file1.py"), line: Some(2), name: "test2".into() },
                xdist_group: Some("database".into()),
            },
            Function {
                name: "test3".into(),
                nodeid: "file2.py::test3".into(),
                location: Location { path: PathBuf::from("file2.py"), line: Some(1), name: "test3".into() },
                xdist_group: Some("ui".into()),
            },
            Function {
                name: "test4".into(),
                nodeid: "file2.py::test4".into(),
                location: Location { path: PathBuf::from("file2.py"), line: Some(2), name: "test4".into() },
                xdist_group: None,
            },
        ];

        let result = scheduler.distribute(functions, 3);
        
        // Should have 3 groups: database, ui, and ungrouped test4
        assert_eq!(result.len(), 3);
        
        // Verify that tests with same group are together
        let mut group_to_worker: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
        
        for (worker_idx, worker_functions) in result.iter().enumerate() {
            for func in worker_functions {
                let group = func.xdist_group.clone().unwrap_or_else(|| format!("__ungrouped__{}", func.nodeid));
                if let Some(&existing_worker) = group_to_worker.get(&group) {
                    assert_eq!(existing_worker, worker_idx, 
                        "Function {} should be in same worker as other functions from group {}", func.nodeid, group);
                } else {
                    group_to_worker.insert(group, worker_idx);
                }
            }
        }
        
        // Verify all functions are distributed
        let total_functions: usize = result.iter().map(|w| w.len()).sum();
        assert_eq!(total_functions, 4);
    }

    #[test]
    fn test_loadgroup_scheduler_single_worker() {
        use crate::collection::{Function, Location};
        use std::path::PathBuf;

        let scheduler = LoadGroupScheduler;
        
        let functions = vec![
            Function {
                name: "test1".into(),
                nodeid: "file1.py::test1".into(),
                location: Location { path: PathBuf::from("file1.py"), line: Some(1), name: "test1".into() },
                xdist_group: Some("group1".into()),
            },
            Function {
                name: "test2".into(),
                nodeid: "file2.py::test2".into(),
                location: Location { path: PathBuf::from("file2.py"), line: Some(1), name: "test2".into() },
                xdist_group: Some("group2".into()),
            },
        ];

        let result = scheduler.distribute(functions.clone(), 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].len(), 2);
    }

    #[test]
    fn test_loadgroup_scheduler_empty_functions() {
        let scheduler = LoadGroupScheduler;
        let result = scheduler.distribute(vec![], 4);
        assert!(result.is_empty());
    }

    #[test]
    fn test_loadgroup_scheduler_zero_workers() {
        use crate::collection::{Function, Location};
        use std::path::PathBuf;

        let scheduler = LoadGroupScheduler;
        
        let functions = vec![
            Function {
                name: "test1".into(),
                nodeid: "file1.py::test1".into(),
                location: Location { path: PathBuf::from("file1.py"), line: Some(1), name: "test1".into() },
                xdist_group: Some("group1".into()),
            },
        ];

        let result = scheduler.distribute(functions, 0);
        assert!(result.is_empty());
    }
}
