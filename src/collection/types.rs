//! Collection types and traits.

use super::error::{CollectionResult, CollectionWarning};
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
    #[expect(
        clippy::type_complexity,
        reason = "returns both child nodes and warnings"
    )]
    fn collect(&self) -> CollectionResult<(Vec<Box<dyn Collector>>, Vec<CollectionWarning>)>;

    /// Get the path associated with this collector
    #[allow(dead_code)]
    fn path(&self) -> &Path;

    /// Check if this is a test item (leaf node)
    fn is_item(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct DummyCollector {
        nodeid: String,
        path: PathBuf,
        item: bool,
    }

    impl Collector for DummyCollector {
        fn nodeid(&self) -> &str {
            &self.nodeid
        }

        fn parent(&self) -> Option<&dyn Collector> {
            None
        }

        fn collect(&self) -> CollectionResult<(Vec<Box<dyn Collector>>, Vec<CollectionWarning>)> {
            Ok((vec![], vec![]))
        }

        fn path(&self) -> &Path {
            &self.path
        }

        fn is_item(&self) -> bool {
            self.item
        }
    }

    #[derive(Debug)]
    struct DefaultCollector;

    impl Collector for DefaultCollector {
        fn nodeid(&self) -> &str {
            "default"
        }

        fn parent(&self) -> Option<&dyn Collector> {
            None
        }

        fn collect(&self) -> CollectionResult<(Vec<Box<dyn Collector>>, Vec<CollectionWarning>)> {
            Ok((vec![], vec![]))
        }

        fn path(&self) -> &Path {
            Path::new("default.py")
        }
    }

    #[test]
    fn test_location_fields() {
        let loc = Location {
            path: PathBuf::from("tests/test_x.py"),
            line: Some(10),
            name: "test_x".into(),
        };
        assert_eq!(loc.path, PathBuf::from("tests/test_x.py"));
        assert_eq!(loc.line, Some(10));
        assert_eq!(loc.name, "test_x");
    }

    #[test]
    fn test_location_clone() {
        let loc = Location {
            path: PathBuf::from("a.py"),
            line: None,
            name: "n".into(),
        };
        let cloned = loc.clone();
        assert_eq!(cloned.path, loc.path);
        assert_eq!(cloned.line, loc.line);
        assert_eq!(cloned.name, loc.name);
    }

    #[test]
    fn test_default_is_item_is_false() {
        let collector = DefaultCollector;
        assert!(!collector.is_item());
    }

    #[test]
    fn test_collector_trait_methods() {
        let collector = DummyCollector {
            nodeid: "mod.py::test".into(),
            path: PathBuf::from("mod.py"),
            item: true,
        };
        assert_eq!(collector.nodeid(), "mod.py::test");
        assert_eq!(collector.path(), Path::new("mod.py"));
        assert!(collector.parent().is_none());
        assert!(collector.is_item());

        let (children, warnings) = collector.collect().expect("collect should succeed");
        assert!(children.is_empty());
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_collector_as_trait_object() {
        let collector: Box<dyn Collector> = Box::new(DummyCollector {
            nodeid: "n".into(),
            path: PathBuf::from("p.py"),
            item: false,
        });
        assert_eq!(collector.nodeid(), "n");
        assert!(!collector.is_item());
    }
}
