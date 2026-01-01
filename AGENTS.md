# Development Guide for rtest

This guide provides instructions for AI agents and developers working on rtest.

## Project Overview

**rtest** is a high-performance Python test runner built in Rust that provides:

- **Resilient test collection** - continues running tests even when some files fail to collect
- **Built-in parallelization** - no external plugins required
- **Performance** - up to 100x faster than pytest for collection and execution
- **Compatibility** - drop-in replacement for pytest

## Initial Setup

### 1. Clone and Initialize Submodules

```bash
git clone https://github.com/hughhan1/rtest.git
cd rtest

# Initialize git submodules (required for ruff dependency)
git submodule update --init --recursive
```

### 2. Install Dependencies

```bash
# Install uv if not already installed
curl -LsSf https://astral.sh/uv/install.sh | sh

# Sync development dependencies
uv sync --dev
```

### 3. Set up Rust Toolchain

```bash
# Update to stable Rust
rustup update stable

# Install required components (rustfmt and clippy)
rustup component add rustfmt clippy
```

## Development Workflow

**Important**: Always use `uv run` to run Python commands in this project:

- Run Python: `uv run python`
- Run tests: `uv run pytest tests/`
- Build with maturin: `uv run maturin develop`
- Format code after changes: `uv run ruff format python/ tests/ scripts/`

### Building the Project

```bash
# Install rtest in development mode (required for Python bindings)
uv run maturin develop

# For release builds (faster, but takes longer to compile)
uv run maturin develop --release
```

### Running rtest

```bash
# After maturin develop, run as Python module
uv run python -m rtest --collect-only

# Or directly if installed in environment
uv run rtest --collect-only

# With specific test files
uv run rtest tests/test_example.py --collect-only

# Run tests with parallel workers
uv run rtest tests/ -n auto
```

## Testing

### Rust Tests

```bash
# Run all Rust tests
cargo test

# Run specific test
cargo test test_name

# Run with output
cargo test -- --nocapture
```

### Python Tests

```bash
# Run all Python integration tests
uv run pytest tests/ -v

# Run specific test file
uv run pytest tests/test_collection_integration.py -v

# Run with more verbosity and debug output
uv run pytest tests/ -vvv --log-level=debug -s

# Clear cache before running
uv run pytest tests/ --cache-clear
```

### Integration Testing

```bash
# Build and test import
uv run maturin develop
uv run python -c "import rtest; print('Import successful')"

# Test CLI
uv run rtest --help
uv run rtest --version
```

## Code Quality

### Linting and Formatting

#### Python

```bash
# Format Python code
uv run ruff format python/ tests/ scripts/

# Check formatting without making changes
uv run ruff format --check python/ tests/ scripts/

# Run linter
uv run ruff check python/ tests/ scripts/

# Run linter with auto-fix
uv run ruff check --fix python/ tests/ scripts/

# Type checking
uv run ty check python/ tests/ scripts/

# Dead code detection
uv run vulture python/ tests/ scripts/ --min-confidence 80
```

#### Rust

```bash
# Check for compilation errors and warnings (built-in linter)
cargo check

# Format Rust code
cargo fmt

# Check formatting without making changes
cargo fmt --check

# Run clippy (advanced linter with more checks)
cargo clippy

# Run clippy on all targets with strict warnings
cargo clippy --all-targets -- -D warnings

# Run clippy on specific package
cargo clippy -p rtest --lib -- -D warnings
```

### Running All Quality Checks

```bash
# If the quality check script is available
.claude/scripts/quality-check.sh
```

## Common Development Tasks

### Adding a New Feature

1. Create a feature branch:

   ```bash
   git checkout -b feature/your-feature-name
   ```

2. Make your changes

3. Build and test:

   ```bash
   uv run maturin develop
   cargo test
   uv run pytest tests/ -v
   ```

4. Run linting:

   ```bash
   cargo fmt
   cargo clippy
   uv run ruff format python/ tests/ scripts/
   uv run ruff check python/ tests/ scripts/
   ```

