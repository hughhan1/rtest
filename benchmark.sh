#!/usr/bin/env bash

###
# Performance comparison: uv run pytest vs uv run rtest
# Usage: ./benchmark.sh [--collect-only | --all] [test_directory]
###

# Parse arguments
COLLECT_ONLY=false
ALL=false
TEST_DIR="."

while [[ $# -gt 0 ]]; do
  case $1 in
    --collect-only)
      COLLECT_ONLY=true
      shift
      ;;
    --all)
      ALL=true
      shift
      ;;
    *)
      TEST_DIR="$1"
      shift
      ;;
  esac
done

# Check for mutually exclusive flags
if [[ "$COLLECT_ONLY" == true && "$ALL" == true ]]; then
  echo "Error: --collect-only and --all are mutually exclusive"
  exit 1
fi

echo "Benchmarking pytest performance comparison..."
echo "Test directory: ${TEST_DIR}"
echo

# Check if both commands exist
if ! command -v uv &> /dev/null; then
    echo "Error: uv not found. Please install uv first."
    exit 1
fi

if [ ! -d "${TEST_DIR}" ]; then
    echo "Error: Test directory '${TEST_DIR}' not found."
    exit 1
fi

# Run benchmarks based on flags
if [[ "$COLLECT_ONLY" == true ]]; then
  echo "=== Test Collection Only Benchmark ==="
  hyperfine --warmup 5 --min-runs 20 --prepare 'sleep 0.1' \
    --ignore-failure \
    --command-name "pytest --collect-only" \
    --command-name "rtest --collect-only" \
    "uv run pytest --collect-only ${TEST_DIR}" \
    "uv run rtest --collect-only ${TEST_DIR}"
elif [[ "$ALL" == true ]]; then
  echo "=== Full Test Execution Benchmark ==="
  hyperfine --warmup 5 --min-runs 20 --prepare 'sleep 0.1' \
    --ignore-failure \
    --command-name "pytest" \
    --command-name "rtest" \
    "uv run pytest ${TEST_DIR}" \
    "uv run rtest ${TEST_DIR}"
  
  echo
  echo "=== Test Collection Only Benchmark ==="
  hyperfine --warmup 5 --min-runs 20 --prepare 'sleep 0.1' \
    --ignore-failure \
    --command-name "pytest --collect-only" \
    --command-name "rtest --collect-only" \
    "uv run pytest --collect-only ${TEST_DIR}" \
    "uv run rtest --collect-only ${TEST_DIR}"
else
  echo "=== Full Test Execution Benchmark ==="
  hyperfine --warmup 5 --min-runs 20 --prepare 'sleep 0.1' \
    --ignore-failure \
    --command-name "pytest" \
    --command-name "rtest" \
    "uv run pytest ${TEST_DIR}" \
    "uv run rtest ${TEST_DIR}"
fi
