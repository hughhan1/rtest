# Test Collection Method Comparison

| Method | Total (s) | pytest (s) | rtest (s) | rtest tests | pytest tests | Total tests | Files | Unc. files | Match |
|--------|-----------|------------|-----------|-------------|--------------|-------------|-------|------------|-------|
| Pure pytest | 203.79 | 203.79 | - | - | 27838 | 27838 | 3029 | - | base |
| Hybrid WITH flag | 50.26 | 42.11 | 7.88 | 10962 | 16876 | 27838 | 3029 | 702 | ✓ |
| Hybrid WITHOUT flag | 42.55 | 34.55 | 7.76 | 13617 | 14155 | 27772 | 3029 | 482 | ✗ |
| Native rtest | 8.45 | - | 8.45 | 24995 | - | 24995 | 3024 | - | - |

**Speedups:**
- Hybrid WITH flag: 4.0x faster (100% accurate)
- Hybrid WITHOUT flag: 4.7x faster (missing 66 tests)
- Native rtest: 24.1x faster (missing 2843 tests)