5. Commit with conventional commit format:

   ```bash
   git commit -m "feat: add new feature description"
   ```

### Debugging Collection Issues

```bash
# Test collection on a specific file
uv run rtest path/to/test_file.py --collect-only

# Compare with pytest
uv run pytest path/to/test_file.py --collect-only

# Run with verbose output
RUST_LOG=debug uv run rtest path/to/test_file.py --collect-only
```

## Project Structure

```plaintext
rtest/
├── src/                       # Rust source code
│   ├── lib.rs                 # Library entry point + module declarations
│   ├── pyo3.rs                # PyO3 Python bindings
│   ├── cli.rs                 # Command-line argument parsing
│   ├── config.rs              # Pytest config parsing from pyproject.toml
│   ├── collection/            # Test collection logic (uses Rc<Session>)
│   │   ├── mod.rs             # Module entry point
│   │   ├── nodes.rs           # Session, Module, Class, Function collectors
│   │   ├── config.rs          # Collection configuration
│   │   ├── error.rs           # Collection error types
│   │   ├── types.rs           # Collector traits and types
│   │   └── utils.rs           # Collection utilities
│   ├── python_discovery/      # Python AST parsing for test discovery
│   │   ├── mod.rs             # Module entry point
│   │   ├── discovery.rs       # Test discovery logic
│   │   ├── visitor.rs         # AST visitor for test functions
│   │   ├── pattern.rs         # Test name pattern matching
│   │   ├── module_resolver.rs # Python module resolution
│   │   └── semantic_analyzer.rs # Semantic analysis for decorators
│   ├── collection_integration.rs # Bridge between collection and execution
│   ├── runner.rs              # PytestRunner for parallel pytest execution
│   ├── pytest_executor.rs     # Direct pytest subprocess execution
│   ├── native_runner.rs       # Native test runner (no pytest dependency)
│   ├── scheduler.rs           # Test distribution across workers
│   ├── worker.rs              # Worker pool management
│   ├── subproject.rs          # Monorepo subproject detection
│   └── utils.rs               # Utility functions (worker count, etc.)
├── python/rtest/              # Python package (internal + user API)
│   ├── __init__.py            # Main API and CLI entry point
│   ├── __main__.py            # Entry point for `python -m rtest`
│   ├── mark.py                # Test decorators (@rtest.mark.cases, @rtest.mark.skip)
│   └── worker/                # Native test runner worker (spawned by Rust)
│       ├── __init__.py        # Worker package entry point
│       ├── __main__.py        # Entry point for worker subprocess
│       └── runner.py          # Test execution without pytest
├── tests/                     # Python integration tests
├── scripts/                   # Utility scripts
├── Cargo.toml                 # Rust crate configuration
└── pyproject.toml             # Python package configuration
```

## Key Technical Decisions

### Memory Safety ✅

- **Collection system uses `Rc<Session>`** for safe shared ownership
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

## Code Quality Standards

### Rust Best Practices

#### Maintainability

- **Error Handling**: Use `Result<T, E>`, never `panic!` in library code
- **Error Context**: Add context with `.context()` or `.map_err()` for meaningful error chains
- **Graceful Fallbacks**: Pattern match on errors to provide fallbacks (e.g., return empty cache on read failure)
- **Module Visibility**: Use `pub(crate)` for internal APIs, minimize public surface
- **Module Organization**: Group related functionality in submodules with clear `mod.rs` entry points
- **Trait Implementations**: Implement `From`/`Into` for conversions, `Display` for user-facing messages
- **Derive Macros**: Use `#[derive(Debug, Clone, Default)]` where appropriate
- **Type Aliases**: Use `type` aliases to clarify domain concepts (e.g., `type RelativePath = Path`)

#### Performance

