# Repository Benchmarking

This directory contains scripts for benchmarking `rtest` performance against `pytest` across popular Python repositories.

## Usage

From the project root:

```bash
# List available repositories
uv run python scripts/benchmark/benchmark_repositories.py --list-repos

# Run all benchmarks on all repositories
uv run python scripts/benchmark/benchmark_repositories.py

# Run benchmarks on specific repositories
uv run python scripts/benchmark/benchmark_repositories.py --repositories fastapi flask click

# Run only test collection benchmarks (skip execution)
uv run python scripts/benchmark/benchmark_repositories.py --collect-only

# Combine options
uv run python scripts/benchmark/benchmark_repositories.py --repositories click flask --collect-only
```

## Configuration

The `repositories.yml` file contains:
- Repository definitions (name, URL, test directory)
- Benchmark configurations:
  - `collect_only`: Test discovery performance
  - `execution`: Sequential test execution performance
  - `execution_parallel`: Parallel test execution performance using `-n auto --dist load`

## Benchmark Types

The benchmarking suite runs three types of performance tests:

1. **Test Collection** (`collect_only`): Measures how fast each tool can discover tests without executing them
2. **Sequential Execution** (`execution`): Measures test execution performance in a single process
3. **Parallel Execution** (`execution_parallel`): Measures test execution performance using all available CPU cores with `-n auto --dist load`
   - Uses `pytest-xdist` for pytest's parallel execution
   - Uses rtest's built-in parallel execution

## Requirements

- `hyperfine` - Command-line benchmarking tool
- `git` - For cloning repositories
- `uv` - Python package manager
- `pyyaml` - For reading configuration (installed via dev dependencies)