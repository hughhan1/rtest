//! Main entry point for the rustic application.

mod cli;
mod collection;
mod collection_integration;
mod pytest_executor;
mod python_discovery;
mod runner;

use clap::Parser;
use cli::Args;
use collection_integration::{collect_tests_rust, display_collection_results};
use pytest_executor::execute_tests;
use runner::PytestRunner;
use std::env;

fn main() {
    let args = Args::parse();

    let runner = PytestRunner::new(args.package_manager, args.env);

    // Use Rust-based collection
    let rootpath = env::current_dir().expect("Failed to get current directory");
    let test_nodes = collect_tests_rust(rootpath, &args.pytest_args);

    // Display collection results for debugging
    display_collection_results(&test_nodes);

    if test_nodes.is_empty() {
        println!("No tests found.");
        std::process::exit(0);
    }

    execute_tests(
        &runner.program,
        &runner.initial_args,
        test_nodes,
        args.pytest_args,
    );
}
