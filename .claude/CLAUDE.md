# Claude Development Configuration for rtest

This directory contains comprehensive Claude AI assistant configuration to maximize development velocity and maintain code quality for the rtest project.

## 📁 Configuration Structure

```
.claude/
├── CLAUDE.md              # This file - Complete development guide
├── settings.json          # Project configuration for Claude
├── scripts/               # Development automation scripts (Claude use only)
│   ├── dev-setup.sh       # Environment setup
│   ├── quality-check.sh   # Comprehensive quality checks
│   ├── release-prep.sh    # Release preparation automation
│   └── test-workflows.sh  # Verify configuration works
└── templates/             # Code templates for consistency
    └── rust_module.rs     # Rust module template
```

## 🚀 Quick Start for Claude

### Essential First Steps
```bash
# CRITICAL: Initialize git submodules first (required for ruff dependency)
git submodule update --init --recursive

# Install Python dev dependencies
uv sync --dev

# Run automated setup (optional)
./.claude/scripts/dev-setup.sh

# Test configuration works
./.claude/test-workflows.sh
```

### Daily Development Commands
```bash
# Quick quality check (before commit)
./.claude/scripts/quality-check.sh

# Fast development cycle
uv run maturin develop && cargo test && uv run pytest tests/

# Release preparation
./.claude/scripts/release-prep.sh 0.1.0
```

## 🧠 What This Configuration Provides Claude

### 1. **Comprehensive Project Context**
- **Architecture Understanding**: Hybrid Rust-Python project structure
- **Safety Guarantees**: Memory safety patterns and error handling  
- **Performance Focus**: Optimization guidelines and benchmarking
- **Quality Standards**: Coding conventions and testing strategies

### 2. **Development Workflow Optimization** 
- **Fast Iteration**: Quick build and test cycles via automation scripts
- **Quality Gates**: Automated checks before commit/push
- **Error Prevention**: Proper patterns for memory safety and error handling
- **Templates**: Consistent code patterns for Rust and Python

### 3. **Critical Project Knowledge**
- **uv-based tooling**: All Python commands must use `uv run` prefix
- **Git submodules**: Required initialization for ruff dependency
- **Memory safety**: Uses `Rc<Session>` patterns, no unsafe code
- **Testing strategy**: Comprehensive unit, integration, and performance tests

## Project Overview

**rtest** is a high-performance Python test runner built in Rust that provides:
- **Resilient test collection** - continues running tests even when some files fail to collect
- **Built-in parallelization** - no external plugins required
- **Performance** - up to 100x faster than pytest for collection and execution
- **Compatibility** - drop-in replacement for pytest

## Architecture

```
rtest/
├── src/                    # Python bindings and CLI entry point
│   ├── lib.rs             # PyO3 Python module bindings
│   └── main.rs            # CLI application entry point
├── rtest/                 # Core Rust implementation
│   └── src/
│       ├── collection.rs      # Test collection logic (SAFE: uses Rc<Session>)
│       ├── python_discovery/  # Python AST parsing for test discovery
│       ├── scheduler.rs       # Test distribution across workers
│       ├── worker.rs          # Parallel test execution
│       └── cli.rs            # Command-line argument parsing
├── python/rtest/          # Python package
└── tests/                 # Integration and unit tests
```

## Key Technical Decisions

### Memory Safety ✅
- **Collection system uses `Rc<Session>`** for safe shared ownership (fixed from unsafe raw pointers)
- **Proper error handling** with `Result<T, E>` throughout
- **No unsafe code** in current implementation

### Performance Focus
- **Zero-copy string operations** where possible using `Cow<str>` and `.into()`
- **Iterator chains** to avoid intermediate allocations
- **Parallel execution** with configurable worker pools
- **Fast Python AST parsing** using ruff's parser

### Error Handling Strategy
- **Resilient collection**: Continue collecting tests even when some files fail
- **Proper propagation**: Use `?` operator and `Result` types consistently  
- **User-friendly messages**: Contextual error information
- **No panics**: Replace `unwrap()` with proper error handling

## Development Workflow Commands

### Prerequisites (Critical!)

**Before any development work, these steps are REQUIRED:**

1. **Initialize Git Submodules**: The project depends on ruff crates as submodules
   ```bash
   git submodule update --init --recursive
   ```

2. **Use uv for Python tooling**: All Python commands must be run via `uv run`
   ```bash
   uv sync --dev  # Install dev dependencies including ruff, mypy, pytest
   ```

3. **Verify setup**: Ensure both Rust and Python toolchains work
   ```bash
   cargo check          # Should compile without errors
   uv run ruff --version  # Should show ruff version
   ```

