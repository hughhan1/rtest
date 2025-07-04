#!/bin/bash
# Test script to verify Claude configuration workflows
set -euo pipefail

echo "🧪 Testing Claude Configuration Workflows"
echo "========================================="

# Test 1: Check submodules are initialized
echo "📦 Test 1: Checking git submodules..."
if [ -d "ruff/.git" ] || [ -f "ruff/.git" ]; then
    echo "✅ Git submodules are initialized"
else
    echo "❌ Git submodules not initialized"
    echo "   Run: git submodule update --init --recursive"
    exit 1
fi

# Test 2: Check uv is available
echo "🐍 Test 2: Checking uv availability..."
if command -v uv &> /dev/null; then
    echo "✅ uv is available: $(uv --version)"
else
    echo "❌ uv is not installed"
    echo "   Install from: https://docs.astral.sh/uv/"
    exit 1
fi

# Test 3: Check Rust compilation
echo "🦀 Test 3: Checking Rust compilation..."
if cargo check --quiet; then
    echo "✅ Rust compilation successful"
else
    echo "❌ Rust compilation failed"
    echo "   Check Cargo.toml and dependencies"
    exit 1
fi

# Test 4: Check Python dev dependencies
echo "🔧 Test 4: Checking Python dev dependencies..."
if uv run ruff --version &> /dev/null; then
    echo "✅ Python dev tools available: $(uv run ruff --version)"
else
    echo "❌ Python dev tools not available"
    echo "   Run: uv sync --dev"
    exit 1
fi

# Test 5: Check if we can run Python linting
echo "📝 Test 5: Testing Python linting..."
if uv run ruff check python/ --quiet; then
    echo "✅ Python linting works"
else
    echo "⚠️  Python linting found issues (expected for unlinted code)"
fi

# Test 6: Check if we can run Python tests
echo "🧪 Test 6: Testing Python test runner..."
if uv run pytest --version &> /dev/null; then
    echo "✅ pytest is available via uv: $(uv run pytest --version)"
else
    echo "❌ pytest not available via uv"
    exit 1
fi

echo ""
echo "🎉 All workflow tests passed!"
echo "Claude configuration is working correctly."
echo ""
echo "📚 Next steps:"
echo "  - Run: ./.claude/scripts/dev-setup.sh"
echo "  - Run: ./.claude/scripts/quality-check.sh"
echo "  - Use: uv run <python-command> for all Python tools"