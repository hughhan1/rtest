# RTest

A fast Python test runner built with Rust.

## Key Features

### Resilient Test Collection
Unlike pytest which stops execution when collection errors occur, RTest continues running tests even when some files fail to collect. This means you get partial test results while fixing syntax errors or other collection issues.

**pytest behavior:**
```
collected 22 items / 3 errors
!!!!!!!!!!!!!!!!!!!!! Interrupted: 3 errors during collection !!!!!!!!!!!!!!!!!!!!!!!!!
============================== 1 warning, 3 errors in 0.97s ==============================
# No tests are executed
```

**rtest behavior:**
```
collected 22 items / 3 errors
!!!!!!!!!!!!!!!!!! Warning: 3 errors during collection !!!!!!!!!!!!!!!!!!!!!
================================== test session starts ===================================
# Continues to run the 22 successfully collected tests
```

This partial-success approach provides immediate feedback on working tests while you fix collection errors in problematic files.

## Usage

### As a Python module

```python
from rtest import run_tests

# Run tests
run_tests()

# Run tests with specific pytest arguments
run_tests(pytest_args=["tests/", "-v"])
```

### As a CLI tool

```bash
# Run all tests
rtest

# Run specific tests
rtest tests/test_example.py

# Run with pytest arguments
rtest -- -v -k test_specific
```

## Development

### Building and Testing

This project is a hybrid Rust/Python package using PyO3 and maturin.

#### Rust Development
```bash
# Run Rust unit tests
cargo test --bin rtest

# Format Rust code
cargo fmt --all

# Run clippy lints
cargo clippy --bin rtest -- -D warnings
```

#### Python Extension Development
```bash
# Build and install the Python extension in development mode
uv run maturin develop

# Test the Python extension
uv run python -c "import rtest; print('Python extension imported successfully')"

# Run Python tests (if any)
uv run pytest
```

#### Full Development Workflow
```bash
# 1. Set up the environment
uv sync

# 2. Build the Python extension
uv run maturin develop

# 3. Run Rust tests
cargo test --bin rtest

# 4. Test Python integration
uv run python -c "import rtest; print('Success')"
```

#### Why Separate Commands?

The Rust codebase has two parts:
- **Binary target** (`src/main.rs`): Standalone Rust application, tested with `cargo test --bin rtest`
- **Library target** (`src/lib.rs`): Python extension built with maturin, requires Python linking

Use `cargo` commands for Rust development and `maturin` for Python extension building.

## License

MIT