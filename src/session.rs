//! CLI session orchestration: collect → select → display → run.
//!
//! Selection (`-k`, `--lf`, `--ff`) is applied in the coordinator after collection.
//! Runners receive final nodeids only; they must not re-apply keyword or cache filters.

use crate::cache::{
    cache_dir_for_root, clear_cache, update_lastfailed_from_outcomes, LastFailedNoFailures,
};
use crate::cli::{exit_codes, Args, Runner};
use crate::collection::error::CollectionError;
use crate::collection_integration::{
    collect_tests_rust, display_collection_results, CollectedItem, CollectionDisplayStats,
    CollectionErrors,
};
use crate::config::{read_pytest_config, PytestConfig};
use crate::native_runner::{
    default_python_classes, default_python_files, default_python_functions, execute_native,
    files_from_nodeids, NativeRunnerConfig,
};
use crate::pytest_executor::execute_tests;
use crate::runner::{execute_tests_parallel_with_outcomes, ParallelExecutionConfig, PytestRunner};
use crate::selection::{apply_selection, SelectionError};
use crate::subproject;
use std::path::PathBuf;
use tempfile::TempDir;

pub struct SessionContext {
    pub args: Args,
    pub rootpath: PathBuf,
    pub python_executable: String,
    pub pytest_runner: PytestRunner,
    pub worker_count: usize,
    pub pytest_config: PytestConfig,
    pub cache_dir: PathBuf,
    pub lfnf: LastFailedNoFailures,
}

impl SessionContext {
    pub fn from_parts(
        args: Args,
        rootpath: PathBuf,
        python_executable: String,
        pytest_runner: PytestRunner,
        worker_count: usize,
    ) -> Result<Self, String> {
        let pytest_config = read_pytest_config(&rootpath);
        let cache_dir = cache_dir_for_root(&rootpath, pytest_config.cache_dir.as_ref());
        let lfnf = args.lfnf.parse()?;
        Ok(Self {
            args,
            rootpath,
            python_executable,
            pytest_runner,
            worker_count,
            pytest_config,
            cache_dir,
            lfnf,
        })
    }
}

pub fn handle_collection_error(e: CollectionError) -> ! {
    match e {
        CollectionError::FileNotFound(path) => {
            eprintln!("ERROR: file or directory not found: {}", path.display());
            std::process::exit(exit_codes::USAGE_ERROR);
        }
        e => {
            eprintln!("FATAL: {e}");
            std::process::exit(exit_codes::TESTS_FAILED);
        }
    }
}

fn apply_cache_clear(ctx: &SessionContext) {
    if ctx.args.cache_clear {
        if let Err(e) = clear_cache(&ctx.cache_dir) {
            eprintln!("Warning: failed to clear cache: {e}");
        }
    }
}

fn python_files(config: &PytestConfig) -> Vec<String> {
    if config.python_files.is_empty() {
        default_python_files()
    } else {
        config.python_files.clone()
    }
}

fn python_classes(config: &PytestConfig) -> Vec<String> {
    if config.python_classes.is_empty() {
        default_python_classes()
    } else {
        config.python_classes.clone()
    }
}

fn python_functions(config: &PytestConfig) -> Vec<String> {
    if config.python_functions.is_empty() {
        default_python_functions()
    } else {
        config.python_functions.clone()
    }
}

fn collect_and_select(
    ctx: &SessionContext,
    file_args: &[String],
) -> Result<(Vec<CollectedItem>, CollectionErrors, usize), SelectionError> {
    let (items, errors) = match collect_tests_rust(ctx.rootpath.clone(), file_args) {
        Ok(result) => result,
        Err(e) => handle_collection_error(e),
    };

    let (selected, stats) = apply_selection(
        items,
        ctx.args.keyword_expr.as_deref(),
        ctx.args.last_failed,
        ctx.args.failed_first,
        ctx.lfnf,
        &ctx.cache_dir,
    )?;

    Ok((selected, errors, stats.deselected))
}

