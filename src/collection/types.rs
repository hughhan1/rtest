//! Collection nodes. Only directory and module branches can have children.

use std::path::PathBuf;

#[derive(Debug)]
pub enum CollectionNode {
    Branch(CollectionBranch),
    Test(String),
}

#[derive(Debug)]
pub enum CollectionBranch {
    Directory(PathBuf),
    Module(PathBuf),
}
