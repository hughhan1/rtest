use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;

/// Result from a worker process that executed a batch of tests.
/// 
/// Used in traditional distribution modes (load, loadscope, loadfile) where
/// tests are pre-assigned to workers in batches. Each worker process executes
/// multiple tests in a single pytest invocation, and this struct captures the
/// combined output and exit status of that batch execution.
#[derive(Debug)]
pub struct WorkerResult {
    /// Identifier of the worker that produced this result
    pub worker_id: usize,
    /// Exit code from the pytest process (0 = success, non-zero = failure)
    pub exit_code: i32,
    /// Combined stdout from all tests executed by this worker
    pub stdout: String,
    /// Combined stderr from all tests executed by this worker
    pub stderr: String,
}

pub struct WorkerPool {
    workers: Vec<WorkerHandle>,
}

struct WorkerHandle {
    id: usize,
    handle: thread::JoinHandle<WorkerResult>,
}

impl Default for WorkerPool {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkerPool {
    pub fn new() -> Self {
        Self {
            workers: Vec::new(),
        }
    }

    pub fn spawn_worker(
        &mut self,
        worker_id: usize,
        program: String,
        initial_args: Vec<String>,
        tests: Vec<String>,
        pytest_args: Vec<String>,
        working_dir: Option<PathBuf>,
    ) {
        let handle = thread::spawn(move || {
            let mut cmd = Command::new(&program);

            for arg in initial_args {
                cmd.arg(arg);
            }

            // Add --rootdir to prevent pytest from traversing up the directory tree during
            // its collection phase. Without this, pytest searches upward for config files
            // and can hit protected Windows system directories like "C:\Documents and Settings",
            // causing PermissionError even when we provide explicit test node IDs.
            if let Some(ref dir) = working_dir {
                cmd.arg("--rootdir");
                cmd.arg(dir);
            }

            for test in tests {
                cmd.arg(test);
            }

            for arg in pytest_args {
                cmd.arg(arg);
            }

            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            if let Some(dir) = working_dir {
                cmd.current_dir(dir);
            }

            match cmd.output() {
                Ok(output) => WorkerResult {
                    worker_id,
                    exit_code: output.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                },
                Err(e) => WorkerResult {
                    worker_id,
                    exit_code: -1,
                    stdout: String::new(),
                    stderr: format!("Failed to execute command: {e}"),
                },
            }
        });

        self.workers.push(WorkerHandle {
            id: worker_id,
            handle,
        });
    }

    pub fn wait_for_all(self) -> Vec<WorkerResult> {
        let mut results = Vec::new();

        for worker in self.workers {
            match worker.handle.join() {
                Ok(result) => results.push(result),
                Err(_) => {
                    results.push(WorkerResult {
                        worker_id: worker.id,
                        exit_code: -1,
                        stdout: String::new(),
                        stderr: "Worker thread panicked".into(),
                    });
                }
            }
        }

        results.sort_by_key(|r| r.worker_id);
        results
    }

    #[allow(dead_code)]
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_pool_creation() {
        let pool = WorkerPool::new();
        assert_eq!(pool.worker_count(), 0);
    }

    #[test]
    fn test_worker_result_creation() {
        let result = WorkerResult {
            worker_id: 1,
            exit_code: 0,
            stdout: "test output".into(),
            stderr: String::new(),
        };
        assert_eq!(result.worker_id, 1);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout, "test output");
    }

    #[test]
    fn test_spawn_and_wait_echo_command() {
        let mut pool = WorkerPool::new();

        // Use echo command for testing
        pool.spawn_worker(
            0,
            "echo".into(),
            vec![],
            vec!["hello".into()],
            vec!["world".into()],
            None,
        );

        assert_eq!(pool.worker_count(), 1);

        let results = pool.wait_for_all();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].worker_id, 0);
        assert_eq!(results[0].exit_code, 0);
        assert!(results[0].stdout.contains("hello"));
        assert!(results[0].stdout.contains("world"));
    }

    #[test]
    fn test_multiple_workers() {
        let mut pool = WorkerPool::new();

        // Spawn multiple echo workers
        pool.spawn_worker(
            0,
            "echo".into(),
            vec![],
            vec!["worker0".into()],
            vec![],
            None,
        );

        pool.spawn_worker(
            1,
            "echo".into(),
            vec![],
            vec!["worker1".into()],
            vec![],
            None,
        );

        assert_eq!(pool.worker_count(), 2);

        let results = pool.wait_for_all();
        assert_eq!(results.len(), 2);

        // Results should be sorted by worker_id
        assert_eq!(results[0].worker_id, 0);
        assert_eq!(results[1].worker_id, 1);
        assert!(results[0].stdout.contains("worker0"));
        assert!(results[1].stdout.contains("worker1"));
    }

    #[test]
    fn test_worker_failure() {
        let mut pool = WorkerPool::new();

        // Use a command that should fail
        pool.spawn_worker(
            0,
            "false".into(), // Command that always exits with code 1
            vec![],
            vec![],
            vec![],
            None,
        );

        let results = pool.wait_for_all();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].worker_id, 0);
        assert_ne!(results[0].exit_code, 0); // Should be non-zero exit code
    }

    #[test]
    fn test_worker_with_initial_args() {
        let mut pool = WorkerPool::new();

        // Test with initial args (like "python -m pytest")
        pool.spawn_worker(
            0,
            "echo".into(),
            vec!["initial".into(), "args".into()],
            vec!["test1".into()],
            vec!["final".into()],
            None,
        );

        let results = pool.wait_for_all();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].exit_code, 0);

        let output = &results[0].stdout;
        assert!(output.contains("initial"));
        assert!(output.contains("args"));
        assert!(output.contains("test1"));
        assert!(output.contains("final"));
    }

    #[test]
    fn test_worker_invalid_command() {
        let mut pool = WorkerPool::new();

        // Use a command that doesn't exist
        pool.spawn_worker(
            0,
            "nonexistent_command_12345".into(),
            vec![],
            vec![],
            vec![],
            None,
        );

        let results = pool.wait_for_all();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].worker_id, 0);
        assert_eq!(results[0].exit_code, -1);
        assert!(results[0].stderr.contains("Failed to execute command"));
    }

    #[test]
    fn test_worker_result_sorting() {
        let mut pool = WorkerPool::new();

        // Spawn workers in reverse order
        pool.spawn_worker(
            2,
            "echo".into(),
            vec![],
            vec!["worker2".into()],
            vec![],
            None,
        );

        pool.spawn_worker(
            0,
            "echo".into(),
            vec![],
            vec!["worker0".into()],
            vec![],
            None,
        );

        pool.spawn_worker(
            1,
            "echo".into(),
            vec![],
            vec!["worker1".into()],
            vec![],
            None,
        );

        let results = pool.wait_for_all();
        assert_eq!(results.len(), 3);

        // Should be sorted by worker_id regardless of spawn order
        assert_eq!(results[0].worker_id, 0);
        assert_eq!(results[1].worker_id, 1);
        assert_eq!(results[2].worker_id, 2);
    }

    #[test]
    fn test_empty_worker_pool() {
        let pool = WorkerPool::new();
        let results = pool.wait_for_all();
        assert!(results.is_empty());
    }
}

