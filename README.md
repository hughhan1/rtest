# rtest

[![PyPI version](https://badge.fury.io/py/rtest.svg)](https://badge.fury.io/py/rtest)
[![Python](https://img.shields.io/pypi/pyversions/rtest.svg)](https://pypi.org/project/rtest/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A Python test runner built with Rust, currently supporting high-performance test-collection, with the goal of being a drop-in replacement for [`pytest`](https://pytest.org).

> **⚠️ Development Status**: This project is in early development (v0.0.x). Expect bugs, breaking changes, and evolving features as we work toward stability.

## Performance

*Benchmarks performed using [hyperfine](https://github.com/sharkdp/hyperfine) with 20 runs, 3 warmup runs per measurement, on an M4 Macbook Pro with 14 cores and 48GB RAM.* **More sophisticated benchmarks will be implemented in the future.**

### Against the [`flask`](https://github.com/pallets/flask) Repository
```
hyperfine --command-name pytest --command-name rtest "pytest --collect-only" "rtest --collect-only" --warmup 3 --runs 20
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

### Against the [`httpx`](https://github.com/encode/httpx) Repository
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

### Against the [`pydantic`](https://github.com/pydantic/pydantic) Repository
```
hyperfine --command-name pytest --command-name rtest "pytest --collect-only" "rtest --collect-only" --warmup 3 --runs 20
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

## Quick Start

### Installation

```bash
pip install rtest
```

*Requires Python 3.9+*

### Basic Usage

```bash
rtest --collect-only
```

## Roadmap
Support executing tests, with parallelization built out of the box (bypassing [`pytest-xdist`](https://pypi.org/project/pytest-xdist/)). Currently, this works for some cases, but is not yet stable.

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

We welcome contributions! See [Contributing Guide](CONTRIBUTING.rst).

## License

MIT - see [LICENSE](LICENSE) file for details.

---

## Acknowledgments

This project takes inspiration from [Astral](https://astral.sh) and leverages their excellent Rust crates:
- [`ruff_python_ast`](https://github.com/astral-sh/ruff/tree/main/crates/ruff_python_ast) - Python AST utilities
- [`ruff_python_parser`](https://github.com/astral-sh/ruff/tree/main/crates/ruff_python_parser) - Python parser implementation
