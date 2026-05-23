//! Pytest-compatible failure cache (`.pytest_cache/v/cache/lastfailed`).

use crate::collection_integration::CollectedItem;
use regex::Regex;
use serde_json::{Map, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LastFailedMode {
    #[default]
    None,
    LastFailed,
    FailedFirst,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum LastFailedNoFailures {
    #[default]
    All,
    None,
}

impl std::str::FromStr for LastFailedNoFailures {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "all" => Ok(Self::All),
            "none" => Ok(Self::None),
            other => Err(format!(
                "invalid --lfnf value: {other} (choose 'all' or 'none')"
            )),
        }
    }
}

#[derive(Debug)]
pub enum CacheError {
    Io(std::io::Error),
    Json(serde_json::Error),
    InvalidFormat(String),
}

impl std::fmt::Display for CacheError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CacheError::Io(e) => write!(f, "cache I/O error: {e}"),
            CacheError::Json(e) => write!(f, "cache JSON error: {e}"),
            CacheError::InvalidFormat(msg) => write!(f, "cache format error: {msg}"),
        }
    }
}

impl std::error::Error for CacheError {}

impl From<std::io::Error> for CacheError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<serde_json::Error> for CacheError {
    fn from(value: serde_json::Error) -> Self {
        Self::Json(value)
    }
}

pub fn cache_dir_for_root(root: &Path, configured: Option<&PathBuf>) -> PathBuf {
    configured
        .cloned()
        .unwrap_or_else(|| root.join(".pytest_cache"))
}

fn lastfailed_path(cache_dir: &Path) -> PathBuf {
    cache_dir.join("v").join("cache").join("lastfailed")
}

pub fn read_lastfailed(cache_dir: &Path) -> Result<HashSet<String>, CacheError> {
    let path = lastfailed_path(cache_dir);
    if !path.exists() {
        return Ok(HashSet::new());
    }
    let content = fs::read_to_string(&path)?;
    let value: Value = serde_json::from_str(&content)?;
    let obj = value
        .as_object()
        .ok_or_else(|| CacheError::InvalidFormat("lastfailed is not a JSON object".into()))?;
    Ok(obj.keys().cloned().collect())
}

pub fn write_lastfailed(cache_dir: &Path, failed: &HashSet<String>) -> Result<(), CacheError> {
    let path = lastfailed_path(cache_dir);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut map = Map::new();
    for key in failed {
        map.insert(key.clone(), Value::Bool(true));
    }
    let content = serde_json::to_string_pretty(&Value::Object(map))?;
    fs::write(path, content)?;
    Ok(())
}

pub fn clear_cache(cache_dir: &Path) -> Result<(), CacheError> {
    let path = lastfailed_path(cache_dir);
    if path.exists() {
        fs::remove_file(path)?;
    }
    Ok(())
}

pub fn apply_last_failed(
    items: Vec<CollectedItem>,
    mode: LastFailedMode,
    lfnf: LastFailedNoFailures,
    cache_dir: &Path,
) -> Result<Vec<CollectedItem>, CacheError> {
    match mode {
        LastFailedMode::None => Ok(items),
        LastFailedMode::LastFailed => {
            let lastfailed = read_lastfailed(cache_dir)?;
            if lastfailed.is_empty() {
                return match lfnf {
                    LastFailedNoFailures::All => Ok(items),
                    LastFailedNoFailures::None => Ok(vec![]),
                };
            }
            let filtered: Vec<_> = items
                .into_iter()
                .filter(|item| lastfailed.contains(&item.nodeid))
                .collect();
            if !filtered.is_empty() {
                println!(
                    "run-last-failure: rerun previous {} failure(s)",
                    filtered.len()
                );
            }
            Ok(filtered)
        }
        LastFailedMode::FailedFirst => apply_failed_first(items, cache_dir),
    }
}

pub fn apply_failed_first(
    items: Vec<CollectedItem>,
    cache_dir: &Path,
) -> Result<Vec<CollectedItem>, CacheError> {
    let lastfailed = read_lastfailed(cache_dir)?;
    if lastfailed.is_empty() {
        return Ok(items);
    }
    let mut failed_first = Vec::new();
    let mut rest = Vec::new();
    for item in items {
        if lastfailed.contains(&item.nodeid) {
            failed_first.push(item);
        } else {
            rest.push(item);
        }
    }
    if !failed_first.is_empty() {
        println!(
            "run-last-failure: rerun previous {} failure(s) first",
            failed_first.len()
        );
    }
    failed_first.extend(rest);
    Ok(failed_first)
}

