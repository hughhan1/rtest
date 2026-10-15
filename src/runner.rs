use crate::{subproject, DistributionMode, WorkerPool, WorkerTask};
use std::num::NonZeroUsize;
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
    pub worker_count: NonZeroUsize,
    pub dist_mode: DistributionMode,
    pub rootpath: &'a Path,
    pub env_vars: &'a [(String, String)],
}

impl PytestRunner {
    pub fn new(program: String, env_vars: &[String]) -> Self {
        let initial_args = vec!["-m".into(), "pytest".into()];

        // Parse environment variables from KEY=VALUE format
        let parsed_env_vars = parse_env_vars(env_vars, |env_str| {
            eprintln!("Warning: Invalid environment variable format: {}", env_str);
        });

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

    let test_groups = subproject::group_tests_by_subproject(config.rootpath, test_nodes);
    let mut worker_pool = WorkerPool::new();
    let mut worker_id = 0;

    for (subproject_root, tests) in test_groups {
        for tests in config
            .dist_mode
            .distribute_tests(tests, config.worker_count)
        {
            worker_pool.spawn_worker(WorkerTask {
                worker_id,
                program: config.program.to_string(),
                initial_args: config.initial_args.to_vec(),
                tests,
                working_dir: subproject_root.clone(),
                env_vars: config.env_vars.to_vec(),
            });
            worker_id += 1;
        }
    }

    let mut overall_exit_code = 0;
    for result in worker_pool.wait_for_all() {
        println!("=== Worker {} ===", result.worker_id);
        let code = match result.result {
            Ok(output) => {
                print!("{}", String::from_utf8_lossy(&output.stdout));
                eprint!("{}", String::from_utf8_lossy(&output.stderr));
                output.status.code().unwrap_or_else(|| {
                    eprintln!("Worker terminated: {}", output.status);
                    1
                })
            }
            Err(error) => {
                eprintln!("{error}");
                1
            }
        };
        if code != 0 {
            overall_exit_code = code;
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
