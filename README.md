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

*Benchmarks performed using [hyperfine](https://github.com/sharkdp/hyperfine) with 20 runs, 3 warmup runs per measurement, on an M4 Macbook Pro with 14 cores and 48GB RAM.* **More sophisticated benchmarks will be implemented in the future.**

### Against the [`flask`](https://github.com/pallets/flask) Repository

#### Test Collection Performance
```
hyperfine --command-name pytest --command-name rtest "uv run pytest --collect-only" "uv run rtest --collect-only" --warmup 3 --runs 20
Benchmark 1: pytest
  Time (mean ± σ):     229.9 ms ±   2.6 ms    [User: 184.5 ms, System: 37.4 ms]
  Range (min … max):   226.0 ms … 235.4 ms    20 runs
 
Benchmark 2: rtest
  Time (mean ± σ):      35.8 ms ±   1.2 ms    [User: 18.1 ms, System: 10.7 ms]
  Range (min … max):    34.2 ms …  40.3 ms    20 runs
 
Summary
  rtest ran
    6.41 ± 0.23 times faster than pytest
```

#### Test Execution Performance  
```
hyperfine --command-name pytest --command-name rtest "uv run pytest -n auto" "uv run rtest -n auto" --warmup 3 --runs 20
Benchmark 1: pytest
  Time (mean ± σ):      1.156 s ±  0.021 s    [User: 5.314 s, System: 1.044 s]
  Range (min … max):    1.128 s …  1.205 s    20 runs
 
Benchmark 2: rtest
  Time (mean ± σ):     605.4 ms ±  36.2 ms    [User: 4768.0 ms, System: 954.3 ms]
  Range (min … max):   566.0 ms … 700.1 ms    20 runs
 
Summary
  rtest ran
    1.91 ± 0.12 times faster than pytest
```

### Against the [`httpx`](https://github.com/encode/httpx) Repository

#### Test Collection Performance
```
hyperfine --command-name pytest --command-name rtest "pytest --collect-only" "rtest --collect-only" --warmup 3 --runs 20
Benchmark 1: pytest
  Time (mean ± σ):     310.1 ms ±  18.6 ms    [User: 259.3 ms, System: 42.6 ms]
  Range (min … max):   291.0 ms … 344.4 ms    20 runs
 
Benchmark 2: rtest
  Time (mean ± σ):      20.6 ms ±   1.0 ms    [User: 12.5 ms, System: 5.5 ms]
  Range (min … max):    18.6 ms …  21.9 ms    20 runs
 
Summary
  rtest ran
   15.06 ± 1.15 times faster than pytest
```

#### Test Execution Performance
```
hyperfine --command-name pytest --command-name rtest "pytest" "rtest" --warmup 3 --runs 20 --ignore-failure
Benchmark 1: pytest
  Time (mean ± σ):      3.155 s ±  0.073 s    [User: 1.708 s, System: 0.256 s]
  Range (min … max):    3.087 s …  3.296 s    20 runs
 
  Warning: Ignoring non-zero exit code.
 
Benchmark 2: rtest
  Time (mean ± σ):      2.411 s ±  0.111 s    [User: 1.771 s, System: 0.275 s]
  Range (min … max):    2.335 s …  2.827 s    20 runs
 
Summary
  rtest ran
    1.31 ± 0.07 times faster than pytest
```
*Note: `--ignore-failure` is passed at the moment because there are failures when running both `pytest` and `rtest` against the [`httpx`](https://github.com/encode/httpx) repository for reason not yet investigated.*
*Note: `-n auto` is not used because attempting this command for both `pytest` and `rtest` against the [`httpx`](https://github.com/encode/httpx) repository seems to hang for a reason not yet investigated.*

### Against the [`pydantic`](https://github.com/pydantic/pydantic) Repository

#### Test Collection Performance
```
hyperfine --command-name pytest --command-name rtest "uv run pytest --collect-only" "uv run rtest --collect-only" --warmup 3 --runs 20
Benchmark 1: pytest
  Time (mean ± σ):      2.777 s ±  0.031 s    [User: 2.598 s, System: 0.147 s]
  Range (min … max):    2.731 s …  2.864 s    20 runs
 
Benchmark 2: rtest
  Time (mean ± σ):      61.2 ms ±   1.1 ms    [User: 40.1 ms, System: 14.4 ms]
  Range (min … max):    60.1 ms …  64.2 ms    20 runs
 
Summary
  rtest ran
   45.39 ± 0.95 times faster than pytest
```

#### Test Execution Performance  

```
hyperfine --command-name pytest --command-name rtest "uv run pytest -n auto" "uv run rtest -n auto" --warmup 3 --runs 20 --ignore-failure
Benchmark 1: pytest
  Time (mean ± σ):      5.239 s ±  0.223 s    [User: 48.686 s, System: 4.160 s]
  Range (min … max):    4.964 s …  5.712 s    20 runs
 
  Warning: Ignoring non-zero exit code.
 
Benchmark 2: rtest
  Time (mean ± σ):      3.209 s ±  0.238 s    [User: 21.003 s, System: 5.131 s]
  Range (min … max):    2.935 s …  3.680 s    20 runs
 
  Warning: Ignoring non-zero exit code.
 
Summary
  rtest ran
    1.63 ± 0.14 times faster than pytest
```
*Note: `--ignore-failure` is passed at the moment because there are failures when running both `pytest` and `rtest` against the [`pydantic`](https://github.com/pydantic/pydantic) repository for reason not yet investigated.*

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
