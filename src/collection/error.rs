//! Collection error types.

use std::fmt;
use std::path::PathBuf;

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
    FileNotFound(PathBuf),
}

impl fmt::Display for CollectionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IoError(e) => write!(f, "IO error: {e}"),
            Self::ParseError(e) => write!(f, "Parse error: {e}"),
            Self::ImportError(e) => write!(f, "Import error: {e}"),
            Self::SkipError(e) => write!(f, "Skip: {e}"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Error as IoError, ErrorKind};

    #[test]
    fn test_display_io_error() {
        let err = CollectionError::IoError(IoError::other("boom"));
        assert_eq!(err.to_string(), "IO error: boom");
    }

    #[test]
    fn test_display_parse_error() {
        let err = CollectionError::ParseError("bad syntax".into());
        assert_eq!(err.to_string(), "Parse error: bad syntax");
    }

    #[test]
    fn test_display_import_error() {
        let err = CollectionError::ImportError("no module".into());
        assert_eq!(err.to_string(), "Import error: no module");
    }

    #[test]
    fn test_display_skip_error() {
        let err = CollectionError::SkipError("not ready".into());
        assert_eq!(err.to_string(), "Skip: not ready");
    }

    #[test]
    fn test_display_file_not_found() {
        let err = CollectionError::FileNotFound(PathBuf::from("/tmp/missing.py"));
        assert_eq!(
            err.to_string(),
            "file or directory not found: /tmp/missing.py"
        );
    }

    #[test]
    fn test_from_io_error() {
        let io_err = IoError::new(ErrorKind::NotFound, "nope");
        let err: CollectionError = io_err.into();
        match err {
            CollectionError::IoError(inner) => assert_eq!(inner.kind(), ErrorKind::NotFound),
            other => panic!("expected IoError, got {other:?}"),
        }
    }

    #[test]
    fn test_error_trait_object() {
        let err = CollectionError::ParseError("x".into());
        let dyn_err: &dyn std::error::Error = &err;
        assert_eq!(dyn_err.to_string(), "Parse error: x");
    }

    #[test]
    fn test_collection_outcome_equality() {
        assert_eq!(CollectionOutcome::Passed, CollectionOutcome::Passed);
        assert_ne!(CollectionOutcome::Passed, CollectionOutcome::Failed);
        assert_ne!(CollectionOutcome::Failed, CollectionOutcome::Skipped);
    }

    #[test]
    fn test_collection_outcome_is_copy() {
        let outcome = CollectionOutcome::Skipped;
        let copied = outcome;
        // Both remain usable because CollectionOutcome is Copy.
        assert_eq!(outcome, copied);
    }

    #[test]
    fn test_collection_warning_display() {
        let warning = CollectionWarning {
            file_path: "tests/test_foo.py".into(),
            line: 42,
            message: "something odd".into(),
        };
        assert_eq!(
            warning.to_string(),
            "tests/test_foo.py:42: RtestCollectionWarning: something odd"
        );
    }

    #[test]
    fn test_collection_warning_clone() {
        let warning = CollectionWarning {
            file_path: "a.py".into(),
            line: 1,
            message: "m".into(),
        };
        let cloned = warning.clone();
        assert_eq!(warning.to_string(), cloned.to_string());
    }
}