### Quick Development Cycle
```bash
# IMPORTANT: Initialize submodules first (required for ruff dependency)
git submodule update --init --recursive

# Setup development environment
uv sync --dev                                   # Install Python dependencies

# Fast iteration cycle  
cargo check && uv run ruff check .             # Quick syntax/style check
uv run maturin develop                          # Build Python bindings
cargo test && uv run pytest tests/ --tb=short  # Run tests

# Full quality check (before commit)
cargo clippy -- -D warnings && cargo fmt --check && uv run ruff format --check . && uv run mypy python/rtest/
```

### Testing Strategy
```bash
# Rust unit tests
cargo test --lib                              # Core library tests
cd rtest && cargo test                        # Core module tests

# Integration tests  
cargo test --test cli_integration             # CLI integration tests
uv run pytest tests/test_*_integration.py  # Python integration tests

# Performance benchmarks
./benchmark.sh                                # Compare against pytest
```

### Build and Release
```bash
# Development build
uv run maturin develop                         # Local development

# Release build  
uv run maturin build --release                # Production wheel
uv pip install dist/*.whl                     # Test installation
```

## Code Quality Standards

### Rust Best Practices
- **Error Handling**: Always use `Result<T, E>`, never `panic!` in library code
- **Memory Management**: Use `Rc`/`Arc` for shared ownership, avoid raw pointers
- **String Handling**: Use `.into()` for conversions, `Cow<str>` for borrowed/owned
- **Testing**: Unit tests in same file with `#[cfg(test)]`, integration tests in `tests/`
- **Documentation**: `///` for public APIs with examples, `//` for implementation details

### Python Best Practices  
- **Typing**: Strict mypy configuration, type all public interfaces
- **Style**: ruff formatting with 120 char line length
- **Testing**: pytest with descriptive test names and good coverage
- **Documentation**: Google-style docstrings for public APIs

## Common Development Patterns

### Adding New Rust Functionality
1. **Design with safety**: Use `Result` for fallible operations
2. **Test first**: Write failing tests before implementation
3. **Document**: Add `///` docs with examples for public APIs
4. **Memory safety**: Use safe abstractions (`Rc`, `Vec`, etc.)
5. **Error context**: Provide meaningful error messages

### Adding Python Bindings
1. **Update `src/lib.rs`**: Add PyO3 function bindings
2. **Type stubs**: Update `python/rtest/_rtest.pyi` 
3. **Python wrapper**: Add high-level Python API in `python/rtest/__init__.py`
4. **Tests**: Add both Rust and Python tests
5. **Documentation**: Update docstrings and README examples

### Performance Optimization
1. **Profile first**: Use `cargo flamegraph` or similar tools
2. **Measure**: Benchmark before and after changes
3. **Iterative**: Optimize hot paths identified by profiling
4. **Memory**: Minimize allocations in loops
5. **Validate**: Ensure correctness isn't compromised

## Testing Philosophy

### Unit Testing
- **Rust**: Focus on testing individual functions and modules
- **Python**: Test Python API surface and edge cases
- **Mock external dependencies**: File system, network, etc.

### Integration Testing  
- **CLI**: Test actual command-line interface behavior
- **End-to-end**: Real test discovery and execution workflows
- **Cross-language**: Python calling Rust components

### Performance Testing
- **Benchmarks**: Compare against pytest baseline
- **Regression**: Detect performance regressions
- **Profiling**: Identify optimization opportunities

## Debugging Guide

### Rust Debugging
```bash
# Verbose test output
cargo test --verbose -- --nocapture

# Debug prints (remove before commit)
dbg!(variable);

# Clippy for common issues
cargo clippy -- -D warnings
```

### Python Debugging
```bash
# Verbose pytest output  
uv run pytest tests/ -vvv -s

# Type checking
uv run mypy python/rtest/

# Interactive debugging
uv run pytest --pdb tests/test_file.py::test_function
```

### Integration Debugging
```bash
# Test CLI directly
cargo run -- --help
cargo run -- --collect-only tests/

# Test Python module
uv run python -c "import rtest; rtest.run_tests()"
```

## Performance Considerations

### Critical Paths
1. **Test collection**: Python AST parsing and file traversal
2. **Worker coordination**: Distributing tests across processes  
3. **Process spawning**: pytest subprocess execution
4. **Result aggregation**: Combining outputs from workers

### Optimization Guidelines
- **String operations**: Use `.into()` instead of `.to_string()` when possible
- **Collections**: Use iterators instead of collecting to Vec unnecessarily
- **Memory allocation**: Reuse buffers where possible
- **Parallel execution**: Balance worker count with overhead

## Release Process