/// Aggregated statistics from work-stealing test execution.
/// 
/// This struct summarizes the results of executing tests in work-stealing mode,
/// where each test runs in isolation. It provides counts of test outcomes and
/// an overall exit code, derived from the individual TestExecutionResults.
#[derive(Debug)]
pub struct WorkStealingResult {
    /// Total number of tests that were executed
    pub total_tests: usize,
    /// Number of tests that passed (exit code 0)
    pub passed: usize,
    /// Number of tests that failed (non-zero exit code, excluding skipped)
    pub failed: usize,
    /// Number of tests that were skipped (pytest exit code 5)
    pub skipped: usize,
    /// Overall exit code (0 if all passed/skipped, otherwise first failure code)
    pub exit_code: i32,
}

/// Result from executing a single test in isolation.
/// 
/// Used in work-stealing mode where each test is executed independently in its
/// own pytest process. This fine-grained execution model enables dynamic load
/// balancing through rayon's work-stealing scheduler and provides real-time
/// progress reporting as individual tests complete.
#[derive(Debug)]
struct TestExecutionResult {
    /// The specific test node that was executed (e.g., "test_file.py::test_func")
    pub test_id: String,
    /// Exit code from the pytest process for this single test
    pub exit_code: i32,
    /// Stdout from this test's execution
    pub stdout: String,
    /// Stderr from this test's execution
    pub stderr: String,
}

/// Execute tests using work-stealing parallelism with rayon
pub fn execute_work_stealing(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    _worker_count: usize,
    rootpath: &Path,
) -> WorkStealingResult {
    let config = TestExecutionConfig {
        program: program.to_string(),
        initial_args: initial_args.to_vec(),
        rootpath: rootpath.to_path_buf(),
    };

    // Execute tests in parallel using work-stealing
    let results: Vec<TestExecutionResult> = test_nodes
        .par_iter()
        .map(|test| execute_single_test(test, &config))
        .collect();

    // Aggregate results
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;
    let mut overall_exit_code = 0;

    for result in &results {
        match result.exit_code {
            0 => passed += 1,
            5 => skipped += 1,  // pytest exit code 5 means no tests collected
            _ => {
                failed += 1;
                if overall_exit_code == 0 {
                    overall_exit_code = result.exit_code;
                }
            }
        }

        // Print output as we process results
        if !result.stdout.is_empty() {
            print!("{}", result.stdout);
        }
        if !result.stderr.is_empty() {
            eprintln!("[{}] {}", result.test_id, result.stderr.trim());
        }
    }

    WorkStealingResult {
        total_tests: test_nodes.len(),
        passed,
        failed,
        skipped,
        exit_code: overall_exit_code,
    }
}

/// Configuration for test execution
struct TestExecutionConfig {
    program: String,
    initial_args: Vec<String>,
    rootpath: PathBuf,
}

/// Execute a single test node
fn execute_single_test(test_node: &str, config: &TestExecutionConfig) -> TestExecutionResult {
    let mut cmd = Command::new(&config.program);
    
    // Add initial args (e.g., ["-m", "pytest"])
    for arg in &config.initial_args {
        cmd.arg(arg);
    }
    
    // Add rootdir to prevent pytest from searching upward
    cmd.arg("--rootdir");
    cmd.arg(&config.rootpath);
    
    // Add the test node
    cmd.arg(test_node);
    
    cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .current_dir(&config.rootpath);
    
    match cmd.output() {
        Ok(output) => TestExecutionResult {
            test_id: test_node.to_string(),
            exit_code: output.status.code().unwrap_or(-1),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        },
        Err(e) => TestExecutionResult {
            test_id: test_node.to_string(),
            exit_code: -1,
            stdout: String::new(),
            stderr: format!("Failed to execute test: {}", e),
        },
    }
}
