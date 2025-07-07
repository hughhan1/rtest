//! Collection types and traits.

use std::path::PathBuf;

/// Location information for a test item
#[derive(Debug, Clone)]
pub struct Location {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub name: String,
}

