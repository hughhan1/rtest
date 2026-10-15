//! Native test runner that executes tests via Python workers.

use crate::collection::glob_match_any;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use tempfile::TempDir;
use walkdir::WalkDir;

#[derive(Debug, Deserialize)]
struct TestResult {
    nodeid: String,
    duration_ms: f64,
    #[serde(flatten)]
    outcome: TestOutcome,
}

/// Each outcome owns exactly the diagnostics that can accompany it.
#[derive(Debug, Deserialize)]
#[serde(tag = "outcome", rename_all = "lowercase")]
enum TestOutcome {
    Passed,
    Failed(FailureInfo),
    Error(FailureInfo),
    Skipped,
}

#[derive(Debug, Deserialize)]
struct FailureInfo {
    error: TracebackInfo,
    // Deserialized even though only Python consumers currently display the type.
    #[serde(rename = "error_type")]
    _error_type: String,
}

#[derive(Debug, Deserialize)]
struct TracebackInfo {
    traceback: String,
}

#[derive(Debug)]
struct FailedTest {
    nodeid: String,
    info: FailureInfo,
}

#[derive(Debug, Default)]
struct AggregatedResults {
    passed: usize,
    skipped: usize,
    total_duration_ms: f64,
    failures: Vec<FailedTest>,
    errors: Vec<FailedTest>,
    worker_errors: Vec<String>,
}

impl AggregatedResults {
    fn has_failures(&self) -> bool {
        !self.failures.is_empty() || !self.errors.is_empty() || !self.worker_errors.is_empty()
    }
}

pub struct NativeRunnerConfig {
    pub python_executable: String,
    pub root_path: PathBuf,
    pub num_workers: NonZeroUsize,
    pub python_files: Vec<String>,
    pub python_classes: Vec<String>,
    pub python_functions: Vec<String>,
}

/// Default pytest python_files patterns
pub fn default_python_files() -> Vec<String> {
    vec!["test_*.py".into(), "*_test.py".into()]
}

/// Default pytest python_classes patterns
pub fn default_python_classes() -> Vec<String> {
    vec!["Test*".into()]
}

/// Default pytest python_functions patterns
pub fn default_python_functions() -> Vec<String> {
    vec!["test*".into()]
}

fn parse_results_file(path: &Path) -> Result<Vec<TestResult>, String> {
    let file =
        File::open(path).map_err(|e| format!("Failed to open results file {:?}: {}", path, e))?;

    let reader = BufReader::new(file);
    let mut results = Vec::new();

    for (line_num, line) in reader.lines().enumerate() {
        let line = line.map_err(|e| format!("Failed to read line {}: {}", line_num + 1, e))?;

        if line.trim().is_empty() {
            continue;
        }

        let result: TestResult = serde_json::from_str(&line)
            .map_err(|e| format!("Failed to parse JSON on line {}: {}", line_num + 1, e))?;

        results.push(result);
    }

    Ok(results)
}

impl Extend<TestResult> for AggregatedResults {
    fn extend<T: IntoIterator<Item = TestResult>>(&mut self, results: T) {
        for result in results {
            self.total_duration_ms += result.duration_ms;
            match result.outcome {
                TestOutcome::Passed => self.passed += 1,
                TestOutcome::Failed(info) => self.failures.push(FailedTest {
                    nodeid: result.nodeid,
                    info,
                }),
                TestOutcome::Error(info) => self.errors.push(FailedTest {
                    nodeid: result.nodeid,
                    info,
                }),
                TestOutcome::Skipped => self.skipped += 1,
            }
        }
    }
}

fn print_test_results(label: &str, results: &[FailedTest]) {
    if results.is_empty() {
        return;
    }
    println!();
    println!("{}:", label);
    for result in results {
        println!("  {}", result.nodeid);
        for line in result.info.error.traceback.lines() {
            println!("    {}", line);
        }
    }
}

fn print_summary(results: &AggregatedResults) {
    println!();
    println!("================================ SUMMARY ================================");

    print_test_results("FAILURES", &results.failures);
    print_test_results("ERRORS", &results.errors);
    for error in &results.worker_errors {
        eprintln!("{error}");
    }

    println!();
    let duration_secs = results.total_duration_ms / 1000.0;

    let parts: Vec<String> = [
        (results.passed, "passed"),
        (results.failures.len(), "failed"),
        (results.errors.len(), "error"),
        (results.worker_errors.len(), "worker error"),
        (results.skipped, "skipped"),
    ]
    .into_iter()
    .filter(|(count, _)| *count > 0)
    .map(|(count, label)| format!("{count} {label}"))
    .collect();

    let status = if results.has_failures() {
        "FAILED"
    } else {
        "PASSED"
    };

    println!(
        "========== {} {} in {:.2}s ==========",
        status,
        parts.join(", "),
        duration_secs
    );
}

fn run_worker(
    worker_id: usize,
    python: &str,
    root: &Path,
    output_file: &Path,
    files: &[PathBuf],
    python_classes: &[String],
    python_functions: &[String],
) -> Result<Vec<TestResult>, String> {
    let mut cmd = Command::new(python);
    cmd.arg("-m")
        .arg("rtest.worker")
        .arg("--root")
        .arg(root)
        .arg("--out")
        .arg(output_file);

    for (flag, patterns) in [
        ("--python-classes", python_classes),
        ("--python-functions", python_functions),
    ] {
        if !patterns.is_empty() {
            cmd.arg(flag).args(patterns);
        }
    }

    // Use -- to separate options from positional file arguments
    cmd.arg("--").args(files);

    let output = cmd
        .current_dir(root)
        .output()
        .map_err(|e| format!("Worker {} failed to start: {}", worker_id, e))?;

    let exit_code = output.status.code();
    // Exit code 1 is expected when tests fail, only report unexpected codes.
    if !matches!(exit_code, Some(0 | 1)) {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.trim().is_empty() {
            eprintln!("Worker {} stderr:\n{}", worker_id, stderr);
        }
        return Err(format!(
            "Worker {} exited with {} (expected exit code 0 or 1)",
            worker_id, output.status
        ));
    }

    parse_results_file(output_file)
}

