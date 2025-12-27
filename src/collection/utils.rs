//! Collection utility functions.

/// Simple glob pattern matching
pub fn glob_match(pattern: &str, text: &str) -> bool {
    use glob::Pattern;

    // Try to use the glob crate for more accurate matching
    if let Ok(glob_pattern) = Pattern::new(pattern) {
        glob_pattern.matches(text)
    } else {
        // Fallback to simple matching
        if pattern.starts_with('*') && pattern.ends_with('*') {
            let middle = &pattern[1..pattern.len() - 1];
            text.contains(middle)
        } else if let Some(suffix) = pattern.strip_prefix('*') {
            text.ends_with(suffix)
        } else if let Some(prefix) = pattern.strip_suffix('*') {
            text.starts_with(prefix)
        } else {
            pattern == text
        }
    }
}
