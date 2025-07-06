//! String interning for efficient memory usage
//!
//! This module provides string interning capabilities to reduce memory allocations
//! for frequently used strings like test node IDs and group names.

use std::collections::HashMap;
use std::sync::{Arc, RwLock, OnceLock};

/// Global string interner instance
static INTERNER: OnceLock<StringInterner> = OnceLock::new();

/// A thread-safe string interner that deduplicates strings
pub struct StringInterner {
    cache: RwLock<HashMap<String, Arc<str>>>,
}

impl StringInterner {
    /// Create a new string interner
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
        }
    }

    /// Get or create an interned string
    pub fn intern(&self, s: &str) -> Arc<str> {
        // Fast path: check if already interned
        {
            let cache = self.cache.read().unwrap();
            if let Some(interned) = cache.get(s) {
                return Arc::clone(interned);
            }
        }

        // Slow path: intern the string
        let mut cache = self.cache.write().unwrap();
        // Double-check in case another thread interned it
        if let Some(interned) = cache.get(s) {
            return Arc::clone(interned);
        }

        let interned = Arc::from(s);
        cache.insert(s.to_string(), Arc::clone(&interned));
        interned
    }

    /// Get the number of interned strings (for testing/debugging)
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.cache.read().unwrap().len()
    }

    /// Clear the interner (for testing)
    #[cfg(test)]
    pub fn clear(&self) {
        self.cache.write().unwrap().clear();
    }
}

/// Get the global string interner
pub fn get_interner() -> &'static StringInterner {
    INTERNER.get_or_init(StringInterner::new)
}

/// Intern a string using the global interner
#[inline]
pub fn intern(s: &str) -> Arc<str> {
    get_interner().intern(s)
}

/// A string type that can be either static or interned
#[derive(Debug, Clone)]
pub enum InternedString {
    Static(&'static str),
    Interned(Arc<str>),
}

impl InternedString {
    /// Create from a static string
    pub const fn from_static(s: &'static str) -> Self {
        InternedString::Static(s)
    }

    /// Create from a dynamic string (will be interned)
    pub fn from_string(s: &str) -> Self {
        InternedString::Interned(intern(s))
    }

    /// Get the string slice
    pub fn as_str(&self) -> &str {
        match self {
            InternedString::Static(s) => s,
            InternedString::Interned(s) => s,
        }
    }
}

impl AsRef<str> for InternedString {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for InternedString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl PartialEq for InternedString {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for InternedString {}

impl std::hash::Hash for InternedString {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_string_interning() {
        let interner = StringInterner::new();
        
        let s1 = interner.intern("hello");
        let s2 = interner.intern("hello");
        let s3 = interner.intern("world");

        // Same strings should have the same Arc
        assert!(Arc::ptr_eq(&s1, &s2));
        assert!(!Arc::ptr_eq(&s1, &s3));

        assert_eq!(interner.len(), 2);
    }

    #[test]
    fn test_global_interner() {
        // Clear any previous state
        if let Some(interner) = INTERNER.get() {
            interner.clear();
        }

        let s1 = intern("test");
        let s2 = intern("test");
        let s3 = intern("other");

        assert!(Arc::ptr_eq(&s1, &s2));
        assert!(!Arc::ptr_eq(&s1, &s3));
    }

    #[test]
    fn test_interned_string() {
        let static_str = InternedString::from_static("static");
        let dynamic_str = InternedString::from_string("dynamic");

        assert_eq!(static_str.as_str(), "static");
        assert_eq!(dynamic_str.as_str(), "dynamic");

        // Test Display
        assert_eq!(format!("{}", static_str), "static");
        assert_eq!(format!("{}", dynamic_str), "dynamic");

        // Test equality
        let another_static = InternedString::from_static("static");
        assert_eq!(static_str, another_static);
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let handles: Vec<_> = (0..10)
            .map(|i| {
                thread::spawn(move || {
                    let s = intern(&format!("thread_{}", i % 3));
                    (s, i % 3)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

        // Verify that threads with the same i%3 got the same Arc
        for i in 0..results.len() {
            for j in i + 1..results.len() {
                if results[i].1 == results[j].1 {
                    assert!(Arc::ptr_eq(&results[i].0, &results[j].0));
                }
            }
        }
    }
}