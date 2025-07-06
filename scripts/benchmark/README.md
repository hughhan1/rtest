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
- Benchmark configurations (collect_only, execution)

## Requirements

- `hyperfine` - Command-line benchmarking tool
- `git` - For cloning repositories
- `uv` - Python package manager
- `pyyaml` - For reading configuration (installed via dev dependencies)