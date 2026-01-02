# rtest

[![PyPI version](https://badge.fury.io/py/rtest.svg)](https://badge.fury.io/py/rtest)
[![Python](https://img.shields.io/pypi/pyversions/rtest.svg)](https://pypi.org/project/rtest/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A Python test runner built with Rust, featuring high-performance test collection and parallel test execution.

> **⚠️ Development Status**: This project is in early development (v0.0.x). Expect bugs, breaking changes, and evolving
> features as we work toward stability.

## Performance

### Test Collection (`--collect-only`)

Benchmarks performed using [hyperfine](https://github.com/sharkdp/hyperfine) with the following command:

```bash
hyperfine --warmup 3 --min-runs 20 --max-runs 20 \
  --command-name pytest --command-name rtest \
  ".venv/bin/pytest --collect-only -q tests" \
  ".venv/bin/rtest --collect-only tests"
```

| Repository | pytest | rtest | Speedup |
|------------|--------|-------|---------|
| [flask](https://github.com/pallets/flask) | 421 ms ± 12 ms | 43 ms ± 3 ms | **9.81x** |
| [click](https://github.com/pallets/click) | 570 ms ± 341 ms | 86 ms ± 8 ms | **6.61x** |
| [httpx](https://github.com/encode/httpx) | 1.71 s ± 0.76 s | 51 ms ± 5 ms | **33.53x** |
| [pydantic](https://github.com/pydantic/pydantic) | 2.84 s ± 0.24 s | 121 ms ± 9 ms | **23.42x** |
| [fastapi](https://github.com/tiangolo/fastapi) | 2.98 s ± 0.29 s | 107 ms ± 14 ms | **27.80x** |

## Quick Start

### Installation

```bash
pip install rtest
```

_Requires Python 3.10+_

### Basic Usage

```bash
# Collect tests (fast AST-based collection)
rtest --collect-only

# Run tests with native runner
rtest --runner native

# Run tests in parallel (4 workers)
rtest --runner native -n 4
```

### Native Runner

The native runner (`--runner native`) executes tests using rtest's own decorators:

```python
import rtest

@rtest.mark.cases("value", [1, 2, 3])
def test_example(value):
    assert value > 0

@rtest.mark.skip(reason="Not implemented yet")
def test_skipped():
    pass
```

The native runner respects `python_files`, `python_classes`, and `python_functions` patterns from your
`pyproject.toml` under `[tool.pytest.ini_options]`.

For compatibility, `@pytest.mark.parametrize` and `@pytest.mark.skip` decorators are also supported.

## Roadmap

- **Fixtures** - `@rtest.fixture` with function/class/module scopes and dependency resolution
- **conftest.py support** - Fixture discovery across directory hierarchy
- **Distribution modes** - Group tests by module/class, optimized scheduling algorithms
- **Dynamic `@rtest.mark.cases` evaluation** - Support variables, function calls, and comprehensions in decorator arguments
- **Built-in assertions** - `rtest.raises()` and other assertion helpers
- **Additional markers** - `@rtest.mark.xfail`, `@rtest.mark.skipif`
- **Test selection** - `-k` expression filtering, `--last-failed`, `--failed-first`
- **Better error formatting** - Rich diffs, colorized output, traceback filtering
- **Async test support** - Native `async def test_*()` handling
- **Watch mode** - Re-run tests automatically on file changes

## Known Limitations

### Parametrized Test Discovery

`rtest` expands parametrized tests during collection when the decorator arguments are **literal values** (numbers,
strings, booleans, None, lists/tuples of literals). For example:

```python
@pytest.mark.parametrize("value", [1, 2, 3])
def test_example(value):
    assert value > 0
```

**Both pytest and rtest collection show:**

```plaintext
test_example[0]
test_example[1]
test_example[2]
```

However, when parametrize arguments contain **dynamic expressions** (variables, function calls, comprehensions), `rtest`
cannot statically analyze them and will emit a warning while falling back to the base test name:

```python
DATA = [1, 2, 3]

@pytest.mark.parametrize("value", DATA)  # Dynamic - references a variable
def test_example(value):
    assert value > 0
```

```plaintext
warning: Cannot statically expand test cases for 'test.py::test_example': argvalues references variable 'DATA'
```

In these cases, test execution is still functionally equivalent - pytest automatically runs all parametrized variants
when given the base function name.

### Path Separator Handling

`rtest` uses platform-specific path separators in test nodeids, while `pytest` normalizes all paths to use forward
slashes (`/`) regardless of platform. For example:

**On Windows:**

- pytest shows: `tests/unit/test_example.py::test_function`
- rtest shows: `tests\unit\test_example.py::test_function`

**On Unix/macOS:**

- Both show: `tests/unit/test_example.py::test_function`

This difference is intentional as `rtest` preserves the native path format of the operating system.

## Contributing

We welcome contributions! See [Contributing Guide](CONTRIBUTING.rst).

## License

MIT - see [LICENSE](LICENSE) file for details.

---

## Acknowledgments

This project takes inspiration from [Astral](https://astral.sh) and leverages crates from [`ruff`].
