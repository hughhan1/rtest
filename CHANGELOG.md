# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-07-03

### Added
- **Resilient Test Collection**: Unlike pytest which stops execution when collection errors occur, RTest continues running tests even when some files fail to collect, providing partial test results while fixing syntax errors
- **Python Package Integration**: Auto-detection of current Python environment with seamless pytest integration
- **Parallel Test Execution**: pytest-xdist style parallel test execution for improved performance
- **Comprehensive Error Handling**: Collect all syntax errors instead of failing fast, allowing developers to see all issues at once
- **Smart Test Filtering**: Outputs collected tests and filters test files with intelligent pattern matching
- **Python Module and CLI Support**: Can be used both as a Python module (`from rtest import run_tests`) and as a CLI tool (`rtest`)
- **Fatal Error Handling**: Robust handling of Python parse errors and collection failures

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

## [Unreleased]
