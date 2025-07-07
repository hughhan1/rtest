//! Collection error types.

use std::fmt;
use std::sync::Arc;

/// Result type for collection operations
pub type CollectionResult<T> = Result<T, CollectionError>;

/// Collection-specific errors
#[derive(Debug)]
#[allow(dead_code, clippy::enum_variant_names)]
pub enum CollectionError {
    IoError(Arc<std::io::Error>),
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
        CollectionError::IoError(Arc::new(err))
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