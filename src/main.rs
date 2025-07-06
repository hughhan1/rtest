//! Main entry point for the rtest application.

use clap::Parser;
use rtest_core::{
    cli::Args, collect_tests_rust, create_scheduler, determine_worker_count,
    display_collection_results, execute_tests, DistributionMode, PytestRunner, WorkerPool,
};
use std::env;

pub fn main() {
    let args = Args::parse();

    if let Err(e) = args.validate_dist() {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }

    let num_processes = match args.get_num_processes() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
    };
    let worker_count = determine_worker_count(num_processes, args.maxprocesses);

    let runner = PytestRunner::new(args.env);

    let rootpath = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Failed to get current directory: {e}");
            std::process::exit(1);
        }
    };
    let (test_nodes, errors) = match collect_tests_rust(rootpath.clone(), &args.files) {
        Ok((nodes, errors)) => (nodes, errors),
        Err(e) => {
            eprintln!("FATAL: {e}");
            std::process::exit(1);
        }
    };

    display_collection_results(&test_nodes, &errors);

    // Exit early if there are collection errors to prevent test execution
    if !errors.errors.is_empty() {
        std::process::exit(1);
    }

    if test_nodes.is_empty() {
        println!("No tests found.");
        std::process::exit(0);
    }

    // Exit after collection if --collect-only flag is set
    if args.collect_only {
        std::process::exit(0);
    }

    if worker_count == 1 || args.dist == "no" {
        execute_tests(
            &runner.program,
            &runner.initial_args,
            test_nodes,
            vec![],
            Some(&rootpath),
        );
    } else {
        execute_tests_parallel(
            &runner.program,
            &runner.initial_args,
            test_nodes,
            worker_count,
            &args.dist,
            &rootpath,
        );
    }
}

fn execute_tests_parallel(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    worker_count: usize,
    dist_mode: &str,
    rootpath: &std::path::Path,
) {
    println!("Running tests with {worker_count} workers using {dist_mode} distribution");

    let distribution_mode = match dist_mode.parse::<DistributionMode>() {
        Ok(mode) => mode,
        Err(e) => {
            eprintln!("Invalid distribution mode '{dist_mode}': {e}");
            std::process::exit(1);
        }
    };
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

    std::process::exit(overall_exit_code);
}
