//! Native test runner that executes tests via Python workers.

use crate::collection::glob_match;
use serde::Deserialize;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use tempfile::TempDir;
use walkdir::WalkDir;

#[derive(Debug, Clone, Deserialize)]
pub struct TestResult {
    pub nodeid: String,
    pub outcome: String,
    pub duration_ms: f64,
    #[serde(default)]
    pub stdout: String,
    #[serde(default)]
    pub stderr: String,
    pub error: Option<ErrorInfo>,
    #[serde(default)]
    pub error_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ErrorInfo {
    #[serde(default)]
    pub traceback: Option<String>,
    #[serde(default)]
    pub reason: Option<String>,
}

#[derive(Debug, Default)]
pub struct AggregatedResults {
    pub passed: usize,
    pub failed: usize,
    pub error: usize,
    pub skipped: usize,
    pub total: usize,
    pub total_duration_ms: f64,
    pub failures: Vec<TestResult>,
    pub errors: Vec<TestResult>,
}

pub struct NativeRunnerConfig {
    pub python_executable: String,
    pub root_path: PathBuf,
    pub num_workers: usize,
    pub python_files: Vec<String>,
    pub python_classes: Vec<String>,
}

/// Default pytest python_files patterns
pub fn default_python_files() -> Vec<String> {
    vec!["test_*.py".into(), "*_test.py".into()]
}

/// Default pytest python_classes patterns
pub fn default_python_classes() -> Vec<String> {
    vec!["Test*".into()]
}

fn shard_files(files: Vec<PathBuf>, num_workers: usize) -> Vec<Vec<PathBuf>> {
    if num_workers == 0 || files.is_empty() {
        return vec![];
    }

    let actual_workers = num_workers.min(files.len());
    let mut shards: Vec<Vec<PathBuf>> = (0..actual_workers).map(|_| Vec::new()).collect();

    for (i, file) in files.into_iter().enumerate() {
        shards[i % actual_workers].push(file);
    }

    shards.into_iter().filter(|s| !s.is_empty()).collect()
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

fn aggregate_results(all_results: Vec<Vec<TestResult>>) -> AggregatedResults {
    let mut aggregated = AggregatedResults::default();

    for results in all_results {
        for result in results {
            aggregated.total += 1;
            aggregated.total_duration_ms += result.duration_ms;

            match result.outcome.as_str() {
                "passed" => aggregated.passed += 1,
                "failed" => {
                    aggregated.failed += 1;
                    aggregated.failures.push(result);
                }
                "error" => {
                    aggregated.error += 1;
                    aggregated.errors.push(result);
                }
                "skipped" => aggregated.skipped += 1,
                _ => aggregated.error += 1,
            }
        }
    }

    aggregated
}

fn print_error_info(error: &ErrorInfo) {
    if let Some(traceback) = &error.traceback {
        for line in traceback.lines() {
            println!("    {}", line);
        }
    } else if let Some(reason) = &error.reason {
        println!("    {}", reason);
    }
}

fn print_test_results(label: &str, results: &[TestResult]) {
    if results.is_empty() {
        return;
    }
    println!();
    println!("{}:", label);
    for result in results {
        println!("  {}", result.nodeid);
        if let Some(error) = &result.error {
            print_error_info(error);
        }
    }
}

fn print_summary(results: &AggregatedResults) {
    println!();
    println!("================================ SUMMARY ================================");

    print_test_results("FAILURES", &results.failures);
    print_test_results("ERRORS", &results.errors);

    println!();
    let duration_secs = results.total_duration_ms / 1000.0;

    let mut parts = Vec::new();
    if results.passed > 0 {
        parts.push(format!("{} passed", results.passed));
    }
    if results.failed > 0 {
        parts.push(format!("{} failed", results.failed));
    }
    if results.error > 0 {
        parts.push(format!("{} error", results.error));
    }
    if results.skipped > 0 {
        parts.push(format!("{} skipped", results.skipped));
    }

    let status = if results.failed > 0 || results.error > 0 {
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
) -> Result<(), String> {
    let mut cmd = Command::new(python);
    cmd.arg("-m")
        .arg("rtest.worker")
        .arg("--root")
        .arg(root)
        .arg("--out")
        .arg(output_file);

    if !python_classes.is_empty() {
        cmd.arg("--python-classes");
        for pattern in python_classes {
            cmd.arg(pattern);
        }
    }

    // Use -- to separate options from positional file arguments
    cmd.arg("--");
    for file in files {
        cmd.arg(file);
    }

    cmd.current_dir(root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = cmd
        .output()
        .map_err(|e| format!("Worker {} failed to start: {}", worker_id, e))?;

    // Check worker exit code - non-zero indicates test failures or errors
    if !output.status.success() {
        let exit_code = output.status.code().unwrap_or(-1);
        // Exit code 1 is expected when tests fail, only report unexpected codes
        if exit_code != 1 {
            // Print stderr only for unexpected failures
            if !output.stderr.is_empty() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.trim().is_empty() {
                    eprintln!("Worker {} stderr:\n{}", worker_id, stderr);
                }
            }
            return Err(format!(
                "Worker {} crashed with exit code {} (expected 0 or 1)",
                worker_id, exit_code
            ));
        }
    }

    Ok(())
}

pub fn execute_native(config: &NativeRunnerConfig, test_files: Vec<PathBuf>) -> i32 {
    if test_files.is_empty() {
        println!("No test files to run.");
        return 0;
    }

    let num_workers = config.num_workers.max(1);
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

    let shards = shard_files(test_files, num_workers);
    let output_files: Vec<PathBuf> = (0..shards.len())
        .map(|i| temp_dir.path().join(format!("worker-{}.jsonl", i)))
        .collect();

    let mut handles = Vec::new();
    for (i, (shard, output_file)) in shards.into_iter().zip(output_files.iter()).enumerate() {
        let python = config.python_executable.clone();
        let root = config.root_path.clone();
        let output_file = output_file.clone();
        let python_classes = config.python_classes.clone();
        let handle = thread::spawn(move || {
            run_worker(i, &python, &root, &output_file, &shard, &python_classes)
        });
        handles.push(handle);
    }

    let mut worker_errors = Vec::new();
    for (i, handle) in handles.into_iter().enumerate() {
        match handle.join() {
            Ok(Ok(())) => {}
            Ok(Err(e)) => worker_errors.push(format!("Worker {}: {}", i, e)),
            Err(_) => worker_errors.push(format!("Worker {} panicked", i)),
        }
    }

    for error in &worker_errors {
        eprintln!("{}", error);
    }

    let mut all_results = Vec::new();
    for output_file in &output_files {
        if output_file.exists() {
            match parse_results_file(output_file) {
                Ok(results) => all_results.push(results),
                Err(e) => eprintln!("Warning: {}", e),
            }
        }
    }

    let aggregated = aggregate_results(all_results);
    print_summary(&aggregated);

    // TempDir is automatically cleaned up when dropped

    if aggregated.failed > 0 || aggregated.error > 0 || !worker_errors.is_empty() {
        1
    } else {
        0
    }
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

    for pattern in patterns {
        if glob_match(pattern, file_name) {
            return true;
        }
    }

    false
}

fn collect_test_files_in_dir(dir: &Path, patterns: &[String]) -> Vec<PathBuf> {
    WalkDir::new(dir)
        .into_iter()
        .filter_entry(|e| {
            e.file_name()
                .to_str()
                .map(|n| !is_hidden_or_ignored(n))
                .unwrap_or(false)
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && is_test_file(e.path(), patterns))
        .map(|e| e.into_path())
        .collect()
}

pub fn collect_test_files(root: &Path, paths: &[String], patterns: &[String]) -> Vec<PathBuf> {
    let mut test_files = if paths.is_empty() {
        collect_test_files_in_dir(root, patterns)
    } else {
        paths
            .iter()
            .flat_map(|path_str| {
                let path = Path::new(path_str);
                let full_path = if path.is_absolute() {
                    path.to_path_buf()
                } else {
                    root.join(path)
                };

                if full_path.is_file() && is_test_file(&full_path, patterns) {
                    vec![full_path]
                } else if full_path.is_dir() {
                    collect_test_files_in_dir(&full_path, patterns)
                } else {
                    vec![]
                }
            })
            .collect()
    };

    test_files.sort();
    test_files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shard_files_empty() {
        let result = shard_files(vec![], 4);
        assert!(result.is_empty());
    }

    #[test]
    fn test_shard_files_zero_workers() {
        let files = vec![PathBuf::from("a.py"), PathBuf::from("b.py")];
        let result = shard_files(files, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_shard_files_single_worker() {
        let files = vec![
            PathBuf::from("a.py"),
            PathBuf::from("b.py"),
            PathBuf::from("c.py"),
        ];
        let result = shard_files(files.clone(), 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], files);
    }

    #[test]
    fn test_shard_files_round_robin() {
        let files = vec![
            PathBuf::from("a.py"),
            PathBuf::from("b.py"),
            PathBuf::from("c.py"),
            PathBuf::from("d.py"),
            PathBuf::from("e.py"),
        ];
        let result = shard_files(files, 3);

        assert_eq!(result.len(), 3);
        assert_eq!(
            result[0],
            vec![PathBuf::from("a.py"), PathBuf::from("d.py")]
        );
        assert_eq!(
            result[1],
            vec![PathBuf::from("b.py"), PathBuf::from("e.py")]
        );
        assert_eq!(result[2], vec![PathBuf::from("c.py")]);
    }

    #[test]
    fn test_shard_files_more_workers_than_files() {
        let files = vec![PathBuf::from("a.py"), PathBuf::from("b.py")];
        let result = shard_files(files, 5);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], vec![PathBuf::from("a.py")]);
        assert_eq!(result[1], vec![PathBuf::from("b.py")]);
    }

    #[test]
    fn test_is_test_file() {
        let default_patterns = default_python_files();
        assert!(is_test_file(Path::new("test_foo.py"), &default_patterns));
        assert!(is_test_file(Path::new("foo_test.py"), &default_patterns));
        assert!(is_test_file(
            Path::new("path/to/test_bar.py"),
            &default_patterns
        ));
        assert!(!is_test_file(Path::new("foo.py"), &default_patterns));
        assert!(!is_test_file(Path::new("conftest.py"), &default_patterns));
        assert!(!is_test_file(Path::new("test_foo.txt"), &default_patterns));
    }

    #[test]
    fn test_is_test_file_custom_patterns() {
        let patterns = vec!["check_*.py".into(), "*_spec.py".into()];
        assert!(is_test_file(Path::new("check_validation.py"), &patterns));
        assert!(is_test_file(Path::new("user_spec.py"), &patterns));
        assert!(!is_test_file(Path::new("test_foo.py"), &patterns));
        assert!(!is_test_file(Path::new("foo_test.py"), &patterns));
    }

    #[test]
    fn test_aggregate_results() {
        let results = vec![
            vec![
                TestResult {
                    nodeid: "test1".into(),
                    outcome: "passed".into(),
                    duration_ms: 10.0,
                    stdout: String::new(),
                    stderr: String::new(),
                    error: None,
                    error_type: None,
                },
                TestResult {
                    nodeid: "test2".into(),
                    outcome: "failed".into(),
                    duration_ms: 20.0,
                    stdout: String::new(),
                    stderr: String::new(),
                    error: Some(ErrorInfo {
                        traceback: Some("AssertionError".into()),
                        reason: None,
                    }),
                    error_type: Some("AssertionError".into()),
                },
            ],
            vec![TestResult {
                nodeid: "test3".into(),
                outcome: "skipped".into(),
                duration_ms: 0.0,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(ErrorInfo {
                    traceback: None,
                    reason: Some("WIP".into()),
                }),
                error_type: None,
            }],
        ];

        let aggregated = aggregate_results(results);
        assert_eq!(aggregated.total, 3);
        assert_eq!(aggregated.passed, 1);
        assert_eq!(aggregated.failed, 1);
        assert_eq!(aggregated.skipped, 1);
        assert_eq!(aggregated.failures.len(), 1);
        assert_eq!(aggregated.total_duration_ms, 30.0);
    }

    #[test]
    fn test_parse_results_json() {
        // Test deserializing JSON
        let json = r#"{"nodeid":"test::foo","outcome":"passed","duration_ms":1.5,"stdout":"","stderr":"","error":null}"#;
        let result: TestResult = serde_json::from_str(json).unwrap();
        assert_eq!(result.nodeid, "test::foo");
        assert_eq!(result.outcome, "passed");
        assert_eq!(result.duration_ms, 1.5);
    }
}
