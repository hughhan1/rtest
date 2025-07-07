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

/// Directory collector
#[derive(Debug, Clone)]
pub struct Directory {
    pub path: PathBuf,
    pub nodeid: String,
}

/// Module collector
#[derive(Debug, Clone)]
pub struct Module {
    pub path: PathBuf,
    pub nodeid: String,
}

/// Function collector
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub nodeid: String,
    pub location: Location,
}

/// Concrete collector types as an enum
#[derive(Debug, Clone)]
pub enum CollectorNode {
    Directory(Directory),
    Module(Module),
    Function(Function),
}

impl CollectorNode {
    pub fn nodeid(&self) -> &str {
        match self {
            CollectorNode::Directory(d) => &d.nodeid,
            CollectorNode::Module(m) => &m.nodeid,
            CollectorNode::Function(f) => &f.nodeid,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            CollectorNode::Directory(d) => &d.path,
            CollectorNode::Module(m) => &m.path,
            CollectorNode::Function(f) => &f.location.path,
        }
    }

    pub fn is_item(&self) -> bool {
        matches!(self, CollectorNode::Function(_))
    }

    pub fn collect(&self, session: &crate::collection::nodes::Session) -> CollectionResult<Vec<CollectorNode>> {
        match self {
            CollectorNode::Directory(d) => d.collect(session),
            CollectorNode::Module(m) => m.collect(session),
            CollectorNode::Function(_) => Ok(vec![]), // Functions are leaf nodes
        }
    }
}