- **Iterator Chains**: Prefer lazy iterator chains over collecting to intermediate `Vec`s
- **Clone Awareness**: Minimize `.clone()` on large types; prefer references or `Rc`/`Arc`
- **String Efficiency**: Prefer `&str` parameters; use `.into()` over `.to_string()` for conversions
- **Cow for Flexibility**: Use `Cow<str>` when ownership is conditional
- **Collection Selection**: Use `HashMap` for lookups, `BTreeMap` for deterministic ordering

#### Simplicity

- **Memory Management**: Use `Rc`/`Arc` for shared ownership, avoid raw pointers
- **Option Combinators**: Use `.is_some_and()`, `.map()`, `.unwrap_or_default()` over verbose match
- **Testing**: Unit tests in same file with `#[cfg(test)]`, integration tests in `tests/`
- **Documentation**: `///` for public APIs with examples, `//` for implementation details
- **Clippy Compliance**: Run `cargo clippy`; use `#[expect(clippy::...)]` with justification when suppressing

### Python Best Practices

- **Typing**: Type checking with ty, type all public interfaces
- **Style**: ruff formatting with 120 char line length
- **Testing**: pytest with descriptive test names and good coverage
- **Documentation**: Google-style docstrings for public APIs

### Type Annotation Guidelines

- **Prefer structured types over generic dicts**: Use `@dataclass(frozen=True)`, `NamedTuple`, or `TypedDict` instead of
  `dict[str, Any]` or nested dictionaries when the structure is known
- **Use modern generic syntax**: Prefer `list[str]`, `dict[str, int]`, `tuple[int, str]` over `List[str]`,
  `Dict[str, int]`, `Tuple[int, str]` from the `typing` module. Add `from __future__ import annotations` when needed
  for Python 3.9 compatibility
- **Use union syntax**: Prefer `str | None` over `Optional[str]`
- **Named tuples for return values**: When returning multiple values with fixed meaning, use `NamedTuple` or
  `@dataclass` instead of bare `tuple[...]`

## Troubleshooting

### Build Issues

```bash
# Clean build artifacts
cargo clean

# Update dependencies
cargo update

# Rebuild from scratch
cargo clean
uv run maturin develop
```

### Missing Rust Components

If you encounter errors about missing `rustfmt` or `clippy`:

```bash
# Install required Rust components
rustup component add rustfmt clippy
```

### Submodule Issues

```bash
# If ruff submodule is not initialized
git submodule update --init --recursive

# Update submodules to latest
git submodule update --remote
```

**Note**: This project uses ruff as a git submodule (not the Python package) because it depends on internal ruff Rust
crates for Python AST parsing.

### Python Import Issues

```bash
# Ensure maturin develop was run
uv run maturin develop

# Check if module is installed
uv run python -c "import rtest; print(rtest.__file__)"
```

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

## Performance Considerations

### Critical Paths

1. **Test collection**: Python AST parsing and file traversal
2. **Worker coordination**: Distributing tests across processes
3. **Test execution**: Either via pytest subprocess or native Python worker
4. **Result aggregation**: Combining outputs from workers

### Optimization Guidelines

- **String operations**: Use `.into()` instead of `.to_string()` when possible
- **Collections**: Use iterators instead of collecting to Vec unnecessarily
- **Memory allocation**: Reuse buffers where possible
- **Parallel execution**: Balance worker count with overhead

## File Templates

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

## CI/CD Configuration

### GitHub Actions Matrix

The CI pipeline tests across multiple configurations:

- **Operating Systems**: ubuntu-22.04, ubuntu-24.04, windows-2022
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

## Commit Message Format

We use Conventional Commits for automated versioning:

- `feat: add new feature` → minor version bump
- `fix: resolve bug` → patch version bump
- `feat!: breaking change` → major version bump
- `docs: update documentation` → no version bump
- `test: add or update tests` → no version bump
- `refactor: code refactoring` → no version bump
- `chore: maintenance tasks` → no version bump

## Resources

- [Contributing Guide](CONTRIBUTING.rst)
- [Ruff Documentation](https://docs.astral.sh/ruff/)
- [PyO3 Documentation](https://pyo3.rs/)
- [Pytest Documentation](https://docs.pytest.org/)
