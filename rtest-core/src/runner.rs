use crate::{create_scheduler, subproject, DistributionMode, WorkerPool};
use std::path::Path;

pub struct PytestRunner {
    pub program: String,
    pub initial_args: Vec<String>,
}

impl PytestRunner {
    pub fn new(env_vars: Vec<String>) -> Self {
        let program = "python3".into();
        let initial_args = vec!["-m".into(), "pytest".into()];

        // Apply environment variables (though this is typically done before command execution)
        // For now, we'll just acknowledge them, but a real implementation would set them
        // on the Command object before spawning.
        for env_var in env_vars {
            println!("Note: Environment variable '{env_var}' would be set for pytest.");
        }

        println!("Pytest command: {} {}", program, initial_args.join(" "));

        PytestRunner {
            program,
            initial_args,
        }
    }
}

/// Execute tests in parallel across multiple workers
pub fn execute_tests_parallel(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    worker_count: usize,
    dist_mode: &str,
    rootpath: &Path,
    use_subprojects: bool,
) -> i32 {
    println!("Running tests with {worker_count} workers using {dist_mode} distribution");

    let distribution_mode = match dist_mode.parse::<DistributionMode>() {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("Invalid distribution mode '{dist_mode}': {e}");
            return 1;
        }
    };

    if use_subprojects {
        let test_groups = subproject::group_tests_by_subproject(rootpath, &test_nodes);

        let mut worker_pool = WorkerPool::new();
        let mut worker_id = 0;

        for (subproject_root, tests) in test_groups {
            let adjusted_tests = if subproject_root != rootpath {
                subproject::make_test_paths_relative(&tests, rootpath, &subproject_root)
            } else {
                tests
            };

            let scheduler = create_scheduler(distribution_mode.clone());
            let test_batches = scheduler.distribute_tests(adjusted_tests, worker_count);

            for batch in test_batches {
                if !batch.is_empty() {
                    worker_pool.spawn_worker(
                        worker_id,
                        program.to_string(),
                        initial_args.to_vec(),
                        batch,
                        vec![],
                        Some(subproject_root.clone()),
                    );
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
        let test_batches = scheduler.distribute_tests(test_nodes, worker_count);

        if test_batches.is_empty() {
            println!("No test batches to execute.");
            return 0;
        }

        let mut worker_pool = WorkerPool::new();

        for (worker_id, tests) in test_batches.into_iter().enumerate() {
            if !tests.is_empty() {
                worker_pool.spawn_worker(
                    worker_id,
                    program.to_string(),
                    initial_args.to_vec(),
                    tests,
                    vec![],
                    Some(rootpath.to_path_buf()),
                );
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
