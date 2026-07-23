//! Pattern matching utilities for test discovery.

/// Check if a name matches a pattern (supports * wildcards)
pub fn matches(pattern: &str, name: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else if let Some(suffix) = pattern.strip_prefix('*') {
        name.ends_with(suffix)
    } else {
        pattern == name
    }
}

/// Returns `true` if `name` matches any of the given `patterns`.
pub fn matches_any(patterns: &[String], name: &str) -> bool {
    patterns.iter().any(|pattern| matches(pattern, name))
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
    }
}
