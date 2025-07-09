#!/bin/bash
# Release preparation script for rtest
set -euo pipefail

# Colors
GREEN='\033[0;32m'
BLUE='\033[0;34m'
YELLOW='\033[1;33m'
RED='\033[0;31m'
NC='\033[0m'

# Get version from arguments or prompt
VERSION=${1:-}
if [ -z "$VERSION" ]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.1.0"
    exit 1
fi

# Validate version format
if ! [[ $VERSION =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[a-zA-Z0-9]+)?$ ]]; then
    echo -e "${RED}❌ Invalid version format. Use semver (e.g., 1.0.0 or 1.0.0-beta.1)${NC}"
    exit 1
fi

echo -e "${BLUE}🚀 Preparing release $VERSION${NC}"

# Check if on main branch
CURRENT_BRANCH=$(git branch --show-current)
if [ "$CURRENT_BRANCH" != "main" ]; then
    echo -e "${YELLOW}⚠️  Currently on branch '$CURRENT_BRANCH', not 'main'${NC}"
    read -p "Continue anyway? (y/N): " -n 1 -r
    echo
    if [[ ! $REPLY =~ ^[Yy]$ ]]; then
        exit 1
    fi
fi

# Check for uncommitted changes
if ! git diff-index --quiet HEAD --; then
    echo -e "${RED}❌ Uncommitted changes detected. Please commit or stash them.${NC}"
    git status --short
    exit 1
fi

# Run quality checks
echo -e "${BLUE}🔍 Running quality checks...${NC}"
if ! ./.claude/scripts/quality-check.sh; then
    echo -e "${RED}❌ Quality checks failed. Please fix issues before release.${NC}"
    exit 1
fi

# Update version in files
echo -e "${BLUE}📝 Updating version to $VERSION${NC}"

# Update Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" Cargo.toml
rm Cargo.toml.bak

# Update rtest/Cargo.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" rtest/Cargo.toml
rm rtest/Cargo.toml.bak

# Update pyproject.toml
sed -i.bak "s/^version = \".*\"/version = \"$VERSION\"/" pyproject.toml
rm pyproject.toml.bak

# Update Cargo.lock
cargo check --quiet

echo -e "${GREEN}✅ Version updated in all files${NC}"

# Build and test with new version
echo -e "${BLUE}🔨 Building release version...${NC}"
maturin build --release

echo -e "${BLUE}🧪 Testing release build...${NC}"
# Test installation in temporary directory
TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"
python -m venv test_env
source test_env/bin/activate
pip install --quiet "$(dirname "$0")"/../../dist/rtest-*.whl

# Quick smoke test
echo "import rtest; print('✅ Import successful')" | python
rtest --version | grep -q "$VERSION" && echo -e "${GREEN}✅ CLI version correct${NC}"

cd - > /dev/null
rm -rf "$TEMP_DIR"

# Generate changelog entry template
CHANGELOG_FILE="CHANGELOG.md"
if [ ! -f "$CHANGELOG_FILE" ]; then
    echo "# Changelog" > "$CHANGELOG_FILE"
    echo "" >> "$CHANGELOG_FILE"
    echo "All notable changes to this project will be documented in this file." >> "$CHANGELOG_FILE"
    echo "" >> "$CHANGELOG_FILE"
fi

# Get commits since last tag
LAST_TAG=$(git describe --tags --abbrev=0 2>/dev/null || echo "")
if [ -n "$LAST_TAG" ]; then
    COMMITS=$(git log --oneline "$LAST_TAG"..HEAD --grep="^feat\|^fix\|^perf\|^BREAKING")
else
    COMMITS=$(git log --oneline --grep="^feat\|^fix\|^perf\|^BREAKING")
fi

echo -e "${BLUE}📋 Changelog template generated${NC}"
echo ""
echo "## [$VERSION] - $(date +%Y-%m-%d)"
echo ""
if [ -n "$COMMITS" ]; then
    echo "### Changes"
    echo "$COMMITS" | sed 's/^/- /'
else
    echo "### Changes"
    echo "- Initial release"
fi
echo ""
echo -e "${YELLOW}⚠️  Please update CHANGELOG.md with proper release notes${NC}"

# Commit version changes
echo -e "${BLUE}💾 Committing version changes...${NC}"
git add Cargo.toml rtest/Cargo.toml pyproject.toml Cargo.lock
git commit -m "chore: bump version to $VERSION"

echo -e "${GREEN}🎉 Release preparation complete!${NC}"
echo ""
echo "Next steps:"
echo "1. Update CHANGELOG.md with release notes"
echo "2. Review and test the changes"
echo "3. Create release PR or tag:"
echo "   git tag v$VERSION"
echo "   git push origin v$VERSION"
echo "4. GitHub Actions will handle the rest"
echo ""
echo "Build artifacts available in dist/"
ls -la dist/rtest-*.whl