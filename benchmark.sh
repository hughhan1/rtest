#!/usr/bin/env bash

###
# Benchmarking script for rtest vs pytest
# Usage: ./benchmark.sh [options] [test_directory]
###

set -e

# Default values
REPOSITORIES=""
BENCHMARKS=""
SKIP_SETUP=false
OUTPUT_DIR=""
CONFIG_FILE="repositories.yml"
LOCAL_MODE=false
TEST_DIR="."

# Show usage
usage() {
    cat << EOF
Benchmarking script for rtest vs pytest

Usage: $0 [OPTIONS] [test_directory]

Local Mode (default):
    $0 [test_directory]        # Basic benchmark
    $0 --collect-only          # Test collection only
    $0 --all                   # Full execution + collection

Repository Mode:
    $0 -r REPOS -b BENCHMARKS # Benchmark repositories
    $0 --list-repos            # List available repositories
    $0 --list-benchmarks       # List available benchmarks

OPTIONS:
    -r, --repositories REPOS   Comma-separated repositories (fastapi,flask,etc)
    -b, --benchmarks BENCH     Benchmark types (collect_only,quick_run,etc)
    -s, --skip-setup           Skip repository setup
    -o, --output-dir DIR       Output directory
    -c, --config FILE          Config file (default: repositories.yml)
    -h, --help                 Show this help
EOF
}

# Parse command line arguments
while [[ $# -gt 0 ]]; do
    case $1 in
        -r|--repositories)
            REPOSITORIES="$2"
            shift 2
            ;;
        -b|--benchmarks)
            BENCHMARKS="$2"
            shift 2
            ;;
        -s|--skip-setup)
            SKIP_SETUP=true
            shift
            ;;
        -o|--output-dir)
            OUTPUT_DIR="$2"
            shift 2
            ;;
        -c|--config)
            CONFIG_FILE="$2"
            shift 2
            ;;
        --collect-only)
            LOCAL_MODE=true
            COLLECT_ONLY=true
            shift
            ;;
        --all)
            LOCAL_MODE=true
            ALL=true
            shift
            ;;
        --list-repos)
            [ -f "benchmark_repositories.py" ] && uv run python benchmark_repositories.py --list-repos || echo "benchmark_repositories.py not found"
            exit 0
            ;;
        --list-benchmarks)
            [ -f "benchmark_repositories.py" ] && uv run python benchmark_repositories.py --list-benchmarks || echo "benchmark_repositories.py not found"
            exit 0
            ;;
        -h|--help)
            usage
            exit 0
            ;;
        *)
            if [ -z "$REPOSITORIES" ]; then
                TEST_DIR="$1"
                LOCAL_MODE=true
            else
                echo "Unknown option: $1" >&2
                usage
                exit 1
            fi
            shift
            ;;
    esac
done

# Check for mutually exclusive flags
if [[ "$COLLECT_ONLY" == true && "$ALL" == true ]]; then
    echo "Error: --collect-only and --all are mutually exclusive" >&2
    exit 1
fi

# Auto-detect mode
[ -z "$REPOSITORIES" ] && LOCAL_MODE=true

# Local mode (original benchmark.sh behavior)
run_local_benchmark() {
    echo "Running local benchmark on: ${TEST_DIR}"
    
    if ! command -v uv &> /dev/null; then
        echo "Error: uv not found. Please install uv first." >&2
        exit 1
    fi

    if [ ! -d "${TEST_DIR}" ]; then
        echo "Error: Test directory '${TEST_DIR}' not found." >&2
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
}

# Repository mode
run_repository_benchmark() {
    echo "Running repository benchmark mode..."
    
    if [ ! -f "benchmark_repositories.py" ]; then
        echo "Error: benchmark_repositories.py not found" >&2
        exit 1
    fi
    
    # Build command arguments
    args=("--config" "$CONFIG_FILE")
    [ -n "$OUTPUT_DIR" ] && args+=("--output-dir" "$OUTPUT_DIR")
    [ "$SKIP_SETUP" = true ] && args+=("--skip-setup")
    
    if [ -n "$REPOSITORIES" ]; then
        repos_list=$(echo "$REPOSITORIES" | tr ',' ' ')
        args+=("--repositories" $repos_list)
    fi
    
    if [ -n "$BENCHMARKS" ]; then
        benchmarks_list=$(echo "$BENCHMARKS" | tr ',' ' ')
        args+=("--benchmarks" $benchmarks_list)
    fi
    
    # Add timestamped results file
    timestamp=$(date +"%Y%m%d_%H%M%S")
    args+=("--save-results" "benchmark_results_$timestamp.json")
    
    # Run the benchmark
    uv run python benchmark_repositories.py "${args[@]}"
}

# Main execution
if [ "$LOCAL_MODE" = true ]; then
    run_local_benchmark
else
    run_repository_benchmark
fi
