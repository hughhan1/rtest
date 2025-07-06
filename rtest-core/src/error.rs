//! Comprehensive error handling for rtest-core
//!
//! This module provides a unified error handling system with proper
//! error propagation, context, and recovery strategies.

use std::fmt;
use std::path::PathBuf;

/// Top-level error type for rtest-core
#[derive(Debug)]
pub enum RtestError {
    /// Collection-related errors
    Collection(CollectionError),
    /// Scheduling errors
    Scheduler(SchedulerError),
    /// Worker execution errors
    Worker(WorkerError),
    /// Configuration errors
    Config(ConfigError),
    /// I/O errors with context
    Io(IoError),
    /// Python interop errors
    Python(PythonError),
}

impl fmt::Display for RtestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Collection(e) => write!(f, "Collection error: {}", e),
            Self::Scheduler(e) => write!(f, "Scheduler error: {}", e),
            Self::Worker(e) => write!(f, "Worker error: {}", e),
            Self::Config(e) => write!(f, "Configuration error: {}", e),
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Python(e) => write!(f, "Python error: {}", e),
        }
    }
}

impl std::error::Error for RtestError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Collection(e) => Some(e),
            Self::Scheduler(e) => Some(e),
            Self::Worker(e) => Some(e),
            Self::Config(e) => Some(e),
            Self::Io(e) => Some(e),
            Self::Python(e) => Some(e),
        }
    }
}

/// Collection-specific errors with context
#[derive(Debug)]
pub enum CollectionError {
    /// Failed to parse Python file
    ParseError {
        path: PathBuf,
        line: Option<usize>,
        message: String,
    },
    /// Import error during collection
    ImportError {
        module: String,
        message: String,
    },
    /// Test was skipped
    SkipError {
        nodeid: String,
        reason: String,
    },
    /// Permission denied accessing path
    PermissionDenied {
        path: PathBuf,
    },
    /// Path not found
    NotFound {
        path: PathBuf,
    },
    /// Invalid test pattern
    InvalidPattern {
        pattern: String,
        reason: String,
    },
}

impl fmt::Display for CollectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ParseError { path, line, message } => {
                if let Some(line) = line {
                    write!(f, "Parse error in {} at line {}: {}", path.display(), line, message)
                } else {
                    write!(f, "Parse error in {}: {}", path.display(), message)
                }
            }
            Self::ImportError { module, message } => {
                write!(f, "Import error in module '{}': {}", module, message)
            }
            Self::SkipError { nodeid, reason } => {
                write!(f, "Skipped {}: {}", nodeid, reason)
            }
            Self::PermissionDenied { path } => {
                write!(f, "Permission denied: {}", path.display())
            }
            Self::NotFound { path } => {
                write!(f, "Path not found: {}", path.display())
            }
            Self::InvalidPattern { pattern, reason } => {
                write!(f, "Invalid pattern '{}': {}", pattern, reason)
            }
        }
    }
}

impl std::error::Error for CollectionError {}

/// Scheduler-specific errors
#[derive(Debug, Clone)]
pub enum SchedulerError {
    /// Invalid number of workers
    InvalidWorkerCount {
        requested: usize,
        min: usize,
        max: usize,
    },
    /// No items to distribute
    EmptyInput,
    /// Invalid test path format
    InvalidTestPath {
        path: String,
        expected_format: &'static str,
    },
    /// Group extraction failed
    GroupExtractionError {
        item: String,
        reason: String,
    },
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWorkerCount { requested, min, max } => {
                write!(f, "Invalid worker count {}: must be between {} and {}", requested, min, max)
            }
            Self::EmptyInput => {
                write!(f, "No items to distribute")
            }
            Self::InvalidTestPath { path, expected_format } => {
                write!(f, "Invalid test path '{}': expected format {}", path, expected_format)
            }
            Self::GroupExtractionError { item, reason } => {
                write!(f, "Failed to extract group from '{}': {}", item, reason)
            }
        }
    }
}

impl std::error::Error for SchedulerError {}

/// Worker execution errors
#[derive(Debug)]
pub enum WorkerError {
    /// Worker process failed to start
    SpawnError {
        worker_id: usize,
        command: String,
        source: std::io::Error,
    },
    /// Worker process crashed
    CrashError {
        worker_id: usize,
        exit_code: Option<i32>,
        stderr: String,
    },
    /// Worker communication error
    CommunicationError {
        worker_id: usize,
        message: String,
    },
    /// Timeout waiting for worker
    TimeoutError {
        worker_id: usize,
        duration_secs: u64,
    },
}

impl fmt::Display for WorkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SpawnError { worker_id, command, source } => {
                write!(f, "Failed to spawn worker {}: command '{}' failed: {}", worker_id, command, source)
            }
            Self::CrashError { worker_id, exit_code, stderr } => {
                if let Some(code) = exit_code {
                    write!(f, "Worker {} crashed with exit code {}: {}", worker_id, code, stderr)
                } else {
                    write!(f, "Worker {} crashed: {}", worker_id, stderr)
                }
            }
            Self::CommunicationError { worker_id, message } => {
                write!(f, "Communication error with worker {}: {}", worker_id, message)
            }
            Self::TimeoutError { worker_id, duration_secs } => {
                write!(f, "Worker {} timed out after {} seconds", worker_id, duration_secs)
            }
        }
    }
}

