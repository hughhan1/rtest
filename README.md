# rtest

[![PyPI version](https://badge.fury.io/py/rtest.svg)](https://badge.fury.io/py/rtest)
[![Python](https://img.shields.io/pypi/pyversions/rtest.svg)](https://pypi.org/project/rtest/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A Python test runner built with Rust.

> **⚠️ Development Notice**: `rtest` is in active development and not yet ready for production use. Expect bugs, incomplete features, and breaking changes as we work toward stability.

## Features

### Resilient Test Collection
Unlike [`pytest`](https://pytest.org) which stops execution when collection errors occur, `rtest` continues running tests even when some files fail to collect:

**`pytest` stops everything when collection fails:**
```bash
collected 22 items / 3 errors
!!!!!!!!!!!!!!!!!!!!! Interrupted: 3 errors during collection !!!!!!!!!!!!!!!!!!!!!!!!
============================== 1 warning, 3 errors in 0.97s ==============================
# No tests run - you're stuck
```

**`rtest` keeps going:**
```bash
collected 22 items / 3 errors
!!!!!!!!!!!!!!!!!! Warning: 3 errors during collection !!!!!!!!!!!!!!!!!!!!!
================================== test session starts ===================================
# Your 22 working tests run while you fix the 3 broken files
```

### Built-in Parallelization
`rtest` includes parallel test execution out of the box, without requiring additional plugins like [`pytest-xdist`](https://github.com/pytest-dev/pytest-xdist). Simply use the `-n` flag to run tests across multiple processes:

```bash
# Run tests in parallel (recommended for large test suites)
rtest -n 4                    # Use 4 processes
rtest -n auto                 # Auto-detect CPU cores
rtest --maxprocesses 8        # Limit maximum processes
```

### Current Implementation
At the current moment, `rtest` delegates to [`pytest`](https://pytest.org) for test execution while providing enhanced collection and parallelization features.

## Performance

`rtest` delivers significant performance improvements over [`pytest`](https://pytest.org):

```bash
=== Full Test Execution Benchmark ===
Benchmark 1: pytest
  Time (mean ± σ):      3.990 s ±  0.059 s    [User: 3.039 s, System: 0.937 s]
  Range (min … max):    3.881 s …  4.113 s    20 runs
 
Benchmark 2: rtest
  Time (mean ± σ):      65.9 ms ±  10.6 ms    [User: 22.9 ms, System: 22.8 ms]
  Range (min … max):    40.6 ms …  78.7 ms    20 runs
 
Summary
  rtest ran
   60.52 ± 9.78 times faster than pytest

=== Test Collection Only Benchmark ===
Benchmark 1: pytest --collect-only
  Time (mean ± σ):      4.051 s ±  0.114 s    [User: 3.060 s, System: 0.959 s]
  Range (min … max):    3.961 s …  4.424 s    20 runs
 
Benchmark 2: rtest --collect-only
  Time (mean ± σ):      40.7 ms ±  11.6 ms    [User: 16.6 ms, System: 12.8 ms]
  Range (min … max):    27.0 ms …  80.8 ms    20 runs
 
Summary
  rtest --collect-only ran
   99.61 ± 28.52 times faster than pytest --collect-only
```

*Performance benchmarks are a work-in-progress. These results are from a typical test suite using hyperfine with 20 runs each on MacBook Pro M4 Pro (48GB RAM). More comprehensive benchmarking is on the roadmap once additional features are implemented.*

## Quick Start

### Installation

```bash
pip install rtest
```

*Requires Python 3.9+*

### Basic Usage

```bash
# Drop-in replacement for [`pytest`](https://pytest.org)
rtest

# That's it! All your existing [`pytest`](https://pytest.org) workflows work
rtest tests/
rtest tests/test_auth.py -v
rtest -- -k "test_user" --tb=short
```

## Advanced Usage

### Environment Configuration
```bash
# Set environment variables for your tests
rtest -e DEBUG=1 -e DATABASE_URL=sqlite://test.db

# Perfect for testing different configurations
rtest -e ENVIRONMENT=staging -- tests/integration/
```

### Collection and Discovery
```bash
# See what tests would run without executing them
rtest --collect-only

# Mix `rtest` options with any [`pytest`](https://pytest.org) arguments
rtest -n 4 -- -v --tb=short -k "not slow"
```

### Python API
```python
from rtest import run_tests

# Programmatic test execution
run_tests()

# With custom [`pytest`](https://pytest.org) arguments
run_tests(pytest_args=["tests/unit/", "-v", "--tb=short"])

# Perfect for CI/CD pipelines and automation
result = run_tests(pytest_args=["--junitxml=results.xml"])
```

### Command Reference

| Option | Description |
|--------|-------------|
| `-n, --numprocesses N` | Run tests in N parallel processes |
| `--maxprocesses N` | Maximum number of worker processes |
| `-e, --env KEY=VALUE` | Set environment variables (can be repeated) |
| `--dist MODE` | Distribution mode for parallel execution (default: load) |
| `--collect-only` | Show what tests would run without executing them |
| `--help` | Show all available options |
| `--version` | Show `rtest` version |

**Pro tip**: Use `--` to separate `rtest` options from [`pytest`](https://pytest.org) arguments:
```bash
rtest -n 4 -e DEBUG=1 -- -v -k "integration" --tb=short
```

## Contributing

We welcome contributions! Check out our [Contributing Guide](CONTRIBUTING.rst) for details on:

- Reporting bugs
- Suggesting features  
- Development setup
- Documentation improvements

## License

MIT - see [LICENSE](LICENSE) file for details.

---

## Acknowledgments

This project takes inspiration from [Astral](https://astral.sh) and leverages their excellent Rust crates:
- [`ruff_python_ast`](https://github.com/astral-sh/ruff/tree/main/crates/ruff_python_ast) - Python AST utilities
- [`ruff_python_parser`](https://github.com/astral-sh/ruff/tree/main/crates/ruff_python_parser) - Python parser implementation

**Built with Rust for the Python community**