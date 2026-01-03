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
- Benchmark configurations (collect_only, execution_native, execution_pytest, startup_time)
- Per-repository benchmark overrides

### Benchmark Types

| Benchmark | Description | pytest args | rtest args |
|-----------|-------------|-------------|------------|
| `collect_only` | Test discovery performance | `--collect-only -q` | `--collect-only` |
| `execution_native` | Test execution with rtest's native runner | (none) | `--runner native` |
| `execution_pytest` | Test execution via pytest with parallelism | `-n 4` | `--runner pytest -n 4` |
| `startup_time` | CLI startup overhead | `--version` | `--version` |

### Per-Repository Overrides

Some repositories require custom benchmark configuration due to test suite incompatibilities. Use `benchmark_overrides` in `repositories.yml`:

```yaml
- name: "example-repo"
  # ... other config ...
  benchmark_overrides:
    execution_pytest:
      pytest_args: ""        # Override pytest arguments
      rtest_args: "--runner pytest"  # Override rtest arguments
      skip: true             # Skip this benchmark entirely
```

### Known Repository Issues

| Repository | Issue | Workaround |
|------------|-------|------------|
| **httpx** | Test suite has 1 failing test; tests hang with pytest-xdist `-n 4` | `execution_native` and `execution_pytest` benchmarks skipped |
| **more-itertools** | pytest-xdist can't serialize `range`, `Decimal`, `type` objects in test parameters | `execution_pytest` runs without `-n 4` parallelism |

## Requirements

- `hyperfine` - Command-line benchmarking tool
- `git` - For cloning repositories
- `uv` - Python package manager
- `pyyaml` - For reading configuration (installed via dev dependencies)