/// Update `lastfailed` from structured test outcomes (coordinator-owned for all runners).
pub fn update_lastfailed_from_outcomes(
    cache_dir: &Path,
    outcomes: &[(String, TestOutcome)],
) -> Result<(), CacheError> {
    let mut lastfailed = read_lastfailed(cache_dir)?;
    for (nodeid, outcome) in outcomes {
        match outcome {
            TestOutcome::Failed | TestOutcome::Error => {
                lastfailed.insert(nodeid.clone());
            }
            TestOutcome::Passed | TestOutcome::Skipped => {
                lastfailed.remove(nodeid);
            }
        }
    }
    write_lastfailed(cache_dir, &lastfailed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestOutcome {
    Passed,
    Failed,
    Skipped,
    Error,
}

static TESTCASE_OPEN: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"<testcase\s+([^>]+?)(\s*/>|\s*>)"#).expect("valid regex"));
static ATTR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"(\w+)="([^"]*)""#).expect("valid regex"));

fn junit_nodeid(classname: &str, name: &str) -> String {
    if classname.is_empty() {
        name.to_string()
    } else {
        format!("{classname}::{name}")
    }
}

fn junit_outcome(fragment: &str) -> TestOutcome {
    if fragment.contains("<failure") || fragment.contains("<error") {
        if fragment.contains("<error") {
            TestOutcome::Error
        } else {
            TestOutcome::Failed
        }
    } else if fragment.contains("<skipped") {
        TestOutcome::Skipped
    } else {
        TestOutcome::Passed
    }
}

/// Parse pytest `--junitxml` output into nodeid/outcome pairs.
pub fn parse_junit_xml(path: &Path) -> Result<Vec<(String, TestOutcome)>, CacheError> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)?;
    let mut outcomes = Vec::new();

    for caps in TESTCASE_OPEN.captures_iter(&content) {
        let attrs = &caps[1];
        let self_closing = caps[2].contains('/');

        let mut classname = String::new();
        let mut name = String::new();
        for attr in ATTR.captures_iter(attrs) {
            match &attr[1] {
                "classname" => classname = attr[2].to_string(),
                "name" => name = attr[2].to_string(),
                _ => {}
            }
        }
        if name.is_empty() {
            continue;
        }

        let nodeid = junit_nodeid(&classname, &name);
        let start = caps.get(0).map(|m| m.end()).unwrap_or(0);
        let fragment = if self_closing {
            String::new()
        } else {
            let rest = &content[start..];
            rest.split("</testcase>").next().unwrap_or(rest).to_string()
        };
        outcomes.push((nodeid, junit_outcome(&fragment)));
    }

    Ok(outcomes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lastfailed_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join(".pytest_cache");
        let mut failed = HashSet::new();
        failed.insert("test_foo.py::test_bar".into());
        write_lastfailed(&cache_dir, &failed).unwrap();
        let read = read_lastfailed(&cache_dir).unwrap();
        assert!(read.contains("test_foo.py::test_bar"));
    }

    #[test]
    fn test_parse_junit_xml() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("results.xml");
        fs::write(
            &path,
            r#"<?xml version="1.0" encoding="utf-8"?>
<testsuite>
  <testcase classname="tests/test_a.py" name="test_x" time="0.01"/>
  <testcase classname="tests/test_a.py" name="test_y" time="0.01"><failure message="fail"/></testcase>
</testsuite>"#,
        )
        .unwrap();
        let outcomes = parse_junit_xml(&path).unwrap();
        assert_eq!(outcomes.len(), 2);
        assert!(outcomes
            .iter()
            .any(|(n, o)| n == "tests/test_a.py::test_x" && *o == TestOutcome::Passed));
        assert!(outcomes
            .iter()
            .any(|(n, o)| n == "tests/test_a.py::test_y" && *o == TestOutcome::Failed));
    }

    #[test]
    fn test_lfnf_parse() {
        assert_eq!(
            "all".parse::<LastFailedNoFailures>().unwrap(),
            LastFailedNoFailures::All
        );
        assert_eq!(
            "none".parse::<LastFailedNoFailures>().unwrap(),
            LastFailedNoFailures::None
        );
        assert!("invalid".parse::<LastFailedNoFailures>().is_err());
    }
}
