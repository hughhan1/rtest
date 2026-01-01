use crate::{create_scheduler, subproject, DistributionMode, WorkerPool, WorkerTask};
use std::path::Path;

pub struct PytestRunner {
    pub program: String,
    pub initial_args: Vec<String>,
    pub env_vars: Vec<(String, String)>,
}

/// Configuration for parallel test execution
pub struct ParallelExecutionConfig<'a> {
    pub program: &'a str,
    pub initial_args: &'a [String],
    pub worker_count: usize,
    pub dist_mode: &'a str,
    pub rootpath: &'a Path,
    pub use_subprojects: bool,
    pub env_vars: &'a [(String, String)],
}

impl PytestRunner {
    pub fn new(env_vars: Vec<String>) -> Self {
        let program = "python3".into();
        let initial_args = vec!["-m".into(), "pytest".into()];

        // Parse environment variables from KEY=VALUE format
        let parsed_env_vars: Vec<(String, String)> = env_vars
            .iter()
            .filter_map(|env_str| {
                if let Some((key, value)) = env_str.split_once('=') {
                    Some((key.to_string(), value.to_string()))
                } else {
                    eprintln!("Warning: Invalid environment variable format: {}", env_str);
                    None
                }
            })
            .collect();

        println!("Pytest command: {} {}", program, initial_args.join(" "));

        PytestRunner {
            program,
            initial_args,
            env_vars: parsed_env_vars,
        }
    }
}

/// Execute tests in parallel across multiple workers
pub fn execute_tests_parallel(config: &ParallelExecutionConfig, test_nodes: Vec<String>) -> i32 {
    println!(
        "Running tests with {} workers using {} distribution",
        config.worker_count, config.dist_mode
    );

    let distribution_mode = match config.dist_mode.parse::<DistributionMode>() {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("Invalid distribution mode '{}': {e}", config.dist_mode);
            return 1;
        }
    };

    if config.use_subprojects {
        let test_groups = subproject::group_tests_by_subproject(config.rootpath, &test_nodes);

        let mut worker_pool = WorkerPool::new();
        let mut worker_id = 0;

        for (subproject_root, tests) in test_groups {
            let adjusted_tests = if subproject_root != config.rootpath {
                subproject::make_test_paths_relative(&tests, config.rootpath, &subproject_root)
            } else {
                tests
            };

            let scheduler = create_scheduler(distribution_mode.clone());
            let test_batches = scheduler.distribute_tests(adjusted_tests, config.worker_count);

            for batch in test_batches {
                if !batch.is_empty() {
                    worker_pool.spawn_worker(WorkerTask {
                        worker_id,
                        program: config.program.to_string(),
                        initial_args: config.initial_args.to_vec(),
                        tests: batch,
                        pytest_args: vec![],
                        working_dir: Some(subproject_root.clone()),
                        env_vars: config.env_vars.to_vec(),
                    });
                    worker_id += 1;
                }
            }
        }

        if worker_id == 0 {
            println!("No test batches to execute.");
            return 0;
        }

        let results = worker_pool.wait_for_all();

        let mut overall_exit_code = 0;
        for result in results {
            println!("=== Worker {} ===", result.worker_id);
            if !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }

            if result.exit_code != 0 {
                overall_exit_code = result.exit_code;
            }
        }

        overall_exit_code
    } else {
        let scheduler = create_scheduler(distribution_mode);
        let test_batches = scheduler.distribute_tests(test_nodes, config.worker_count);

        if test_batches.is_empty() {
            println!("No test batches to execute.");
            return 0;
        }

        let mut worker_pool = WorkerPool::new();

        for (worker_id, tests) in test_batches.into_iter().enumerate() {
            if !tests.is_empty() {
                worker_pool.spawn_worker(WorkerTask {
                    worker_id,
                    program: config.program.to_string(),
                    initial_args: config.initial_args.to_vec(),
                    tests,
                    pytest_args: vec![],
                    working_dir: Some(config.rootpath.to_path_buf()),
                    env_vars: config.env_vars.to_vec(),
                });
            }
        }

        let results = worker_pool.wait_for_all();

        let mut overall_exit_code = 0;
        for result in results {
            println!("=== Worker {} ===", result.worker_id);
            if !result.stdout.is_empty() {
                print!("{}", result.stdout);
            }
            if !result.stderr.is_empty() {
                eprint!("{}", result.stderr);
            }

            if result.exit_code != 0 {
                overall_exit_code = result.exit_code;
            }
        }

        overall_exit_code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_python_runner() {
        let runner = PytestRunner::new(vec![]);

        assert_eq!(runner.program, "python3");
        assert_eq!(runner.initial_args, vec!["-m", "pytest"]);
    }

    #[test]
    fn test_env_vars_acknowledged() {
        let env_vars = vec!["DEBUG=1".into(), "TEST_ENV=staging".into()];
        let runner = PytestRunner::new(env_vars);

        // The runner should be created successfully
        // (Environment variables are currently just acknowledged, not stored)
        assert_eq!(runner.program, "python3");
        assert_eq!(runner.initial_args, vec!["-m", "pytest"]);
    }
}
