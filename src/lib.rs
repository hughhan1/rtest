//! Python bindings for rtest library.

use clap::Parser;
use pyo3::prelude::*;
use rtest::{
    cli::Args, collect_tests_rust, determine_worker_count, display_collection_results,
    execute_tests, execute_tests_parallel,
};
use std::env;

pub struct PytestRunner {
    pub program: String,
    pub initial_args: Vec<String>,
}

impl PytestRunner {
    pub fn from_current_python(py: Python) -> Self {
        let python_path = py
            .import("sys")
            .and_then(|sys| sys.getattr("executable"))
            .and_then(|exe| exe.extract::<String>())
            .unwrap_or_else(|_| "python3".to_string());

        let initial_args = vec!["-m".to_string(), "pytest".to_string()];

        Self {
            program: python_path,
            initial_args,
        }
    }
}

#[pyfunction]
#[pyo3(signature = (pytest_args=None))]
fn run_tests(py: Python, pytest_args: Option<Vec<String>>) -> i32 {
    let pytest_args = pytest_args.unwrap_or_default();

    // Use the current Python executable
    let runner = PytestRunner::from_current_python(py);

    // Determine root path: if the first argument is a path and not a pytest flag, use it
    let (rootpath, filtered_args) = if let Some(first_arg) = pytest_args.first() {
        if !first_arg.starts_with('-') && std::path::Path::new(first_arg).exists() {
            // First argument is a path, use it as root and remove it from pytest args
            let path = std::path::PathBuf::from(first_arg);
            let remaining_args = pytest_args.into_iter().skip(1).collect();
            (path, remaining_args)
        } else {
            // First argument is not a path, use current directory
            (
                match env::current_dir() {
                    Ok(dir) => dir,
                    Err(e) => {
                        eprintln!("Failed to get current directory: {e}");
                        return 1;
                    }
                },
                pytest_args,
            )
        }
    } else {
        (
            match env::current_dir() {
                Ok(dir) => dir,
                Err(e) => {
                    eprintln!("Failed to get current directory: {e}");
                    return 1;
                }
            },
            pytest_args,
        )
    };

    let collection_result = collect_tests_rust(rootpath.clone(), &filtered_args);

    let (test_nodes, errors) = match collection_result {
        Ok((nodes, errs)) => (nodes, errs),
        Err(e) => {
            eprintln!("Collection failed: {e}");
            return 1;
        }
    };

    display_collection_results(&test_nodes, &errors);

    if test_nodes.is_empty() {
        println!("No tests found.");
        return 0;
    }

    execute_tests(
        &runner.program,
        &runner.initial_args,
        test_nodes,
        filtered_args,
        Some(&rootpath),
    )
}

#[pyfunction]
fn main_cli_with_args(py: Python, argv: Vec<String>) {
    // Prepend program name for clap parsing
    let mut full_args = vec!["rtest".to_string()];
    full_args.extend(argv);
    let args = Args::parse_from(full_args);

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

    let runner = PytestRunner::from_current_python(py);

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

    let exit_code = if worker_count == 1 {
        execute_tests(
            &runner.program,
            &runner.initial_args,
            test_nodes,
            vec![],
            Some(&rootpath),
        )
    } else {
        execute_tests_parallel(
            &runner.program,
            &runner.initial_args,
            test_nodes,
            worker_count,
            &args.dist,
            &rootpath,
            false, // Python bindings don't use subprojects
        )
    };
    std::process::exit(exit_code);
}

#[pymodule]
fn _rtest(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_tests, m)?)?;
    m.add_function(wrap_pyfunction!(main_cli_with_args, m)?)?;
    Ok(())
}
