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

`rtest` delivers significant performance improvements over [`pytest`](https://pytest.org) across popular open-source Python projects:

### Test Collection Performance
```
Repository      pytest               rtest                Speedup
-----------     ------               -----                -------
FastAPI         5.583s ± 0.083s      0.096s ± 0.000s     58.00x
Flask           0.502s ± 0.004s      0.044s ± 0.001s     11.35x
Requests        0.442s ± 0.002s      0.040s ± 0.000s     11.09x
Click           0.395s ± 0.001s      0.043s ± 0.000s     9.22x
HTTPX           0.259s ± 0.003s      0.044s ± 0.000s     5.89x
Scikit-learn    0.241s ± 0.003s      0.225s ± 0.001s     1.07x
Pandas          0.242s ± 0.002s      0.514s ± 0.004s     0.47x
```

### Test Execution Performance (Sequential)
```
Repository      pytest               rtest                Speedup
-----------     ------               -----                -------
Requests        7.488s ± 0.003s      0.034s ± 0.002s     219.09x
Flask           1.724s ± 0.009s      0.035s ± 0.000s     49.00x
Click           1.403s ± 0.006s      0.035s ± 0.000s     40.65x
FastAPI         0.665s ± 0.007s      0.035s ± 0.000s     18.92x
Django          0.577s ± 0.023s      0.037s ± 0.001s     15.77x
HTTPX           0.259s ± 0.003s      0.034s ± 0.000s     7.51x
Scikit-learn    0.239s ± 0.002s      0.060s ± 0.001s     3.97x
Pandas          0.243s ± 0.003s      0.061s ± 0.001s     3.99x
```

### Test Execution Performance (Parallel with -n auto)
```
Repository      pytest               rtest                Speedup
-----------     ------               -----                -------
Requests        8.241s ± 0.009s      0.034s ± 0.000s     243.79x
Flask           1.995s ± 0.013s      0.035s ± 0.000s     56.32x
Click           1.726s ± 0.028s      0.035s ± 0.000s     49.64x
FastAPI         1.597s ± 0.013s      0.035s ± 0.000s     45.13x
Django          1.311s ± 0.011s      0.036s ± 0.000s     36.81x
HTTPX           0.258s ± 0.002s      0.034s ± 0.000s     7.54x
Pandas          0.243s ± 0.002s      0.061s ± 0.001s     3.99x
Scikit-learn    0.237s ± 0.001s      0.060s ± 0.001s     3.94x
```

*Benchmarks performed using [hyperfine](https://github.com/sharkdp/hyperfine) with 20 runs, 3 warmup runs per measurement. Results show mean ± standard deviation across popular Python projects on Ubuntu Linux (GitHub Actions runner).*

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

## Known Limitations

### Parametrized Test Discovery
`rtest` currently discovers only the base function names for parametrized tests (created with `@pytest.mark.parametrize`), rather than expanding them into individual test items during collection. For example:

```python
@pytest.mark.parametrize("value", [1, 2, 3])
def test_example(value):
    assert value > 0
```

**pytest collection shows:**
```
test_example[1]
test_example[2] 
test_example[3]
```

**rtest collection shows:**
```
test_example
```

However, when `rtest` executes tests using pytest as the executor, passing the base function name (`test_example`) to pytest results in identical behavior - pytest automatically runs all parametrized variants. This means test execution is functionally equivalent between the tools, but collection counts may differ.

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