//! Pattern matching utilities for test discovery.

use glob::Pattern;

/// Check if a name matches a pytest-style glob pattern (fnmatch / `glob` semantics).
pub fn matches(pattern: &str, name: &str) -> bool {
    Pattern::new(pattern)
        .map(|p| p.matches(name))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        assert!(matches("test_*", "test_foo"));
        assert!(matches("test_*", "test_"));
        assert!(!matches("test_*", "foo_test"));

        assert!(matches("Test*", "TestCase"));
        assert!(matches("Test*", "Test"));
        assert!(!matches("Test*", "MyTest"));

        assert!(matches("*_test", "foo_test"));
        assert!(!matches("*_test", "test_foo"));

        assert!(matches("*Test*", "MyTestCase"));
        assert!(matches("*Test*", "SuiteTestRunner"));
        assert!(!matches("*Test*", "NoMatch"));
    }
}
