//! Collection types and traits.

use super::error::{CollectionResult, CollectionWarning};
use super::nodes::Function;
use std::path::PathBuf;

/// Location information for a test item
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct Location {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub name: String,
}

/// A leaf test has no collection operation; only branches can have children.
#[derive(Debug)]
pub enum CollectionNode {
    Branch(Box<dyn Collector>),
    Test(Function),
}

pub trait Collector: std::fmt::Debug {
    fn nodeid(&self) -> &str;
    fn collect(&self) -> CollectionResult<(Vec<CollectionNode>, Vec<CollectionWarning>)>;
}
