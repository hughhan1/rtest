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
        let parsed_env_vars = parse_env_vars(&env_vars, |env_str| {
            log::warn!("Invalid environment variable format: {}", env_str);
        });

        log::debug!("Pytest command: {} {}", program, initial_args.join(" "));

        PytestRunner {
            program,
            initial_args,
            env_vars: parsed_env_vars,
        }
    }
}

pub(crate) fn parse_env_vars(env_vars: &[String], warn: impl Fn(&str)) -> Vec<(String, String)> {
    env_vars
        .iter()
        .filter_map(|env_str| match env_str.split_once('=') {
            Some((key, value)) => Some((key.into(), value.into())),
            None => {
                warn(env_str);
                None
            }
        })
        .collect()
}

/// Execute tests in parallel across multiple workers
pub fn execute_tests_parallel(config: &ParallelExecutionConfig, test_nodes: Vec<String>) -> i32 {
    log::info!(
        "Running tests with {} workers using {} distribution",
        config.worker_count,
        config.dist_mode
    );

    let distribution_mode = match config.dist_mode.parse::<DistributionMode>() {
        Ok(mode) => mode,
        Err(e) => {
            log::warn!("Invalid distribution mode '{}': {e}", config.dist_mode);
            return 1;
        }
    };

    let test_groups = if config.use_subprojects {
        subproject::group_tests_by_subproject(config.rootpath, &test_nodes)
    } else {
        [(config.rootpath.to_path_buf(), test_nodes)].into()
    };
    let scheduler = create_scheduler(distribution_mode);
    let mut worker_pool = WorkerPool::new();
    let mut worker_id = 0;

    for (subproject_root, tests) in test_groups {
        let adjusted_tests = if subproject_root != config.rootpath {
            subproject::make_test_paths_relative(&tests, config.rootpath, &subproject_root)
        } else {
            tests
        };

        // Schedulers return only nonempty batches.
        for tests in scheduler.distribute_tests(adjusted_tests, config.worker_count) {
            worker_pool.spawn_worker(WorkerTask {
                worker_id,
                program: config.program.to_string(),
                initial_args: config.initial_args.to_vec(),
                tests,
                pytest_args: vec![],
                working_dir: Some(subproject_root.clone()),
                env_vars: config.env_vars.to_vec(),
            });
            worker_id += 1;
        }
    }

    if worker_id == 0 {
        log::info!("No test batches to execute.");
        return 0;
    }

    let mut overall_exit_code = 0;
    for result in worker_pool.wait_for_all() {
        println!("=== Worker {} ===", result.worker_id);
        print!("{}", result.stdout);
        eprint!("{}", result.stderr);
        if result.exit_code != 0 {
            overall_exit_code = result.exit_code;
        }
    }

    overall_exit_code
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_execute_tests_nonexistent_program() {
        let exit_code = crate::pytest_executor::execute_tests(
            "nonexistent_program_xyz_12345",
            &[],
            vec!["test::something".into()],
            vec![],
            None,
            &[],
        );
        assert_eq!(exit_code, 1);
    }
}