pub fn execute_native(config: &NativeRunnerConfig, test_files: Vec<PathBuf>) -> i32 {
    if test_files.is_empty() {
        println!("No test files to run.");
        return 0;
    }

    let num_workers = config.num_workers;
    println!(
        "Running {} test file(s) with {} worker(s)",
        test_files.len(),
        num_workers
    );

    let temp_dir = match TempDir::with_prefix("rtest-") {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Failed to create temp directory: {}", e);
            return 1;
        }
    };

    let shards = crate::scheduler::distribute_round_robin(test_files, num_workers);
    let mut handles = Vec::new();
    for (i, shard) in shards.into_iter().enumerate() {
        let output_file = temp_dir.path().join(format!("worker-{}.jsonl", i));
        let python = config.python_executable.clone();
        let root = config.root_path.clone();
        let python_classes = config.python_classes.clone();
        let python_functions = config.python_functions.clone();
        let handle = thread::spawn(move || {
            run_worker(
                i,
                &python,
                &root,
                &output_file,
                &shard,
                &python_classes,
                &python_functions,
            )
        });
        handles.push(handle);
    }

    let mut aggregated = AggregatedResults::default();
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(Ok(results)) => aggregated.extend(results),
            Ok(Err(e)) => aggregated
                .worker_errors
                .push(format!("Worker {}: {}", i, e)),
            Err(_) => aggregated
                .worker_errors
                .push(format!("Worker {} panicked", i)),
        }
    }

    print_summary(&aggregated);

    // TempDir is automatically cleaned up when dropped

    i32::from(aggregated.has_failures())
}

fn is_hidden_or_ignored(name: &str) -> bool {
    name.starts_with('.') || name == "__pycache__"
}

/// Check if a file matches any of the python_files patterns
fn is_test_file(path: &Path, patterns: &[String]) -> bool {
    let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

    if path.extension().is_none_or(|ext| ext != "py") {
        return false;
    }

    glob_match_any(patterns, file_name)
}

pub fn collect_test_files(root: &Path, paths: &[String], patterns: &[String]) -> Vec<PathBuf> {
    let search_paths = if paths.is_empty() {
        vec![root.to_path_buf()]
    } else {
        paths.iter().map(|path| root.join(path)).collect()
    };

    let mut test_files: Vec<PathBuf> = search_paths
        .into_iter()
        .flat_map(|path| {
            // Explicit files include hidden names and symlinks; directory walks do not.
            let explicit_file = !paths.is_empty() && path.is_file();
            WalkDir::new(path)
                .follow_links(explicit_file)
                .into_iter()
                .filter_entry(move |entry| {
                    explicit_file
                        || entry
                            .file_name()
                            .to_str()
                            .is_some_and(|name| !is_hidden_or_ignored(name))
                })
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file() && is_test_file(entry.path(), patterns))
                .map(|entry| entry.into_path())
        })
        .collect();
    test_files.sort();
    test_files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_results_json() {
        // Test deserializing JSON
        let json = r#"{"nodeid":"test::foo","outcome":"passed","duration_ms":1.5,"stdout":"","stderr":"","error":null}"#;
        let result: TestResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.nodeid, "test::foo");
        assert!(matches!(result.outcome, TestOutcome::Passed));
        assert_eq!(result.duration_ms, 1.5);
    }

    #[test]
    fn rejects_invalid_worker_json_at_the_protocol_boundary() {
        for json in [
            r#"{"nodeid":"test::foo","outcome":"unknown","duration_ms":0}"#,
            r#"{"nodeid":"test::foo","outcome":"failed","duration_ms":0}"#,
            r#"{"nodeid":"test::foo","outcome":"error","duration_ms":0,"error":{"reason":"skip"},"error_type":"ValueError"}"#,
        ] {
            assert!(serde_json::from_str::<TestResult>(json).is_err());
        }
    }

    #[test]
    fn aggregates_worker_outcomes_with_their_diagnostics() {
        let results = [
            r#"{"nodeid":"passed","outcome":"passed","duration_ms":1}"#,
            r#"{"nodeid":"skipped","outcome":"skipped","duration_ms":0,"error":{"reason":"later"}}"#,
            r#"{"nodeid":"failed","outcome":"failed","duration_ms":2,"error":{"traceback":"assertion"},"error_type":"AssertionError"}"#,
            r#"{"nodeid":"error","outcome":"error","duration_ms":3,"error":{"traceback":"exception"},"error_type":"ValueError"}"#,
        ].into_iter().map(|json| serde_json::from_str(json).expect("valid worker JSON"));
        let mut summary = AggregatedResults::default();
        summary.extend(results);
        assert_eq!(summary.passed, 1);
        assert_eq!(summary.skipped, 1);
        assert_eq!(summary.total_duration_ms, 6.0);
        assert_eq!(summary.failures[0].info.error.traceback, "assertion");
        assert_eq!(summary.errors[0].info.error.traceback, "exception");
        assert!(summary.has_failures());
    }
}
