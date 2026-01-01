use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

#[derive(Debug)]
pub struct WorkerResult {
    pub worker_id: usize,
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Configuration for a worker task
pub struct WorkerTask {
    pub worker_id: usize,
    pub program: String,
    pub initial_args: Vec<String>,
    pub tests: Vec<String>,
    pub pytest_args: Vec<String>,
    pub working_dir: Option<PathBuf>,
    pub env_vars: Vec<(String, String)>,
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

    pub fn spawn_worker(&mut self, task: WorkerTask) {
        let worker_id = task.worker_id;
        let handle = thread::spawn(move || {
            let mut cmd = Command::new(&task.program);

            // Set environment variables
            for (key, value) in task.env_vars {
                cmd.env(key, value);
            }

            for arg in task.initial_args {
                cmd.arg(arg);
            }

            // Add --rootdir to prevent pytest from traversing up the directory tree during
            // its collection phase. Without this, pytest searches upward for config files
            // and can hit protected Windows system directories like "C:\Documents and Settings",
            // causing PermissionError even when we provide explicit test node IDs.
            if let Some(ref dir) = task.working_dir {
                cmd.arg("--rootdir");
                cmd.arg(dir);
            }

            for test in task.tests {
                cmd.arg(test);
            }

            for arg in task.pytest_args {
                cmd.arg(arg);
            }

            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            if let Some(dir) = task.working_dir {
                cmd.current_dir(dir);
            }

            match cmd.output() {
                Ok(output) => WorkerResult {
                    worker_id: task.worker_id,
                    exit_code: output.status.code().unwrap_or(-1),
                    stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                    stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
                },
                Err(e) => WorkerResult {
                    worker_id: task.worker_id,
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
        pool.spawn_worker(WorkerTask {
            worker_id: 0,
            program: "echo".into(),
            initial_args: vec![],
            tests: vec!["hello".into()],
            pytest_args: vec!["world".into()],
            working_dir: None,
            env_vars: vec![],
        });

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
        pool.spawn_worker(WorkerTask {
            worker_id: 0,
            program: "echo".into(),
            initial_args: vec![],
            tests: vec!["worker0".into()],
            pytest_args: vec![],
            working_dir: None,
            env_vars: vec![],
        });

        pool.spawn_worker(WorkerTask {
            worker_id: 1,
            program: "echo".into(),
            initial_args: vec![],
            tests: vec!["worker1".into()],
            pytest_args: vec![],
            working_dir: None,
            env_vars: vec![],
        });

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
        pool.spawn_worker(WorkerTask {
            worker_id: 0,
            program: "false".into(), // Command that always exits with code 1
            initial_args: vec![],
            tests: vec![],
            pytest_args: vec![],
            working_dir: None,
            env_vars: vec![],
        });

        let results = pool.wait_for_all();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].worker_id, 0);
        assert_ne!(results[0].exit_code, 0); // Should be non-zero exit code
    }

    #[test]
    fn test_worker_with_initial_args() {
        let mut pool = WorkerPool::new();

        // Test with initial args (like "python -m pytest")
        pool.spawn_worker(WorkerTask {
            worker_id: 0,
            program: "echo".into(),
            initial_args: vec!["initial".into(), "args".into()],
            tests: vec!["test1".into()],
            pytest_args: vec!["final".into()],
            working_dir: None,
            env_vars: vec![],
        });

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
        pool.spawn_worker(WorkerTask {
            worker_id: 0,
            program: "nonexistent_command_12345".into(),
            initial_args: vec![],
            tests: vec![],
            pytest_args: vec![],
            working_dir: None,
            env_vars: vec![],
        });

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
        pool.spawn_worker(WorkerTask {
            worker_id: 2,
            program: "echo".into(),
            initial_args: vec![],
            tests: vec!["worker2".into()],
            pytest_args: vec![],
            working_dir: None,
            env_vars: vec![],
        });

        pool.spawn_worker(WorkerTask {
            worker_id: 0,
            program: "echo".into(),
            initial_args: vec![],
            tests: vec!["worker0".into()],
            pytest_args: vec![],
            working_dir: None,
            env_vars: vec![],
        });

        pool.spawn_worker(WorkerTask {
            worker_id: 1,
            program: "echo".into(),
            initial_args: vec![],
            tests: vec!["worker1".into()],
            pytest_args: vec![],
            working_dir: None,
            env_vars: vec![],
        });

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
