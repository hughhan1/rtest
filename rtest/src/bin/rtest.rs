//! Main entry point for the rtest application.

use clap::Parser;
use rtest::{
    cli::Args, collect_tests_rust_with_options, determine_worker_count, display_collection_results,
    execute_tests, execute_tests_parallel, subproject, CollectionOptions, PytestRunner,
};
use std::env;
use std::fs;
use std::io::Write;

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

    let runner = PytestRunner::new(args.env.clone());

    let rootpath = match env::current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Failed to get current directory: {e}");
            std::process::exit(1);
        }
    };

    // Set collection options based on CLI flags
    let collection_options = CollectionOptions {
        all_parametrized_uncertain: args.all_parametrized_uncertain,
    };

    let (test_nodes, errors) =
        match collect_tests_rust_with_options(rootpath.clone(), &args.files, &collection_options) {
            Ok((nodes, errors)) => (nodes, errors),
            Err(e) => {
                eprintln!("FATAL: {e}");
                std::process::exit(1);
            }
        };

    display_collection_results(&test_nodes, &errors);

    // Emit uncertain files if requested
    if let Some(ref output_path) = args.emit_uncertain_files {
        if let Err(e) = write_uncertain_files(&errors.uncertain_files, output_path) {
            eprintln!("Failed to write uncertain files to {}: {}", output_path, e);
            std::process::exit(1);
        }
        eprintln!(
            "Wrote {} uncertain files to {}",
            errors.uncertain_files.len(),
            output_path
        );
    }

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
                &runner.env_vars,
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
            &runner.env_vars,
        );
        std::process::exit(exit_code);
    }
}

/// Write uncertain files list to the specified path
fn write_uncertain_files(files: &[String], output_path: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(output_path)?;
    for filepath in files {
        writeln!(file, "{}", filepath)?;
    }
    Ok(())
}
