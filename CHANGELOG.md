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

## [Unreleased]
