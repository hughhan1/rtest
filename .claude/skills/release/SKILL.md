---
name: release
description: This skill should be used when the user asks to "release", "publish", "bump version", "create a release", "prepare release", or wants to publish a new version to PyPI.
version: 1.0.0
---

# Release Skill

This skill guides the release process for rtest, ensuring all version files are updated consistently.

## When This Skill Applies

Use this skill when the user wants to:
- Publish a new version to PyPI
- Bump the version number
- Create a release
- Prepare a release commit and tag

## Release Process

### Step 1: Determine Version Number

Ask the user for the new version number if not provided. The current version can be found in `pyproject.toml`.

For semantic versioning:
- **patch** (0.0.X): Bug fixes, minor changes
- **minor** (0.X.0): New features, backwards compatible
- **major** (X.0.0): Breaking changes

### Step 2: Update Version Files

Update the version in **both** files (they must stay in sync):

1. **pyproject.toml** - Update `version = "X.X.X"` under `[project]`
2. **Cargo.toml** - Update `version = "X.X.X"` under `[package]`

### Step 3: Update CHANGELOG.md

Add a new section after `## [Unreleased]`:

```markdown
## [X.X.X] - YYYY-MM-DD

### Added/Changed/Fixed
- Description of changes
```

Ask the user for a summary of changes if not provided.

### Step 4: Create Release Commit

Stage and commit all changes:
```bash
git add pyproject.toml Cargo.toml CHANGELOG.md
git commit -m "chore: release vX.X.X"
```

### Step 5: Create and Push Tag

```bash
git tag vX.X.X
git push origin main --tags
```

## Important Notes

- Always update BOTH pyproject.toml AND Cargo.toml - missing one will cause the PyPI release to fail with "file already exists" error
- The tag push triggers the GitHub Actions release workflow which builds and publishes to PyPI
- Use conventional commit format: `chore: release vX.X.X`
