# Rustic

A fast Python test runner built with Rust.

## Key Features

### Resilient Test Collection
Unlike pytest which stops execution when collection errors occur, Rustic continues running tests even when some files fail to collect. This means you get partial test results while fixing syntax errors or other collection issues.

**pytest behavior:**
```
collected 22 items / 3 errors
!!!!!!!!!!!!!!!!!!!!! Interrupted: 3 errors during collection !!!!!!!!!!!!!!!!!!!!!!!!!
============================== 1 warning, 3 errors in 0.97s ==============================
# No tests are executed
```

**rustic behavior:**
```
collected 22 items / 3 errors
!!!!!!!!!!!!!!!!!! Interrupted: 3 errors during collection !!!!!!!!!!!!!!!!!!!!!
================================== test session starts ===================================
# Continues to run the 22 successfully collected tests
```

This partial-success approach provides immediate feedback on working tests while you fix collection errors in problematic files.

## Usage

### As a Python module

```python
from rustic import run_tests

# Run tests
run_tests()

# Run tests with specific pytest arguments
run_tests(pytest_args=["tests/", "-v"])
```

### As a CLI tool

```bash
# Run all tests
rustic

# Run specific tests
rustic tests/test_example.py

# Run with pytest arguments
rustic -- -v -k test_specific
```


## License

MIT