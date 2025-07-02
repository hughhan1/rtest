//! Main entry point for the rustic application.

mod cli;
mod collection;
mod collection_integration;
mod pytest_executor;
mod python_discovery;
mod runner;
mod scheduler;
mod utils;
mod worker;

use clap::Parser;
use cli::Args;
use collection_integration::{collect_tests_rust, display_collection_results};
use pytest_executor::execute_tests;
use runner::PytestRunner;
use scheduler::{create_scheduler, DistributionMode};
use std::env;
use utils::determine_worker_count;
use worker::WorkerPool;

fn main() {
    let args = Args::parse();

    if let Err(e) = args.validate_dist() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    let worker_count = determine_worker_count(args.get_num_processes(), args.maxprocesses);

    let runner = PytestRunner::new(args.package_manager, args.env);

    let rootpath = env::current_dir().expect("Failed to get current directory");
    let test_nodes = match collect_tests_rust(rootpath, &args.pytest_args) {
        Ok(nodes) => nodes,
        Err(e) => {
            eprintln!("FATAL: {e}");
            std::process::exit(1);
        }
    };

    display_collection_results(&test_nodes);

    if test_nodes.is_empty() {
        println!("No tests found.");
        std::process::exit(0);
    }

    if worker_count == 1 {
        execute_tests(
            &runner.program,
            &runner.initial_args,
            test_nodes,
            args.pytest_args,
        );
    } else {
        execute_tests_parallel(
            &runner.program,
            &runner.initial_args,
            test_nodes,
            args.pytest_args,
            worker_count,
            &args.dist,
        );
    }
}

fn execute_tests_parallel(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    pytest_args: Vec<String>,
    worker_count: usize,
    dist_mode: &str,
) {
    println!("Running tests with {worker_count} workers using {dist_mode} distribution");

    let distribution_mode = dist_mode.parse::<DistributionMode>().unwrap();
    let scheduler = create_scheduler(distribution_mode);
    let test_batches = scheduler.distribute_tests(test_nodes, worker_count);

    if test_batches.is_empty() {
        println!("No test batches to execute.");
        std::process::exit(0);
    }

    let mut worker_pool = WorkerPool::new();

    for (worker_id, tests) in test_batches.into_iter().enumerate() {
        if !tests.is_empty() {
            worker_pool.spawn_worker(
                worker_id,
                program.to_string(),
                initial_args.to_vec(),
                tests,
                pytest_args.clone(),
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

    std::process::exit(overall_exit_code);
}
