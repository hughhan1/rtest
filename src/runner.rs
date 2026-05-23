use crate::cache::{parse_junit_xml, TestOutcome};
use crate::worker::WorkerResult;
use crate::{create_scheduler, subproject, DistributionMode, WorkerPool, WorkerTask};
use std::path::{Path, PathBuf};
use tempfile::TempDir;

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
    /// Extra pytest CLI args appended after test nodeids.
    pub worker_pytest_args: &'a [String],
}

fn parse_env_vars(env_vars: &[String], warn_invalid: bool) -> Vec<(String, String)> {
    env_vars
        .iter()
        .filter_map(|env_str| {
            if let Some((key, value)) = env_str.split_once('=') {
                Some((key.to_string(), value.to_string()))
            } else {
                if warn_invalid {
                    log::warn!("Invalid environment variable format: {}", env_str);
                }
                None
            }
        })
        .collect()
}

impl PytestRunner {
    pub fn new(env_vars: Vec<String>) -> Self {
        let program = "python3".into();
        let initial_args = vec!["-m".into(), "pytest".into()];
        let parsed_env_vars = parse_env_vars(&env_vars, true);

        log::debug!("Pytest command: {} {}", program, initial_args.join(" "));

        PytestRunner {
            program,
            initial_args,
            env_vars: parsed_env_vars,
        }
    }

    #[cfg(feature = "extension-module")]
    pub fn from_current_python(py: pyo3::Python<'_>) -> Self {
        use pyo3::types::PyAnyMethods;

        let python_path = py
            .import("sys")
            .and_then(|sys| sys.getattr("executable"))
            .and_then(|exe| exe.extract::<String>())
            .unwrap_or_else(|_| "python3".to_string());

        Self {
            program: python_path,
            initial_args: vec!["-m".to_string(), "pytest".to_string()],
            env_vars: vec![],
        }
    }

    #[cfg(feature = "extension-module")]
    pub fn from_current_python_with_env(py: pyo3::Python<'_>, env_vars: Vec<String>) -> Self {
        let mut runner = Self::from_current_python(py);
        runner.env_vars = parse_env_vars(&env_vars, false);
        runner
    }
}

/// Execute tests in parallel across multiple workers.
pub fn execute_tests_parallel(config: &ParallelExecutionConfig, test_nodes: Vec<String>) -> i32 {
    execute_tests_parallel_with_outcomes(config, test_nodes).0
}

/// Run tests in parallel and return exit code plus structured outcomes from `--junitxml` reports.
pub fn execute_tests_parallel_with_outcomes(
    config: &ParallelExecutionConfig,
    test_nodes: Vec<String>,
) -> (i32, Vec<(String, TestOutcome)>) {
    log::info!(
        "Running tests with {} workers using {} distribution",
        config.worker_count,
        config.dist_mode
    );

    let distribution_mode = match config.dist_mode.parse::<DistributionMode>() {
        Ok(mode) => mode,
        Err(e) => {
            log::warn!("Invalid distribution mode '{}': {e}", config.dist_mode);
            return (1, Vec::new());
        }
    };

    let junit_dir = match TempDir::with_prefix("rtest-junit-") {
        Ok(dir) => dir,
        Err(e) => {
            log::error!("Failed to create junit temp directory: {e}");
            return (1, Vec::new());
        }
    };

    let mut junit_paths: Vec<PathBuf> = Vec::new();
    let mut worker_pool = WorkerPool::new();
    let mut worker_id = 0;

    let spawn_batch = |pool: &mut WorkerPool,
                       id: usize,
                       batch: Vec<String>,
                       working_dir: PathBuf,
                       junit_path: PathBuf| {
        let mut pytest_args = config.worker_pytest_args.to_vec();
        pytest_args.push(format!("--junitxml={}", junit_path.display()));
        pool.spawn_worker(WorkerTask {
            worker_id: id,
            program: config.program.to_string(),
            initial_args: config.initial_args.to_vec(),
            tests: batch,
            pytest_args,
            working_dir: Some(working_dir),
            env_vars: config.env_vars.to_vec(),
        });
    };

    if config.use_subprojects {
        let test_groups = subproject::group_tests_by_subproject(config.rootpath, &test_nodes);

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
                    let junit_path = junit_dir.path().join(format!("worker-{worker_id}.xml"));
                    junit_paths.push(junit_path.clone());
                    spawn_batch(
                        &mut worker_pool,
                        worker_id,
                        batch,
                        subproject_root.clone(),
                        junit_path,
                    );
                    worker_id += 1;
                }
            }
        }
    } else {
        let scheduler = create_scheduler(distribution_mode);
        let test_batches = scheduler.distribute_tests(test_nodes, config.worker_count);

        for batch in test_batches {
            if !batch.is_empty() {
                let junit_path = junit_dir.path().join(format!("worker-{worker_id}.xml"));
                junit_paths.push(junit_path.clone());
                spawn_batch(
                    &mut worker_pool,
                    worker_id,
                    batch,
                    config.rootpath.to_path_buf(),
                    junit_path,
                );
                worker_id += 1;
            }
        }
    }

    if worker_id == 0 {
        log::info!("No test batches to execute.");
        return (0, Vec::new());
    }

    let results = worker_pool.wait_for_all();
    let (exit_code, _) = summarize_worker_results(results);

    let mut outcomes = Vec::new();
    for path in junit_paths {
        match parse_junit_xml(&path) {
            Ok(mut batch) => outcomes.append(&mut batch),
            Err(e) => log::warn!("Failed to parse junit xml {}: {e}", path.display()),
        }
    }

    (exit_code, outcomes)
}

fn summarize_worker_results(results: Vec<WorkerResult>) -> (i32, String) {
    let mut overall_exit_code = 0;
    let mut combined_output = String::new();
    for result in results {
        println!("=== Worker {} ===", result.worker_id);
        if !result.stdout.is_empty() {
            print!("{}", result.stdout);
            combined_output.push_str(&result.stdout);
        }
        if !result.stderr.is_empty() {
            eprint!("{}", result.stderr);
            combined_output.push_str(&result.stderr);
        }

        if result.exit_code != 0 {
            overall_exit_code = result.exit_code;
        }
    }

    (overall_exit_code, combined_output)
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

        assert_eq!(runner.program, "python3");
        assert_eq!(runner.initial_args, vec!["-m", "pytest"]);
    }

    #[test]
    fn test_env_var_parsing_skips_invalid() {
        let env_vars = vec![
            "VALID=value".into(),
            "INVALID_NO_EQUALS".into(),
            "ALSO_VALID=another".into(),
        ];
        let runner = PytestRunner::new(env_vars);
        assert_eq!(runner.env_vars.len(), 2);
        assert_eq!(
            runner.env_vars[0],
            ("VALID".to_string(), "value".to_string())
        );
        assert_eq!(
            runner.env_vars[1],
            ("ALSO_VALID".to_string(), "another".to_string())
        );
    }

    #[test]
    fn test_execute_tests_nonexistent_program() {
        let result = crate::pytest_executor::execute_tests(
            "nonexistent_program_xyz_12345",
            &[],
            vec!["test::something".into()],
            vec![],
            None,
            &[],
            None,
        );
        assert_eq!(result.exit_code, 1);
    }
}
