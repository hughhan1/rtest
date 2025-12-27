# Claude Code Instructions for rtest

See **[AGENTS.md](../AGENTS.md)** for all development instructions including setup, building, testing, linting, and project architecture.

## Quick Reference

```bash
# Build and test
uv run maturin develop && cargo test && uv run pytest tests/ -v

# Quality checks
cargo fmt && cargo clippy && uv run ruff check python/ tests/
```

## Key Guidelines

- Use safe Rust patterns (`Result`, `Rc`), no `unsafe` blocks
- Include tests for new functionality
- Make small, atomic changes
- This is performance-critical CI/CD tooling - correctness is paramount
