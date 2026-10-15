use crate::cli::NumProcesses;
use std::num::NonZeroUsize;

pub fn determine_worker_count(
    num_processes: Option<NumProcesses>,
    max_processes: Option<NonZeroUsize>,
) -> NonZeroUsize {
    let desired = match num_processes {
        Some(NumProcesses::Auto) => {
            NonZeroUsize::new(num_cpus::get_physical()).unwrap_or(NonZeroUsize::MIN)
        }
        Some(NumProcesses::Logical) => {
            NonZeroUsize::new(num_cpus::get()).unwrap_or(NonZeroUsize::MIN)
        }
        Some(NumProcesses::Count(n)) => n,
        None => NonZeroUsize::MIN,
    };

    desired.min(max_processes.unwrap_or(desired))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workers(value: usize) -> NonZeroUsize {
        NonZeroUsize::new(value).expect("positive test worker count")
    }

    #[test]
    fn test_determine_worker_count_none() {
        let count = determine_worker_count(None, None);
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn test_determine_worker_count_with_max() {
        let count = determine_worker_count(Some(NumProcesses::Count(workers(8))), Some(workers(4)));
        assert_eq!(count.get(), 4);
    }

    #[test]
    fn test_determine_worker_count_under_max() {
        let count = determine_worker_count(Some(NumProcesses::Count(workers(2))), Some(workers(8)));
        assert_eq!(count.get(), 2);
    }

    #[test]
    fn test_determine_worker_count_auto_vs_logical() {
        let auto_count = determine_worker_count(Some(NumProcesses::Auto), None);
        let logical_count = determine_worker_count(Some(NumProcesses::Logical), None);

        // Auto (physical cores) should be <= logical cores
        assert!(auto_count <= logical_count);
    }

    #[test]
    fn test_determine_worker_count_auto_with_max() {
        let count = determine_worker_count(Some(NumProcesses::Auto), Some(workers(1)));
        assert_eq!(count.get(), 1);
    }

    #[test]
    fn test_determine_worker_count_logical_with_max() {
        let count = determine_worker_count(Some(NumProcesses::Logical), Some(workers(2)));
        assert!(count.get() <= 2);
    }
}
