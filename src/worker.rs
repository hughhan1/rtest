use std::path::PathBuf;
use std::thread;

use crate::pytest_executor::pytest_command;

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
            match pytest_command(
                &task.program,
                &task.initial_args,
                &task.tests,
                &task.pytest_args,
                task.working_dir.as_deref(),
                &task.env_vars,
            )
            .output()
            {
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