impl std::error::Error for WorkerError {}

/// Configuration errors
#[derive(Debug)]
pub enum ConfigError {
    /// Invalid configuration value
    InvalidValue {
        key: String,
        value: String,
        expected: String,
    },
    /// Missing required configuration
    MissingRequired {
        key: String,
    },
    /// Configuration file error
    FileError {
        path: PathBuf,
        source: std::io::Error,
    },
    /// Parse error in configuration
    ParseError {
        path: PathBuf,
        line: usize,
        message: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidValue { key, value, expected } => {
                write!(f, "Invalid value for '{}': '{}', expected {}", key, value, expected)
            }
            Self::MissingRequired { key } => {
                write!(f, "Missing required configuration: '{}'", key)
            }
            Self::FileError { path, source } => {
                write!(f, "Error reading config file {}: {}", path.display(), source)
            }
            Self::ParseError { path, line, message } => {
                write!(f, "Parse error in {} at line {}: {}", path.display(), line, message)
            }
        }
    }
}

impl std::error::Error for ConfigError {}

/// I/O errors with additional context
#[derive(Debug)]
pub struct IoError {
    /// The operation that failed
    pub operation: std::borrow::Cow<'static, str>,
    /// The path involved (if any)
    pub path: Option<PathBuf>,
    /// The underlying I/O error
    pub source: std::io::Error,
}

impl fmt::Display for IoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(path) = &self.path {
            write!(f, "{} failed for {}: {}", self.operation, path.display(), self.source)
        } else {
            write!(f, "{} failed: {}", self.operation, self.source)
        }
    }
}

impl std::error::Error for IoError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Python interop errors
#[derive(Debug)]
pub enum PythonError {
    /// AST parsing error
    AstError {
        path: PathBuf,
        message: String,
    },
    /// Import resolution error
    ImportResolution {
        module: String,
        from_path: PathBuf,
    },
    /// Invalid Python syntax
    SyntaxError {
        path: PathBuf,
        line: usize,
        column: Option<usize>,
        message: String,
    },
}

impl fmt::Display for PythonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AstError { path, message } => {
                write!(f, "AST error in {}: {}", path.display(), message)
            }
            Self::ImportResolution { module, from_path } => {
                write!(f, "Cannot resolve import '{}' from {}", module, from_path.display())
            }
            Self::SyntaxError { path, line, column, message } => {
                if let Some(col) = column {
                    write!(f, "Syntax error in {} at {}:{}: {}", path.display(), line, col, message)
                } else {
                    write!(f, "Syntax error in {} at line {}: {}", path.display(), line, message)
                }
            }
        }
    }
}

impl std::error::Error for PythonError {}

/// Result type alias for convenience
pub type Result<T> = std::result::Result<T, RtestError>;

/// Extension trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn context(self, msg: &str) -> Result<T>;
    
    /// Add context with a closure (lazy evaluation)
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: Into<RtestError>,
{
    fn context(self, _msg: &str) -> Result<T> {
        self.map_err(|e| {
            let base_error = e.into();
            // In a real implementation, we'd wrap the error with context
            // For now, just return the base error
            base_error
        })
    }
    
    fn with_context<F>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> String,
    {
        self.map_err(|e| {
            let _context = f();
            let base_error = e.into();
            // In a real implementation, we'd wrap the error with context
            base_error
        })
    }
}

/// Helper function to create I/O errors with context
pub fn io_error(operation: impl Into<std::borrow::Cow<'static, str>>, path: Option<PathBuf>, source: std::io::Error) -> IoError {
    IoError {
        operation: operation.into(),
        path,
        source,
    }
}

// Conversion implementations
impl From<std::io::Error> for RtestError {
    fn from(err: std::io::Error) -> Self {
        RtestError::Io(IoError {
            operation: std::borrow::Cow::Borrowed("I/O operation"),
            path: None,
            source: err,
        })
    }
}

impl From<CollectionError> for RtestError {
    fn from(err: CollectionError) -> Self {
        RtestError::Collection(err)
    }
}

impl From<SchedulerError> for RtestError {
    fn from(err: SchedulerError) -> Self {
        RtestError::Scheduler(err)
    }
}

impl From<WorkerError> for RtestError {
    fn from(err: WorkerError) -> Self {
        RtestError::Worker(err)
    }
}

impl From<ConfigError> for RtestError {
    fn from(err: ConfigError) -> Self {
        RtestError::Config(err)
    }
}

impl From<PythonError> for RtestError {
    fn from(err: PythonError) -> Self {
        RtestError::Python(err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = CollectionError::ParseError {
            path: PathBuf::from("test.py"),
            line: Some(42),
            message: "Invalid syntax".to_string(),
        };
        
        let display = format!("{}", err);
        assert!(display.contains("test.py"));
        assert!(display.contains("42"));
        assert!(display.contains("Invalid syntax"));
    }

    #[test]
    fn test_error_conversion() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let rtest_err: RtestError = io_err.into();
        
        match rtest_err {
            RtestError::Io(_) => {}
            _ => panic!("Expected Io error"),
        }
    }
}