### Version Management
- **Cargo.toml**: Update version for Rust crates
- **pyproject.toml**: Update version for Python package  
- **CHANGELOG.md**: Document changes following conventional commits

### Quality Gates
1. **Local testing**: Full test suite passes
2. **Performance**: Benchmarks show no regression
3. **Documentation**: README and examples are current
4. **Build**: Clean release build succeeds
5. **Installation**: Wheel installs and works correctly

## Troubleshooting Common Issues

### Build Issues
- **maturin**: Ensure compatible Python and Rust versions
- **PyO3**: Check version compatibility matrix
- **Linking**: May need to rebuild after dependency changes

### Test Issues  
- **Rust tests**: Use `cargo test --verbose` for details
- **Python tests**: Use `pytest -vvv` for maximum verbosity
- **Integration**: Check file permissions and test data setup

### Performance Issues
- **Profile**: Use `cargo flamegraph` to identify bottlenecks
- **Memory**: Check for excessive allocations with `valgrind` 
- **Parallelism**: Verify worker distribution is balanced

## AI Assistant Guidelines

When working on this codebase:

1. **Safety First**: Always use safe Rust patterns, no raw pointers or unsafe blocks
2. **Test Coverage**: Include tests for any new functionality
3. **Error Handling**: Use proper `Result` types, don't panic in library code
4. **Performance Aware**: Consider performance implications of changes
5. **Documentation**: Update docs and examples when changing APIs
6. **Incremental**: Make small, atomic changes that can be easily reviewed
7. **Quality**: Run the full CI pipeline locally before suggesting changes

Remember: This is a performance-critical tool used in CI/CD pipelines. Correctness and reliability are paramount.

---

## 📖 Documentation Structure

This single file replaces the previous `.claude/README.md` to eliminate redundancy. The documentation hierarchy is now:

- **README.md** → Users (installation and usage)
- **CONTRIBUTING.rst** → Contributors (how to contribute + references this file)
- **.claude/CLAUDE.md** → Developers & Claude AI (comprehensive development guide)

**Built for high-velocity, high-quality development of performance-critical testing tools.**

---

## 📋 Quality Gates

### Before Commit
Run these checks before every commit:
```bash
cargo check
cargo clippy -- -D warnings
cargo fmt --check
uv run ruff check .
uv run ruff format --check .
cargo test
uv run pytest tests/ --tb=short
```

### Before Push
Additional checks before pushing:
```bash
cargo test --release
uv run maturin build
uv run pytest tests/ -v
```

## 🏗️ File Templates

### Rust Module Template
```rust
//! Brief module description.
//!
//! More detailed module documentation here.

use std::collections::HashMap;

/// Public struct documentation
#[derive(Debug, Clone)]
pub struct NewStruct {
    field: String,
}

impl NewStruct {
    /// Constructor documentation
    pub fn new(field: String) -> Self {
        Self { field }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_struct_creation() {
        let instance = NewStruct::new("test".into());
        assert_eq!(instance.field, "test");
    }
}
```

### Python Module Template
```python
"""Brief module description.

More detailed module documentation here.
"""

from __future__ import annotations

from typing import Any, Dict, List, Optional


class NewClass:
    """Brief class description.
    
    Args:
        param: Parameter description.
    """
    
    def __init__(self, param: str) -> None:
        self._param = param
    
    def method(self) -> str:
        """Brief method description.
        
        Returns:
            Description of return value.
        """
        return self._param


def function(arg: str) -> str:
    """Brief function description.
    
    Args:
        arg: Argument description.
        
    Returns:
        Description of return value.
    """
    return arg
```

## 🔄 CI/CD Configuration

### GitHub Actions Matrix
The CI pipeline tests across multiple configurations:
- **Operating Systems**: ubuntu-latest, macos-latest, windows-latest
- **Python Versions**: 3.9, 3.10, 3.11, 3.12
- **Rust Version**: stable

### CI Pipeline Steps
1. Setup Rust toolchain
2. Setup Python environment
3. Install dependencies
4. Run Rust tests
5. Run Python tests
6. Build wheels with maturin
7. Upload artifacts

## 🔧 Additional Troubleshooting

### Common Build Issues
- **Maturin issues**: Check Python environment and maturin version compatibility
- **Rust compilation errors**: Ensure Rust toolchain is up to date with `rustup update`
- **Linking errors**: Verify PyO3 version compatibility with Python version

### Common Test Failures
- **Rust test failures**: Use `cargo test --verbose -- --nocapture` for detailed output
- **Python test failures**: Use `uv run pytest -vvv -s` for maximum verbosity
- **Integration test issues**: Verify test environment setup and file permissions