# Claude AI Assistant Configuration for rtest

This file contains Claude-specific guidance for working on the rtest project. For general development instructions,
testing, building, linting, and project architecture, see **[AGENTS.md](../AGENTS.md)** at the repository root.

## 📁 Configuration Structure

```plaintext
.claude/
├── CLAUDE.md              # This file - Claude-specific guidance
├── settings.json          # Project configuration for Claude
├── scripts/               # Development automation scripts
│   ├── dev-setup.sh       # Environment setup
│   ├── quality-check.sh   # Comprehensive quality checks
│   ├── release-prep.sh    # Release preparation automation
│   └── test-workflows.sh  # Verify configuration works
└── templates/             # Code templates for consistency
    └── rust_module.rs     # Rust module template
```

## 🚀 Quick Start for Claude

### Essential Commands

```bash
# Complete development cycle
./.claude/scripts/quality-check.sh

# Fast iteration
uv run maturin develop && cargo test && uv run pytest tests/

# See AGENTS.md for detailed development workflow
```

## 🧠 Claude-Specific Context

When working on this codebase as Claude:

1. **Safety First**: Always use safe Rust patterns, no raw pointers or unsafe blocks
2. **Test Coverage**: Include tests for any new functionality
3. **Error Handling**: Use proper `Result` types, don't panic in library code
4. **Performance Aware**: Consider performance implications of changes
5. **Documentation**: Update docs and examples when changing APIs
6. **Incremental**: Make small, atomic changes that can be easily reviewed
7. **Quality**: Run the full quality check script before suggesting changes
8. **Refer to AGENTS.md**: For project architecture, technical decisions, development patterns, and all development
   workflow details

Remember: This is a performance-critical tool used in CI/CD pipelines. Correctness and reliability are paramount.

## 📖 Documentation Structure

The documentation hierarchy is:

- **[AGENTS.md](../AGENTS.md)** → All developers and AI agents (setup, testing, building, linting, architecture,
  patterns)
- **[README.md](../README.md)** → Users (installation and usage)
- **[CONTRIBUTING.rst](../CONTRIBUTING.rst)** → Contributors (how to contribute)
- **[.claude/CLAUDE.md](CLAUDE.md)** → Claude AI (Claude-specific automation and workflow)

## 📋 Quick Reference to AGENTS.md

For detailed instructions on:

- **Project Overview** → [AGENTS.md - Project Overview](../AGENTS.md#project-overview)
- **Initial Setup** → [AGENTS.md - Initial Setup](../AGENTS.md#initial-setup)
- **Building** → [AGENTS.md - Building the Project](../AGENTS.md#building-the-project)
- **Testing** → [AGENTS.md - Testing](../AGENTS.md#testing)
- **Code Quality** → [AGENTS.md - Code Quality](../AGENTS.md#code-quality)
- **Project Structure** → [AGENTS.md - Project Structure](../AGENTS.md#project-structure)
- **Technical Decisions** → [AGENTS.md - Key Technical Decisions](../AGENTS.md#key-technical-decisions)
- **Development Patterns** → [AGENTS.md - Common Development Patterns](../AGENTS.md#common-development-patterns)
- **Testing Philosophy** → [AGENTS.md - Testing Philosophy](../AGENTS.md#testing-philosophy)
- **Performance** → [AGENTS.md - Performance Considerations](../AGENTS.md#performance-considerations)
- **File Templates** → [AGENTS.md - File Templates](../AGENTS.md#file-templates)
- **CI/CD** → [AGENTS.md - CI/CD Configuration](../AGENTS.md#cicd-configuration)
- **Debugging** → [AGENTS.md - Debugging](../AGENTS.md#debugging-collection-issues)
- **Troubleshooting** → [AGENTS.md - Troubleshooting](../AGENTS.md#troubleshooting)

**Built for high-velocity, high-quality development of performance-critical testing tools.**
