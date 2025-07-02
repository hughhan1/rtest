#!/usr/bin/env bash

###
# Performance comparison: uv run pytest vs rustic --package-manager uv
# Usage: ./benchmark_pytest.sh [test_directory]
###

TEST_DIR=${1:-"."}

echo "Benchmarking pytest performance comparison..."
echo "Test directory: ${TEST_DIR}"
echo

# Check if both commands exist
if ! command -v uv &> /dev/null; then
    echo "Error: uv not found. Please install uv first."
    exit 1
fi

if [ ! -f "./target/debug/rustic" ]; then
    echo "Error: ./target/debug/rustic not found. Please build rustic first with 'cargo build'."
    exit 1
fi

if [ ! -d "${TEST_DIR}" ]; then
    echo "Error: Test directory '${TEST_DIR}' not found."
    exit 1
fi

hyperfine --warmup 3 --min-runs 5 \
  --ignore-failure \
  --command-name "uv run pytest" \
  --command-name "rustic with uv" \
  "uv run pytest ${TEST_DIR}" \
  "./target/debug/rustic --package-manager uv ${TEST_DIR}"