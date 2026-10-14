# rtest

[![PyPI version](https://badge.fury.io/py/rtest.svg)](https://badge.fury.io/py/rtest)
[![Python](https://img.shields.io/pypi/pyversions/rtest.svg)](https://pypi.org/project/rtest/)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

A Python test runner built with Rust, featuring high-performance test collection and parallel test execution.

> **⚠️ Development Status**: This project is in early development (v0.0.x). Expect bugs, breaking changes, and evolving
> features as we work toward stability.

## Performance

`rtest` discovers tests through static AST analysis without importing test modules. CI measures collection on fixed
Click, Flask, and Pydantic source revisions using a release build, 3 warmups, and 20 measured runs.

The [benchmark job](https://github.com/hughhan1/rtest/actions/workflows/ci.yml) publishes timings, collected test IDs,
and raw Hyperfine reports. See [the benchmark guide](scripts/benchmark/README.md) to reproduce a run.

Static discovery and pytest's import-based collection perform different work and can discover different tests.
The benchmark reports collection time only; it does not claim equivalent test coverage, execution speedups,
or performance regressions across different CI machines.

## Quick Start

### Installation

```bash
pip install rtest
```

_Requires Python 3.10+_

Building from source requires Rust 1.97+ and the initialized Ruff submodule (`git submodule update --init`).

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
- **Imported constants**: `from other_module import DATA` / `from colors import Color`
- **Inherited class attributes**: members defined on a parent class
- **`pytest.param(..., id=...)`**: explicit case IDs

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

`@pytest.mark.parametrize` uses pytest-compatible IDs (runtime values for primitives, `Color.RED` for enums).
`@rtest.mark.cases` keeps source-path IDs (`Config.MAX_SIZE`, `TEST_DATA[1]`).

However, `rtest` cannot statically analyze **truly dynamic expressions** and will emit a warning while falling back to
the base test name:

```python
@pytest.mark.parametrize("value", get_data())  # Function call
def test_dynamic(value):
    assert value > 0

@pytest.mark.parametrize("value", [x for x in range(3)])  # Comprehension
def test_comprehension(value):
    assert value > 0
```

```plaintext
warning: Cannot statically expand test cases for 'test.py::test_dynamic': argvalues contains function call 'get_data'
warning: Cannot statically expand test cases for 'test.py::test_comprehension': argvalues contains a comprehension
```

Use `--cannot-expand-report` to control the extra machine-readable lines (`rtest-cannot-expand: <nodeid>`). The default is `stdout`. Pass `stderr`, a file path, or `none` to omit them (human-readable warnings still print).

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

### Native Runner Scope

The native runner (`--runner native`) does not currently support pytest fixtures or `conftest.py` discovery. Projects
that rely on fixtures should use `--runner pytest` instead. Fixture support is tracked in
[#105](https://github.com/hughhan1/rtest/issues/105).

### pytest-xdist Serialization with Decimal Objects

When using `--runner pytest -n N` (parallel pytest execution), projects that use `Decimal` objects in parametrized test
data may fail with a serialization error:

```plaintext
execnet.gateway_base.DumpError: can't serialize <class 'decimal.Decimal'>
```

This is a limitation of pytest-xdist's inter-process communication, which cannot serialize certain Python types. Tests
pass when run without parallelization (`--runner pytest` without `-n`). See
[#116](https://github.com/hughhan1/rtest/issues/116) for details.

### Collection Failures on Complex Codebases

Some projects with complex runtime behavior (e.g., extensive use of plugins, dynamic imports, or metaclass-heavy
patterns) may fail during `rtest` collection despite working correctly with `pytest`. For example, the
[pydantic](https://github.com/pydantic/pydantic) repository encounters errors when collected by `rtest`. See
[#55](https://github.com/hughhan1/rtest/issues/55) for details.

## Contributing

We welcome contributions! See [Contributing Guide](CONTRIBUTING.rst).

## License

MIT - see [LICENSE](LICENSE) file for details.

---

## Acknowledgments

This project takes inspiration from [Astral](https://astral.sh) and leverages crates from [`ruff`].
