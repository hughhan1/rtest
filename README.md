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
| [flask](https://github.com/pallets/flask) | 226 ms ± 5 ms | 34 ms ± 11 ms | **6.57x** |
| [click](https://github.com/pallets/click) | 221 ms ± 10 ms | 38 ms ± 7 ms | **5.77x** |
| [httpx](https://github.com/encode/httpx) | 344 ms ± 12 ms | 33 ms ± 4 ms | **10.56x** |
| [pydantic](https://github.com/pydantic/pydantic) | 1.60 s ± 21 ms | 82 ms ± 13 ms | **19.52x** |
| [fastapi](https://github.com/tiangolo/fastapi) | 1.59 s ± 20 ms | 62 ms ± 5 ms | **25.57x** |
| [more-itertools](https://github.com/more-itertools/more-itertools) | 156 ms ± 5 ms | 29 ms ± 5 ms | **5.32x** |
| [boltons](https://github.com/mahmoud/boltons) | 234 ms ± 7 ms | 35 ms ± 3 ms | **6.77x** |

### Test Execution (`--runner native`)

For repositories that don't rely on pytest fixtures or conftest.py, the native runner can execute tests directly:

```bash
hyperfine --warmup 3 --min-runs 20 --max-runs 20 \
  --command-name pytest --command-name rtest \
  ".venv/bin/pytest tests" \
  ".venv/bin/rtest --runner native tests"
```

| Repository | pytest | rtest | Speedup |
|------------|--------|-------|---------|
| [flask](https://github.com/pallets/flask) | 764 ms ± 15 ms | 205 ms ± 5 ms | **3.72x** |
| [more-itertools](https://github.com/more-itertools/more-itertools) | 8.90 s ± 194 ms | 1.34 s ± 185 ms | **6.65x** |

> **Note**: Native runner execution benchmarks are limited to repositories that use simple test patterns (unittest.TestCase,
> plain assertions) without pytest fixtures. Most real-world projects use fixtures and conftest.py, which require the
> native runner's [fixtures support](https://github.com/hughhan1/rtest/issues/105) (in development).

### Test Execution (`--runner pytest -n 4`)

For repositories that use pytest fixtures and conftest.py, rtest can use pytest as the execution backend while still
benefiting from fast Rust-based collection:

```bash
hyperfine --warmup 3 --min-runs 20 --max-runs 20 \
  --command-name pytest --command-name rtest \
  ".venv/bin/pytest -n 4 tests" \
  ".venv/bin/rtest --runner pytest -n 4 tests"
```

| Repository | pytest | rtest | Speedup |
|------------|--------|-------|---------|
| [more-itertools](https://github.com/more-itertools/more-itertools) | 8.96 s ± 218 ms | 1.48 s ± 240 ms | **6.04x** |

> **Note**: Most repositories have limited `--runner pytest` benchmarks due to
> [test ID generation differences](https://github.com/hughhan1/rtest/issues/124) between rtest and pytest for certain
> parametrized values. When rtest generates a different test ID than pytest expects, pytest cannot locate the test.

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
- **Cross-module constant resolution** - Resolve constants imported from other modules in `@rtest.mark.cases`
- **Built-in assertions** - `rtest.raises()` and other assertion helpers
- **Additional markers** - `@rtest.mark.xfail`, `@rtest.mark.skipif`
- **Test selection** - `-k` expression filtering, `--last-failed`, `--failed-first`
- **Better error formatting** - Rich diffs, colorized output, traceback filtering
- **Async test support** - Native `async def test_*()` handling
- **Watch mode** - Re-run tests automatically on file changes

## Known Limitations

### Parametrized Test Discovery

`rtest` expands parametrized tests during collection when the decorator arguments can be **statically resolved**. This
includes:

- **Literal values**: numbers, strings, booleans, None, lists/tuples of literals
- **Module-level constants**: `DATA = [1, 2, 3]` then `@cases("x", DATA)`
- **Class constants**: `Config.MAX_SIZE`
- **Enum members**: `Color.RED`
- **Nested class constants**: `Outer.Inner.VALUE`

```python
from enum import Enum
import rtest

class Color(Enum):
    RED = 1
    GREEN = 2

DATA = [1, 2, 3]

@rtest.mark.cases("value", DATA)  # Resolves to [1, 2, 3]
def test_module_constant(value):
    assert value > 0

@rtest.mark.cases("color", [Color.RED, Color.GREEN])  # Resolves enum members
def test_enum_values(color):
    assert color.value in [1, 2]
```

However, `rtest` cannot statically analyze **truly dynamic expressions** and will emit a warning while falling back to
the base test name:

```python
from other_module import DATA  # Imported from another module

@pytest.mark.parametrize("value", DATA)
def test_example(value):
    assert value > 0

@pytest.mark.parametrize("value", get_data())  # Function call
def test_dynamic(value):
    assert value > 0

@pytest.mark.parametrize("value", [x for x in range(3)])  # Comprehension
def test_comprehension(value):
    assert value > 0
```

```plaintext
warning: Cannot statically expand test cases for 'test.py::test_example': argvalues references variable 'DATA'
warning: Cannot statically expand test cases for 'test.py::test_dynamic': argvalues contains function call 'get_data'
warning: Cannot statically expand test cases for 'test.py::test_comprehension': argvalues contains a comprehension
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
