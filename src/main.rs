//! Main entry point for the rtest application.

use clap::Parser;
use rtest::{
    cli::Args, collect_tests_rust, determine_worker_count, display_collection_results,
    execute_tests, execute_tests_parallel, subproject, PytestRunner,
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
        let exit_code = execute_tests_parallel(
            &runner.program,
            &runner.initial_args,
            test_nodes,
            worker_count,
            &args.dist,
            &rootpath,
            true, // CLI uses subprojects
        );
        std::process::exit(exit_code);
    }
}
