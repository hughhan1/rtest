# rtest

[![PyPI version](https://badge.fury.io/py/rtest.svg)](https://badge.fury.io/py/rtest)
[![Python](https://img.shields.io/pypi/pyversions/rtest.svg)](https://pypi.org/project/rtest/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A high-performance Python test runner built with Rust, designed as a drop-in replacement for [`pytest`](https://pytest.org) with enhanced collection resilience and built-in parallelization.

> **⚠️ Development Status**: This project is in early development (v0.0.x). While functional, expect breaking changes and evolving features as we work toward stability.

## Features

### Resilient Test Collection
Unlike [`pytest`](https://pytest.org) which stops execution when collection errors occur, `rtest` continues running tests even when some files fail to collect:

**`pytest` stops when collection fails:**
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

#### Distribution Modes

Control how tests are distributed across workers with the `--dist` flag:

- **`--dist load`** (default): Round-robin distribution of individual tests
- **`--dist loadscope`**: Group tests by module/class scope for fixture reuse
- **`--dist loadfile`**: Group tests by file to keep related tests together  
- **`--dist worksteal`**: Optimized distribution for variable test execution times
- **`--dist no`**: Sequential execution (no parallelization)

```bash
# Examples
rtest -n auto --dist loadfile      # Group by file across all CPU cores
rtest -n 4 --dist worksteal        # Work-steal optimized distribution
rtest --dist no                    # Sequential execution for debugging
```

**Note**: The `loadgroup` distribution mode from pytest-xdist is not yet supported. xdist_group mark parsing is planned for future releases.

### Current Implementation
The current implementation focuses on enhanced test collection and parallelization, with test execution delegated to [`pytest`](https://pytest.org) for maximum compatibility.

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

*Performance benchmarks shown are preliminary results from a specific test suite using hyperfine with 20 runs each on MacBook Pro M4 Pro (48GB RAM). Results may vary significantly depending on test suite characteristics, system configuration, and workload. More comprehensive benchmarking across diverse scenarios is planned.*

## Quick Start

### Installation

```bash
pip install rtest
```

*Requires Python 3.9+*

### Basic Usage

```bash
# Drop-in replacement for pytest
rtest

# That's it! All your existing pytest workflows work
rtest tests/
rtest tests/test_auth.py -v
rtest -- -k "test_user" --tb=short
```

## Advanced Usage

### Environment Configuration
```bash
# Set environment variables for your tests
rtest -e DEBUG=1 -e DATABASE_URL=sqlite://test.db

# Useful for testing different configurations
rtest -e ENVIRONMENT=staging -- tests/integration/
```

### Collection and Discovery
```bash
# See what tests would run without executing them
rtest --collect-only

# Mix `rtest` options with any pytest arguments
rtest -n 4 -- -v --tb=short -k "not slow"
```

### Python API
```python
from rtest import run_tests

# Programmatic test execution
run_tests()

# With custom pytest arguments
run_tests(pytest_args=["tests/unit/", "-v", "--tb=short"])

# Suitable for CI/CD pipelines and automation
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