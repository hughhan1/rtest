//! Collection types and traits.

use std::path::{Path, PathBuf};

/// Location information for a test item
#[derive(Debug, Clone)]
pub struct Location {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub name: String,
}

/// Directory collector
#[derive(Debug, Clone)]
pub struct Directory {
    pub path: PathBuf,
    pub nodeid: String,
}

impl Directory {
    pub fn new(path: &Path, rootpath: &Path) -> Self {
        let nodeid = path
            .strip_prefix(rootpath)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();

        Self {
            path: path.to_path_buf(),
            nodeid,
        }
    }
}

/// Module collector
#[derive(Debug, Clone)]
pub struct Module {
    pub path: PathBuf,
    pub nodeid: String,
}

impl Module {
    pub fn new(path: &Path, rootpath: &Path) -> Self {
        let nodeid = path
            .strip_prefix(rootpath)
            .unwrap_or(path)
            .to_string_lossy()
            .into_owned();

        Self {
            path: path.to_path_buf(),
            nodeid,
        }
    }
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
pub enum Collector {
    Directory(Directory),
    Module(Module),
    Function(Function),
}

impl Collector {
    pub fn nodeid(&self) -> &str {
        match self {
            Collector::Directory(d) => &d.nodeid,
            Collector::Module(m) => &m.nodeid,
            Collector::Function(f) => &f.nodeid,
        }
    }

    pub fn path(&self) -> &Path {
        match self {
            Collector::Directory(d) => &d.path,
            Collector::Module(m) => &m.path,
            Collector::Function(f) => &f.location.path,
        }
    }

    pub fn is_item(&self) -> bool {
        matches!(self, Collector::Function(_))
    }
}

