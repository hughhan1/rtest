//! Main entry point for the rtest application.

use clap::Parser;
use rtest_core::{
    cli::Args, collect_tests_rust, create_scheduler, determine_worker_count,
    display_collection_results, execute_tests, execute_work_stealing, subproject, DistributionMode,
    PytestRunner, WorkerPool,
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
        // Group tests by subproject
        let test_groups = subproject::group_tests_by_subproject(&rootpath, &test_nodes);

        let mut overall_exit_code = 0;

        for (subproject_root, tests) in test_groups {
            if tests.is_empty() {
                continue;
            }

            let adjusted_tests = if subproject_root != rootpath {
                subproject::make_test_paths_relative(&tests, &rootpath, &subproject_root)
            } else {
                tests
            };

            let exit_code = execute_tests(
                &runner.program,
                &runner.initial_args,
                adjusted_tests,
                vec![],
                Some(&subproject_root),
            );

            if exit_code != 0 {
                overall_exit_code = exit_code;
            }
        }

        std::process::exit(overall_exit_code);
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

    match distribution_mode {
        DistributionMode::WorkSteal => {
            execute_with_work_stealing(program, initial_args, test_nodes, worker_count, rootpath);
        }
        _ => {
            execute_with_traditional_distribution(
                program,
                initial_args,
                test_nodes,
                worker_count,
                distribution_mode,
                rootpath,
            );
        }
    }
}

fn execute_with_traditional_distribution(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    worker_count: usize,
    distribution_mode: DistributionMode,
    rootpath: &std::path::Path,
) {
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
        std::process::exit(0);
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

fn execute_with_work_stealing(
    program: &str,
    initial_args: &[String],
    test_nodes: Vec<String>,
    worker_count: usize,
    rootpath: &std::path::Path,
) {
    println!("Using dynamic work-stealing execution");

    // For work-stealing, we also need to handle subprojects
    let test_groups = subproject::group_tests_by_subproject(rootpath, &test_nodes);

    // If we have multiple subprojects, we need to handle them separately
    if test_groups.len() > 1 {
        println!("Note: Work-stealing with multiple subprojects will execute each subproject sequentially");

        let mut overall_exit_code = 0;

        for (subproject_root, tests) in test_groups {
            if tests.is_empty() {
                continue;
            }

            let adjusted_tests = if subproject_root != rootpath {
                subproject::make_test_paths_relative(&tests, rootpath, &subproject_root)
            } else {
                tests
            };

            println!(
                "\nExecuting tests in subproject: {}",
                subproject_root.display()
            );

            let result = execute_work_stealing(
                program,
                initial_args,
                adjusted_tests,
                worker_count,
                &subproject_root,
            );

            result.print_summary();

            if result.exit_code() != 0 {
                overall_exit_code = result.exit_code();
            }
        }

        std::process::exit(overall_exit_code);
    } else {
        // Single project or subproject - use regular work-stealing
        let result =
            execute_work_stealing(program, initial_args, test_nodes, worker_count, rootpath);

        result.print_summary();
        std::process::exit(result.exit_code());
    }
}