pub fn run_session(ctx: SessionContext) -> i32 {
    apply_cache_clear(&ctx);

    let file_args = ctx.args.files.clone();
    let collect_result = collect_and_select(&ctx, &file_args);

    let (items, errors, deselected) = match collect_result {
        Ok(result) => result,
        Err(SelectionError::KeywordParse(e)) => {
            eprintln!("ERROR: Invalid -k expression {e}");
            return exit_codes::USAGE_ERROR;
        }
        Err(SelectionError::Cache(e)) => {
            eprintln!("ERROR: {e}");
            return exit_codes::TESTS_FAILED;
        }
    };

    display_collection_results(&items, &errors, CollectionDisplayStats { deselected });

    if !errors.errors.is_empty() {
        return exit_codes::TESTS_FAILED;
    }

    if items.is_empty() {
        if ctx.args.last_failed && ctx.lfnf == LastFailedNoFailures::None {
            return exit_codes::OK;
        }
        println!("No tests found.");
        return exit_codes::OK;
    }

    if ctx.args.collect_only {
        return exit_codes::OK;
    }

    match ctx.args.runner {
        Runner::Native => run_native(&ctx, items),
        Runner::Pytest => run_pytest(&ctx, items),
    }
}

fn run_native(ctx: &SessionContext, items: Vec<CollectedItem>) -> i32 {
    let test_files = files_from_nodeids(&ctx.rootpath, &items);
    let selected_nodeids = CollectedItem::nodeids(&items);

    let config = NativeRunnerConfig {
        python_executable: ctx.python_executable.clone(),
        root_path: ctx.rootpath.clone(),
        num_workers: ctx.worker_count,
        python_files: python_files(&ctx.pytest_config),
        python_classes: python_classes(&ctx.pytest_config),
        python_functions: python_functions(&ctx.pytest_config),
        selected_nodeids,
    };

    let result = execute_native(&config, test_files);
    if let Err(e) = update_lastfailed_from_outcomes(&ctx.cache_dir, &result.outcomes) {
        eprintln!("Warning: failed to update failure cache: {e}");
    }
    result.exit_code
}

fn run_pytest(ctx: &SessionContext, items: Vec<CollectedItem>) -> i32 {
    let test_nodes = CollectedItem::nodeids(&items);

    if ctx.worker_count == 1 || ctx.args.dist == "no" {
        return run_pytest_serial(ctx, test_nodes);
    }

    let config = ParallelExecutionConfig {
        program: &ctx.pytest_runner.program,
        initial_args: &ctx.pytest_runner.initial_args,
        worker_count: ctx.worker_count,
        dist_mode: &ctx.args.dist,
        rootpath: &ctx.rootpath,
        use_subprojects: true,
        env_vars: &ctx.pytest_runner.env_vars,
        worker_pytest_args: &[],
    };
    let (exit_code, outcomes) = execute_tests_parallel_with_outcomes(&config, test_nodes);
    if let Err(e) = update_lastfailed_from_outcomes(&ctx.cache_dir, &outcomes) {
        eprintln!("Warning: failed to update failure cache: {e}");
    }
    exit_code
}

fn run_pytest_serial(ctx: &SessionContext, test_nodes: Vec<String>) -> i32 {
    let junit_dir = match TempDir::with_prefix("rtest-junit-serial-") {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Failed to create junit temp directory: {e}");
            return exit_codes::TESTS_FAILED;
        }
    };

    let test_groups = subproject::group_tests_by_subproject(&ctx.rootpath, &test_nodes);
    let mut overall_exit_code = 0;
    let mut all_outcomes = Vec::new();
    let mut batch_index = 0usize;

    for (subproject_root, tests) in test_groups {
        if tests.is_empty() {
            continue;
        }

        let adjusted_tests = if subproject_root != ctx.rootpath {
            subproject::make_test_paths_relative(&tests, &ctx.rootpath, &subproject_root)
        } else {
            tests
        };

        let junit_path = junit_dir.path().join(format!("batch-{batch_index}.xml"));
        batch_index += 1;

        let result = execute_tests(
            &ctx.pytest_runner.program,
            &ctx.pytest_runner.initial_args,
            adjusted_tests,
            vec![],
            Some(&subproject_root),
            &ctx.pytest_runner.env_vars,
            Some(&junit_path),
        );

        all_outcomes.extend(result.outcomes);

        if result.exit_code != 0 {
            overall_exit_code = result.exit_code;
        }
    }

    if let Err(e) = update_lastfailed_from_outcomes(&ctx.cache_dir, &all_outcomes) {
        eprintln!("Warning: failed to update failure cache: {e}");
    }

    overall_exit_code
}
