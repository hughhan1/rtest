//! Rustic library for Python test collection and execution.

use pyo3::prelude::*;
use std::env;

pub mod cli;
pub mod collection;
pub mod collection_integration;
pub mod pytest_executor;
pub mod python_discovery;
pub mod runner;
pub mod scheduler;
pub mod utils;
pub mod worker;

use collection_integration::{collect_tests_rust, display_collection_results};
use pytest_executor::execute_tests;
use runner::PytestRunner;

#[pyfunction]
#[pyo3(signature = (pytest_args=None))]
fn run_tests(py: Python, pytest_args: Option<Vec<String>>) {
    let pytest_args = pytest_args.unwrap_or_default();

    // Use the current Python executable
    let runner = PytestRunner::from_current_python(py);

    let rootpath = env::current_dir().expect("Failed to get current directory");
    let collection_result = collect_tests_rust(rootpath, &pytest_args);

    let (test_nodes, errors) = match collection_result {
        Ok((nodes, errs)) => (nodes, errs),
        Err(e) => {
            eprintln!("Collection failed: {e}");
            return;
        }
    };

    display_collection_results(&test_nodes, &errors);

    if test_nodes.is_empty() {
        println!("No tests found.");
        return;
    }

    execute_tests(
        &runner.program,
        &runner.initial_args,
        test_nodes,
        pytest_args,
    );
}

#[pymodule]
fn _rustic(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_tests, m)?)?;
    Ok(())
}
