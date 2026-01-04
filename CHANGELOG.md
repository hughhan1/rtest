# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.0.45] - 2026-01-03

### Fixed
- Multi-param parametrize tests with complex objects now preserve literal IDs for non-complex parameters (e.g., `(MyData(1), 2, 3)` produces `[data0-2-3]` instead of `[data0]`)

## [0.0.44] - 2026-01-03

### Fixed
- Class-level `@pytest.mark.parametrize` decorators now correctly expand to all methods in the class, including inherited methods
- Parametrized tests with complex objects like dataclass instances, nested dictionaries, and class instantiations now expand correctly with positional fallback IDs

## [0.0.43] - 2026-01-03

### Fixed
- `run_tests()` now returns exit code 1 when collection errors occur, matching the CLI behavior

## [0.0.42] - 2026-01-03

### Fixed
- Diamond inheritance patterns (where a class inherits from multiple parents that share a common ancestor) no longer cause collection to fail with spurious cycle detection errors

## [0.0.41] - 2026-01-03

### Added
- Per-repository benchmark overrides with skip support for more flexible benchmarking configuration

### Chores
- GitHub releases are now created automatically when publishing to PyPI
- Benchmark script no longer exits prematurely when capturing compare_results exit code

## [0.0.40] - 2026-01-03

### Added
- **Constant Resolution in @rtest.mark.cases**: Static parameter expansion now supports enum members (`Color.RED`), class constants (`Config.MAX_SIZE`), module-level constants (`DATA = [1, 2, 3]`), and nested class constants (`Outer.Inner.VALUE`)

### Fixed
- Stacked `@parametrize` decorators now process bottom-to-top (innermost first), matching pytest's behavior for correct test ID generation
- Support for argnames as lists or tuples of strings in `@parametrize`, e.g., `@parametrize(("a", "b"), ...)`
- Generic base classes with type parameters (e.g., `class TestHelper(GenericBase[MyClass])`) no longer cause collection to fail

## [0.0.39] - 2026-01-03

### Fixed
- Parameterized test IDs now use value-based format (`[1]`, `[2]`, `[3]`) instead of index-based (`[0]`, `[1]`, `[2]`), matching pytest's behavior and enabling workflows where rtest handles discovery while pytest handles execution

## [0.0.38] - 2026-01-02

### Added
- **Lazy Collection**: Only parses files users explicitly specify instead of all test files, improving performance when running specific files/directories
- **Nonexistent Path Handling**: Paths that don't exist now fail with exit code 4 (pytest compatibility) with message "ERROR: file or directory not found: <path>"

### Changed
- rtest's own test suite now uses the native runner instead of pytest
- Updated Python code to use modern type syntax (`|` instead of `Union`, `| None` instead of `Optional`)
- Overhauled benchmark script with per-repository configuration and execution benchmarks

### Fixed
- Native runner now adds test directories to `sys.path` before importing modules, fixing sibling imports like `from test_helpers import ...`

## [0.0.37] - 2026-01-01

### Added
- **Native Test Runner**: Execute tests without pytest dependency using `--runner native`. The default runner remains `--runner pytest`
- **rtest Decorators**: New `@rtest.mark.parametrize` and `@rtest.mark.skip` decorators for use with the native runner
- **Parametrized Test Expansion**: AST-based expansion of `@rtest.mark.cases` and `@pytest.mark.parametrize` decorators during collection, generating expanded nodeids like `test_foo[0]`, `test_foo[1]` instead of just `test_foo`
  - Supports literal values (numbers, strings, booleans, None, lists/tuples)
  - Supports custom `ids` parameter for test naming
  - Supports stacked decorators (cartesian product expansion)
  - Emits warnings for dynamic expressions that cannot be statically analyzed
- **pyproject.toml Config Support**: Native runner respects `python_files`, `python_classes`, and `python_functions` patterns from `[tool.pytest.ini_options]`

### Changed
- Deprecation warnings now emitted when using `@pytest.mark.*` decorators with the native runner
- CI now uses tag-triggered PyPI releases
- Replaced mypy with ty (0.0.8) for type checking
- Upgraded ruff to 0.14.10

### Fixed
- Various clippy warnings resolved with improved design patterns
