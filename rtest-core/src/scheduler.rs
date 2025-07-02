use std::fmt;

#[derive(Debug, Clone)]
pub enum DistributionMode {
    Load,
    // Future implementations:
    // LoadScope,
    // LoadFile,
    // LoadGroup,
    // WorkSteal,
}

impl fmt::Display for DistributionMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DistributionMode::Load => write!(f, "load"),
        }
    }
}

impl std::str::FromStr for DistributionMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "load" => Ok(DistributionMode::Load),
            other => Err(format!(
                "Distribution mode '{other}' is not yet implemented. Only 'load' is supported."
            )),
        }
    }
}

pub trait Scheduler {
    fn distribute_tests(&self, tests: Vec<String>, num_workers: usize) -> Vec<Vec<String>>;
}

pub struct LoadScheduler;

impl Scheduler for LoadScheduler {
    fn distribute_tests(&self, tests: Vec<String>, num_workers: usize) -> Vec<Vec<String>> {
        if num_workers == 0 || tests.is_empty() {
            return vec![];
        }

        if num_workers == 1 {
            return vec![tests];
        }

        let mut workers: Vec<Vec<String>> = vec![Vec::new(); num_workers];

        for (i, test) in tests.into_iter().enumerate() {
            workers[i % num_workers].push(test);
        }

        workers.into_iter().filter(|w| !w.is_empty()).collect()
    }
}

pub fn create_scheduler(mode: DistributionMode) -> Box<dyn Scheduler> {
    match mode {
        DistributionMode::Load => Box::new(LoadScheduler),
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
        assert!("loadfile".parse::<DistributionMode>().is_err());
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
        let tests = vec!["test1".to_string(), "test2".to_string()];
        let result = scheduler.distribute_tests(tests, 0);
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
        let result = scheduler.distribute_tests(tests.clone(), 1);
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
            "test4".to_string(),
            "test5".to_string(),
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
        let tests = vec!["test1".to_string(), "test2".to_string()];
        let result = scheduler.distribute_tests(tests, 5);

        assert_eq!(result.len(), 2); // Only non-empty workers
        assert_eq!(result[0], vec!["test1"]);
        assert_eq!(result[1], vec!["test2"]);
    }

    #[test]
    fn test_create_scheduler() {
        let scheduler = create_scheduler(DistributionMode::Load);
        let tests = vec!["test1".to_string(), "test2".to_string()];
        let result = scheduler.distribute_tests(tests, 2);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_load_scheduler_consistent_distribution() {
        let scheduler = LoadScheduler;
        let tests = vec![
            "test1".to_string(),
            "test2".to_string(),
            "test3".to_string(),
            "test4".to_string(),
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
            "test1".to_string(),
            "test2".to_string(),
            "test3".to_string(),
            "test4".to_string(),
            "test5".to_string(),
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
    }

    #[test]
    fn test_distribution_mode_from_str_error_message() {
        let error = "invalid".parse::<DistributionMode>().unwrap_err();
        assert!(error.contains("Distribution mode 'invalid' is not yet implemented"));
        assert!(error.contains("Only 'load' is supported"));
    }
}
