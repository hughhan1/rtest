//! Collection error types.

use std::fmt;
use std::path::PathBuf;

/// Result type for collection operations
pub type CollectionResult<T> = Result<T, CollectionError>;

/// Collection-specific errors
#[derive(Debug)]
pub enum CollectionError {
    IoError(std::io::Error),
    ParseError(String),
    ImportError(String),
    FileNotFound(PathBuf),
}

impl fmt::Display for CollectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {e}"),
            Self::ParseError(e) => write!(f, "Parse error: {e}"),
            Self::ImportError(e) => write!(f, "Import error: {e}"),
            Self::FileNotFound(path) => {
                write!(f, "file or directory not found: {}", path.display())
            }
        }
    }
}

impl std::error::Error for CollectionError {}

impl From<std::io::Error> for CollectionError {
    fn from(err: std::io::Error) -> Self {
        CollectionError::IoError(err)
    }
}

/// Collection warnings
#[derive(Debug, Clone)]
pub struct CollectionWarning {
    pub file_path: String,
    pub line: usize,
    pub message: String,
    /// Structured fallback target; never recovered by parsing the human message.
    pub cannot_expand_nodeid: Option<String>,
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
