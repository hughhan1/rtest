//! Collection types and traits.

use super::error::{CollectionResult, CollectionWarning};

/// A leaf test has no collection operation; only branches can have children.
#[derive(Debug)]
pub enum CollectionNode {
    Branch(Box<dyn Collector>),
    Test(String),
}

pub trait Collector: std::fmt::Debug {
    fn nodeid(&self) -> &str;
    fn collect(&self) -> CollectionResult<(Vec<CollectionNode>, Vec<CollectionWarning>)>;
}
