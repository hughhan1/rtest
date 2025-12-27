use crate::cli::NumProcesses;

pub fn determine_worker_count(
    num_processes: Option<NumProcesses>,
    max_processes: Option<usize>,
) -> usize {
    let desired = match num_processes {
        Some(NumProcesses::Auto) => num_cpus::get_physical(),
        Some(NumProcesses::Logical) => num_cpus::get(),
        Some(NumProcesses::Count(n)) => n,
        None => 1,
    };

    match max_processes {
        Some(max) => desired.min(max),
        None => desired,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_worker_count_none() {
        let count = determine_worker_count(None, None);
        assert_eq!(count, 1);
    }

    #[test]
    fn test_determine_worker_count_with_max() {
        let count = determine_worker_count(Some(NumProcesses::Count(8)), Some(4));
        assert_eq!(count, 4);
    }

    #[test]
    fn test_determine_worker_count_under_max() {
        let count = determine_worker_count(Some(NumProcesses::Count(2)), Some(8));
        assert_eq!(count, 2);
    }

    #[test]
    fn test_determine_worker_count_auto() {
        let count = determine_worker_count(Some(NumProcesses::Auto), None);
        assert!(count >= 1);
    }

    #[test]
    fn test_determine_worker_count_logical() {
        let count = determine_worker_count(Some(NumProcesses::Logical), None);
        assert!(count >= 1);
    }

    #[test]
    fn test_determine_worker_count_auto_vs_logical() {
        let auto_count = determine_worker_count(Some(NumProcesses::Auto), None);
        let logical_count = determine_worker_count(Some(NumProcesses::Logical), None);

        // Auto (physical cores) should be <= logical cores
        assert!(auto_count <= logical_count);
        assert!(auto_count >= 1);
        assert!(logical_count >= 1);
    }

    #[test]
    fn test_determine_worker_count_zero_max() {
        let count = determine_worker_count(Some(NumProcesses::Count(4)), Some(0));
        assert_eq!(count, 0);
    }

    #[test]
    fn test_determine_worker_count_explicit_zero() {
        let count = determine_worker_count(Some(NumProcesses::Count(0)), None);
        assert_eq!(count, 0);
    }

    #[test]
    fn test_determine_worker_count_large_numbers() {
        let count = determine_worker_count(Some(NumProcesses::Count(1000)), Some(500));
        assert_eq!(count, 500);

        let count = determine_worker_count(Some(NumProcesses::Count(100)), Some(1000));
        assert_eq!(count, 100);
    }

    #[test]
    fn test_determine_worker_count_auto_with_max() {
        let count = determine_worker_count(Some(NumProcesses::Auto), Some(1));
        assert_eq!(count, 1);
    }

    #[test]
    fn test_determine_worker_count_logical_with_max() {
        let count = determine_worker_count(Some(NumProcesses::Logical), Some(2));
        assert!(count <= 2);
        assert!(count >= 1);
    }

    #[test]
    fn test_maxprocesses_integration() {
        // Test that maxprocesses actually limits worker count
        let count = determine_worker_count(Some(NumProcesses::Count(10)), Some(3));
        assert_eq!(count, 3, "maxprocesses should limit worker count to 3");

        let count = determine_worker_count(Some(NumProcesses::Count(2)), Some(10));
        assert_eq!(
            count, 2,
            "worker count should not exceed requested when under limit"
        );

        // Test with auto/logical modes
        let count = determine_worker_count(Some(NumProcesses::Auto), Some(1));
        assert_eq!(count, 1, "maxprocesses should limit auto detection");

        let count = determine_worker_count(Some(NumProcesses::Logical), Some(1));
        assert_eq!(count, 1, "maxprocesses should limit logical detection");
    }
}
