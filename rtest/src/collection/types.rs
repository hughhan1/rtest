//! Collection types and traits.

use super::error::CollectionResult;
use std::path::{Path, PathBuf};

/// Location information for a test item
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Location {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub name: String,
}

/// Base trait for all collectible nodes
pub trait Collector: std::fmt::Debug {
    /// Unique identifier for this node
    fn nodeid(&self) -> &str;

    /// Parent collector, if any
    #[allow(dead_code)]
    fn parent(&self) -> Option<&dyn Collector>;

    /// Collect child nodes
    fn collect(&self) -> CollectionResult<Vec<Box<dyn Collector>>>;

    /// Get the path associated with this collector
    #[allow(dead_code)]
    fn path(&self) -> &Path;

    /// Check if this is a test item (leaf node)
    fn is_item(&self) -> bool {
        false
    }
}
