# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
