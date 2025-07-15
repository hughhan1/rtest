//! Collection error types.

use std::fmt;

/// Result type for collection operations
pub type CollectionResult<T> = Result<T, CollectionError>;

/// Collection-specific errors
#[derive(Debug)]
#[allow(dead_code, clippy::enum_variant_names)]
pub enum CollectionError {
    IoError(std::io::Error),
    ParseError(String),
    ImportError(String),
    SkipError(String),
}

impl fmt::Display for CollectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {e}"),
            Self::ParseError(e) => write!(f, "Parse error: {e}"),
            Self::ImportError(e) => write!(f, "Import error: {e}"),
            Self::SkipError(e) => write!(f, "Skip: {e}"),
        }
    }
}

impl std::error::Error for CollectionError {}

impl From<std::io::Error> for CollectionError {
    fn from(err: std::io::Error) -> Self {
        CollectionError::IoError(err)
    }
}

/// Outcome of a collection operation
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum CollectionOutcome {
    Passed,
    Failed,
    Skipped,
}

/// Collection warnings
#[derive(Debug, Clone)]
pub struct CollectionWarning {
    pub file_path: String,
    pub line: usize,
    pub message: String,
}

impl fmt::Display for CollectionWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}: RtestCollectionWarning: {}",
            self.file_path, self.line, self.message
        )
    }
}
