#!/bin/bash
# Development environment setup script for rtest
set -euo pipefail

echo "🦀 Setting up rtest development environment..."

# Check prerequisites
echo "📋 Checking prerequisites..."

# Check Rust installation
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust is not installed. Please install from https://rustup.rs/"
    exit 1
fi

# Check Python installation
if ! command -v python3 &> /dev/null; then
    echo "❌ Python 3 is not installed. Please install Python 3.9+"
    exit 1
fi

# Check Python version
PYTHON_VERSION=$(python3 -c "import sys; print(f'{sys.version_info.major}.{sys.version_info.minor}')")
if ! python3 -c "import sys; sys.exit(0 if sys.version_info >= (3, 9) else 1)"; then
    echo "❌ Python 3.9+ is required. Found: $PYTHON_VERSION"
    exit 1
fi

echo "✅ Rust: $(rustc --version)"
echo "✅ Python: $PYTHON_VERSION"

# Install/update Rust components
echo "🔧 Installing Rust components..."
rustup component add clippy rustfmt
rustup update

# Initialize git submodules (required for ruff dependency)
echo "📦 Initializing git submodules..."
git submodule update --init --recursive

# Install Python dependencies via uv
echo "🐍 Installing Python dependencies via uv..."
uv sync --dev

# Install git hooks if available
if [ -f "scripts/install-git-hooks.sh" ]; then
    echo "🪝 Installing git hooks..."
    bash scripts/install-git-hooks.sh
fi

# Build the project via uv
echo "🔨 Building project..."
uv run maturin develop

# Run tests to verify setup
echo "🧪 Running tests to verify setup..."
echo "Running Rust tests..."
cargo test --quiet

echo "Running Python tests..."
uv run pytest tests/ --tb=short -q

# Run quality checks
echo "🔍 Running quality checks..."
cargo clippy --quiet -- -D warnings
cargo fmt --check
uv run ruff check .
uv run ruff format --check .

echo "✅ Development environment setup complete!"
echo ""
echo "📚 Quick start commands:"
echo "  uv run maturin develop       # Rebuild Python bindings"
echo "  cargo test                   # Run Rust tests"
echo "  uv run pytest                # Run Python tests"
echo "  cargo clippy                 # Lint Rust code"
echo "  uv run ruff check .          # Lint Python code"
echo ""
echo "📖 See .claude/CLAUDE.md for detailed development guide"