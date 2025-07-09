#!/bin/bash

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"

# Handle git worktrees
if [ -f "$PROJECT_ROOT/.git" ]; then
    # This is a worktree, extract the actual git directory
    GIT_DIR=$(cat "$PROJECT_ROOT/.git" | sed 's/gitdir: //')
    HOOKS_DIR="$GIT_DIR/hooks"
else
    HOOKS_DIR="$PROJECT_ROOT/.git/hooks"
fi

echo "Installing Git hooks..."

# Create hooks directory if it doesn't exist
mkdir -p "$HOOKS_DIR"

# Copy hooks from this script inline
cat > "$HOOKS_DIR/pre-commit" << 'EOF'
#!/bin/bash
# Pre-commit hook for Rust projects

set -e

echo "Running pre-commit checks..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

print_status() {
    echo -e "${GREEN}[✓]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[!]${NC} $1"
}

print_error() {
    echo -e "${RED}[✗]${NC} $1"
}

# Check if we have any Rust files to check
RUST_FILES=$(git diff --cached --name-only | grep '\.rs$' || true)
# Check if we have any Python files to check
PYTHON_FILES=$(git diff --cached --name-only | grep '\.py$' || true)

if [ -z "$RUST_FILES" ] && [ -z "$PYTHON_FILES" ]; then
    exit 0
fi

if [ -n "$RUST_FILES" ]; then
    print_status "Checking Rust code..."

    echo "Formatting code..."
    cargo fmt --all
    print_status "Code formatted"

    echo "Applying clippy fixes..."
    # Run clippy on the workspace (both rtest-py and rtest crates)
    if cargo clippy --workspace --fix --allow-staged -- -D warnings; then
        print_status "Clippy fixes applied"
    else
        print_warning "Some clippy issues couldn't be auto-fixed"
    fi

    # Re-stage any files that were modified by formatting/clippy
    git add -u
fi

if [ -n "$PYTHON_FILES" ]; then
    print_status "Checking Python code..."
    
    echo "Running ruff format on python/, tests/, and scripts/ directories..."
    if ! uv run ruff format python/ tests/ scripts/; then
        print_error "Failed to format Python code."
        exit 1
    fi
    
    echo "Running ruff format on staged files..."
    if ! uv run ruff format $PYTHON_FILES; then
        print_error "Failed to format Python code."
        exit 1
    fi
    
    echo "Running ruff linter with auto-fix..."
    if ! uv run ruff check --fix $PYTHON_FILES; then
        print_error "Ruff found unfixable issues. Fix them manually before committing."
        exit 1
    fi
    
    echo "Running vulture to check for dead code..."
    if ! uv run vulture python/ tests/ scripts/ --min-confidence 80; then
        print_warning "Vulture found potential dead code. Review the findings above."
        # Don't fail the commit for dead code warnings, just warn
    fi
    
    # Re-add any files that were modified by ruff
    git add $PYTHON_FILES
fi

print_status "All pre-commit checks passed!"
echo ""
EOF

cat > "$HOOKS_DIR/commit-msg" << 'EOF'
#!/bin/bash
# Commit message hook for Rust projects

commit_regex='^(feat|fix|docs|style|refactor|test|chore|perf|ci|revert)(\(.+\))?: .{1,50}'

if ! grep -qE "$commit_regex" "$1"; then
    echo ""
    echo "Invalid commit message format!"
    echo "Format: <type>(<scope>): <description>"
    echo ""
    echo "Examples:"
    echo "  feat(parser): add support for async functions"
    echo "  fix(linter): resolve false positive in rule detection"
    echo "  docs: update installation guide"
    echo ""
    exit 1
fi
EOF

# Make hooks executable
chmod +x "$HOOKS_DIR/pre-commit"
chmod +x "$HOOKS_DIR/commit-msg"

echo "✅ Git hooks installed successfully!"

echo ""
echo "To bypass hooks temporarily, use:"
echo "  git commit --no-verify"
echo ""
