#!/bin/bash
# Comprehensive quality check script for rtest
set -euo pipefail

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Counters
PASSED=0
FAILED=0

run_check() {
    local name="$1"
    local command="$2"
    local required="${3:-true}"
    
    echo -e "${BLUE}🔍 $name${NC}"
    
    if eval "$command" > /tmp/rtest_check.log 2>&1; then
        echo -e "${GREEN}✅ $name passed${NC}"
        ((PASSED++))
    else
        echo -e "${RED}❌ $name failed${NC}"
        if [ "$required" = "true" ]; then
            echo "Error output:"
            cat /tmp/rtest_check.log
            ((FAILED++))
        else
            echo -e "${YELLOW}⚠️  Optional check failed${NC}"
        fi
    fi
    echo ""
}

echo "🦀 Running comprehensive quality checks for rtest..."
echo ""

# Rust checks
echo -e "${BLUE}=== Rust Quality Checks ===${NC}"
run_check "Rust compilation" "cargo check --all-targets"
run_check "Rust formatting" "cargo fmt --check"
run_check "Rust linting" "cargo clippy --all-targets -- -D warnings"
run_check "Rust tests" "cargo test"
run_check "Rust core tests" "cd rtest && cargo test"

# Python checks  
echo -e "${BLUE}=== Python Quality Checks ===${NC}"
run_check "Python formatting" "uv run ruff format --check ."
run_check "Python linting" "uv run ruff check ."
run_check "Python type checking" "uv run mypy python/rtest/"
run_check "Python tests" "uv run pytest tests/ --tb=short"

# Build checks
echo -e "${BLUE}=== Build Quality Checks ===${NC}"
run_check "Development build" "uv run maturin develop"
run_check "Release build" "uv run maturin build --release" "false"

# Integration checks
echo -e "${BLUE}=== Integration Checks ===${NC}"
run_check "CLI help" "cargo run -- --help"
run_check "CLI version" "cargo run -- --version"
run_check "CLI collect-only" "cargo run -- --collect-only" "false"

# Performance checks (optional)
echo -e "${BLUE}=== Performance Checks ===${NC}"
if [ -f "./benchmark.sh" ]; then
    run_check "Performance benchmark" "./benchmark.sh" "false"
else
    echo -e "${YELLOW}⚠️  No benchmark.sh found, skipping performance checks${NC}"
fi

# Documentation checks
echo -e "${BLUE}=== Documentation Checks ===${NC}"
run_check "Rust documentation" "cargo doc --no-deps" "false"
run_check "README links" "grep -E 'https?://[^)]+' README.md | head -5" "false"

# Security checks (optional)
echo -e "${BLUE}=== Security Checks ===${NC}"
if command -v cargo-audit &> /dev/null; then
    run_check "Security audit" "cargo audit" "false"
else
    echo -e "${YELLOW}⚠️  cargo-audit not installed, skipping security check${NC}"
    echo "   Install with: cargo install cargo-audit"
fi

# Summary
echo -e "${BLUE}=== Summary ===${NC}"
if [ $FAILED -eq 0 ]; then
    echo -e "${GREEN}🎉 All required checks passed! ($PASSED passed, $FAILED failed)${NC}"
    echo "✅ Code is ready for commit/push"
    exit 0
else
    echo -e "${RED}💥 Some checks failed ($PASSED passed, $FAILED failed)${NC}"
    echo "❌ Please fix issues before committing"
    exit 1
fi