use std::fmt;
use std::path::PathBuf;
use std::process::Output;
use std::thread;

use crate::pytest_executor::pytest_command;

#[derive(Debug)]
pub enum WorkerError {
    Launch(std::io::Error),
    Panicked,
}

impl fmt::Display for WorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Launch(error) => write!(f, "Failed to execute command: {error}"),
            Self::Panicked => write!(f, "Worker thread panicked"),
        }
    }
}

impl std::error::Error for WorkerError {}

#[derive(Debug)]
pub struct WorkerResult {
    pub worker_id: usize,
    pub result: Result<Output, WorkerError>,
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
    handle: thread::JoinHandle<std::io::Result<Output>>,
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
            pytest_command(
                &task.program,
                &task.initial_args,
                &task.tests,
                &task.pytest_args,
                task.working_dir.as_deref(),
                &task.env_vars,
            )
            .output()
        });

        self.workers.push(WorkerHandle {
            id: worker_id,
            handle,
        });
    }

    pub fn wait_for_all(self) -> Vec<WorkerResult> {
        let mut results: Vec<_> = self
            .workers
            .into_iter()
            .map(|worker| WorkerResult {
                worker_id: worker.id,
                result: worker
                    .handle
                    .join()
                    .map_err(|_| WorkerError::Panicked)
                    .and_then(|result| result.map_err(WorkerError::Launch)),
            })
            .collect();
        results.sort_by_key(|result| result.worker_id);
        results
    }

    #[allow(dead_code)]
    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }
}
