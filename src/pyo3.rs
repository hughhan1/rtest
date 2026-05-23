//! Python bindings for rtest library.

use clap::{error::ErrorKind, Parser};
use pyo3::prelude::*;
use std::env;
use std::path::PathBuf;

use crate::cli::{exit_codes, Args};
use crate::runner::PytestRunner;
use crate::session::{run_session, SessionContext};
use crate::utils::determine_worker_count;

fn get_current_dir() -> Result<PathBuf, String> {
    env::current_dir().map_err(|e| format!("Failed to get current directory: {e}"))
}

fn get_python_executable(py: Python) -> String {
    py.import("sys")
        .and_then(|sys| sys.getattr("executable"))
        .and_then(|exe| exe.extract::<String>())
        .unwrap_or_else(|_| "python3".to_string())
}

fn parse_cli_args(argv: Vec<String>) -> Result<Args, i32> {
    let mut full_args = vec!["rtest".to_string()];
    full_args.extend(argv);

    match Args::try_parse_from(full_args) {
        Ok(args) => Ok(args),
        Err(err) => {
            let _ = err.print();
            let code = match err.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => exit_codes::OK,
                _ => exit_codes::USAGE_ERROR,
            };
            Err(code)
        }
    }
}

fn build_session(py: Python, argv: Vec<String>) -> Result<SessionContext, i32> {
    let args = parse_cli_args(argv)?;

    if let Err(e) = args.validate_dist() {
        eprintln!("Error: {e}");
        return Err(exit_codes::TESTS_FAILED);
    }

    let num_processes = match args.get_num_processes() {
        Ok(n) => n,
        Err(e) => {
            eprintln!("Error: {e}");
            return Err(exit_codes::TESTS_FAILED);
        }
    };
    let worker_count = determine_worker_count(num_processes, args.maxprocesses);

    let rootpath = match get_current_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("{e}");
            return Err(exit_codes::TESTS_FAILED);
        }
    };

    let python_executable = get_python_executable(py);
    let pytest_runner = PytestRunner::from_current_python_with_env(py, args.env.clone());

    SessionContext::from_parts(
        args,
        rootpath,
        python_executable,
        pytest_runner,
        worker_count,
    )
    .map_err(|e| {
        eprintln!("Error: {e}");
        exit_codes::TESTS_FAILED
    })
}

#[pyfunction]
#[pyo3(signature = (pytest_args=None))]
fn run_tests(py: Python, pytest_args: Option<Vec<String>>) -> i32 {
    let argv = pytest_args.unwrap_or_default();
    let ctx = match build_session(py, argv) {
        Ok(ctx) => ctx,
        Err(code) => return code,
    };
    run_session(ctx)
}

#[pyfunction]
fn main_cli_with_args(py: Python, argv: Vec<String>) {
    let exit_code = match build_session(py, argv) {
        Ok(ctx) => run_session(ctx),
        Err(code) => code,
    };
    std::process::exit(exit_code);
}

#[pymodule]
pub fn _rtest(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_tests, m)?)?;
    m.add_function(wrap_pyfunction!(main_cli_with_args, m)?)?;
    Ok(())
}
