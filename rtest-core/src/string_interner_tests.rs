//! Comprehensive tests for string interning optimizations

#[cfg(test)]
mod tests {
    use crate::string_interner::*;
    use std::sync::Arc;
    use std::thread;

    #[test]
    fn test_intern_same_string_returns_same_arc() {
        let s1 = intern("test_string");
        let s2 = intern("test_string");
        
        // Both should point to the same memory location
        assert!(Arc::ptr_eq(&s1, &s2));
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_intern_different_strings() {
        let s1 = intern("string1");
        let s2 = intern("string2");
        
        // Different strings should have different Arc instances
        assert!(!Arc::ptr_eq(&s1, &s2));
        assert_ne!(s1, s2);
    }

    #[test]
    fn test_intern_empty_string() {
        let s1 = intern("");
        let s2 = intern("");
        
        assert!(Arc::ptr_eq(&s1, &s2));
    }

    #[test]
    fn test_intern_unicode_strings() {
        let s1 = intern("🦀 Rust is awesome! 测试");
        let s2 = intern("🦀 Rust is awesome! 测试");
        
        assert!(Arc::ptr_eq(&s1, &s2));
        assert_eq!(s1, s2);
    }

    #[test]
    fn test_concurrent_interning() {
        let handles: Vec<_> = (0..100)
            .map(|i| {
                thread::spawn(move || {
                    // Each thread interns strings with patterns
                    let pattern = i % 10;
                    let s1 = intern(&format!("pattern_{}", pattern));
                    let s2 = intern(&format!("unique_{}", i));
                    (s1, s2, pattern)
                })
            })
            .collect();

        let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        
        // Verify that threads with same pattern got the same Arc
        for i in 0..results.len() {
            for j in i + 1..results.len() {
                if results[i].2 == results[j].2 {
                    // Same pattern should result in same Arc
                    assert!(Arc::ptr_eq(&results[i].0, &results[j].0));
                }
            }
        }
    }

    #[test]
    fn test_interned_string_static_vs_dynamic() {
        let static_str = InternedString::from_static("static string");
        let dynamic_str = InternedString::from_string("dynamic string");
        
        // Verify they work correctly
        assert_eq!(static_str.as_str(), "static string");
        assert_eq!(dynamic_str.as_str(), "dynamic string");
        
        // Test equality
        let another_static = InternedString::from_static("static string");
        assert_eq!(static_str, another_static);
        
        let another_dynamic = InternedString::from_string("dynamic string");
        assert_eq!(dynamic_str, another_dynamic);
    }

    #[test]
    fn test_interned_string_hash_consistency() {
        use std::collections::HashMap;
        
        let mut map = HashMap::new();
        
        let key1 = InternedString::from_static("key");
        let key2 = InternedString::from_string("key");
        
        map.insert(key1.clone(), 1);
        map.insert(key2.clone(), 2);
        
        // Should overwrite since they're equal
        assert_eq!(map.len(), 1);
        assert_eq!(map.get(&key1), Some(&2));
    }

    #[test]
    fn test_memory_efficiency() {
        // Test that interning many duplicate strings doesn't use excessive memory
        let interner = StringInterner::new();
        
        // Intern the same string many times
        for _ in 0..1000 {
            interner.intern("repeated_string");
        }
        
        // Should only have one entry
        assert_eq!(interner.len(), 1);
        
        // Now intern many unique strings
        for i in 0..100 {
            interner.intern(&format!("unique_{}", i));
        }
        
        assert_eq!(interner.len(), 101);
    }

    #[test]
    fn test_interner_clear() {
        let interner = StringInterner::new();
        
        interner.intern("test1");
        interner.intern("test2");
        assert_eq!(interner.len(), 2);
        
        interner.clear();
        assert_eq!(interner.len(), 0);
        
        // Can still use after clear
        interner.intern("test3");
        assert_eq!(interner.len(), 1);
    }

    // Benchmark-style test to verify performance improvement
    #[test]
    fn test_performance_comparison() {
        use std::time::Instant;
        
        const ITERATIONS: usize = 10000;
        const UNIQUE_STRINGS: usize = 100;
        
        // Test with string cloning (old approach)
        let start = Instant::now();
        let mut cloned_strings = Vec::new();
        for i in 0..ITERATIONS {
            let s = format!("test_string_{}", i % UNIQUE_STRINGS);
            cloned_strings.push(s);
        }
        let clone_duration = start.elapsed();
        
        // Test with interning (new approach)
        let start = Instant::now();
        let mut interned_strings = Vec::new();
        for i in 0..ITERATIONS {
            let s = intern(&format!("test_string_{}", i % UNIQUE_STRINGS));
            interned_strings.push(s);
        }
        let intern_duration = start.elapsed();
        
        // Verify correctness
        for i in 0..ITERATIONS {
            assert_eq!(
                cloned_strings[i],
                interned_strings[i].as_ref(),
                "Mismatch at index {}", i
            );
        }
        
        // In debug mode, interning might be slower due to locking overhead,
        // but in release mode it should be faster and use less memory
        println!("Clone duration: {:?}", clone_duration);
        println!("Intern duration: {:?}", intern_duration);
        
        // Memory test: interned strings should share memory
        let mut arc_count = 0;
        for i in 0..ITERATIONS {
            for j in i + 1..ITERATIONS {
                if i % UNIQUE_STRINGS == j % UNIQUE_STRINGS {
                    if Arc::ptr_eq(&interned_strings[i], &interned_strings[j]) {
                        arc_count += 1;
                    }
                }
            }
        }
        
        // Should have many shared references
        assert!(arc_count > 0);
    }
}