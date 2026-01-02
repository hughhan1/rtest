# rtest Development Guide

## Critical Rules

**ALWAYS:**
- Use `uv run` for ALL Python commands (`uv run pytest`, `uv run rtest`, `uv run python`)
- Run `git submodule update --init` after cloning (ruff is a git submodule for AST parsing)
- Run formatters before committing: `cargo fmt` and `uv run ruff format python/ tests/ scripts/`
- Use conventional commits: `feat|fix|docs|test|refactor|chore: description`

**NEVER:**
- Run `pytest` or `python` directly without `uv run`
- Use `panic!` or `unwrap()` in Rust library code—use `Result<T, E>` with proper error handling
- Commit without running formatters (CI will fail on `cargo fmt --check` and `ruff format --check`)

## Quick Reference

### Build & Run
```bash
uv run maturin develop              # Build and install (required after Rust changes)
uv run rtest tests/ --collect-only  # Test collection
uv run rtest tests/ -n auto         # Run tests in parallel
```

### Testing
```bash
cargo test                          # Rust unit tests
uv run pytest tests/ -v             # Python integration tests
```

### Linting & Formatting
```bash
# Rust (run before commit)
cargo fmt
cargo clippy --all-targets -- -D warnings

# Python (run before commit)
uv run ruff format python/ tests/ scripts/
uv run ruff check --fix python/ tests/ scripts/
uv run ty check python/ tests/ scripts/
```

### Debugging
```bash
RUST_LOG=debug uv run rtest path/to/test.py --collect-only  # Verbose collection
uv run pytest path/to/test.py --collect-only                # Compare with pytest
```

## Project Structure

```
rtest/
├── src/                        # Rust source
│   ├── lib.rs                  # Library entry + module declarations
│   ├── pyo3.rs                 # PyO3 Python bindings
│   ├── cli.rs                  # CLI argument parsing
│   ├── collection/             # Test collection (Session, Module, Class, Function)
│   │   └── nodes.rs            # Core collector implementations
│   ├── python_discovery/       # AST parsing for test discovery
│   │   ├── discovery.rs        # Test discovery logic
│   │   └── visitor.rs          # AST visitor for test functions
│   ├── runner.rs               # Parallel pytest execution
│   └── native_runner.rs        # Native runner (no pytest dependency)
├── python/rtest/               # Python package
│   ├── __init__.py             # Main API + CLI entry
│   ├── mark.py                 # @rtest.mark.cases, @rtest.mark.skip
│   └── worker/                 # Native runner worker subprocess
├── tests/                      # Python integration tests
└── ruff/                       # Git submodule (Rust crates for AST parsing)
```

## Code Standards

### Rust

| Category | Rule |
|----------|------|
| Errors | Use `Result<T, E>`; add context with `.context()` or `.map_err()`; pattern match for fallbacks |
| Visibility | Use `pub(crate)` for internal APIs; minimize public surface |
| Ownership | Use `Rc`/`Arc` for shared ownership (e.g., `Rc<Session>` in collection); avoid `.clone()` on large types |
| Performance | Prefer iterator chains over intermediate `Vec`s; use `HashMap` for lookups, `BTreeMap` for ordering |
| Strings | Use `&str` params; `.into()` over `.to_string()`; `Cow<str>` when ownership varies |
| Traits | Implement `From`/`Into` for conversions; `Display` for user-facing messages |
| Idioms | Use `#[derive(Debug, Clone, Default)]`; prefer `.is_some_and()`, `.map()`, `.unwrap_or_default()` |
| Testing | Unit tests in same file with `#[cfg(test)]`; integration tests in `tests/` |
| Clippy | Run `cargo clippy`; suppress with `#[expect(clippy::...)]` + justification |

### Python

| Category | Rule |
|----------|------|
| Typing | Type all public interfaces; use `str \| None` not `Optional[str]` |
| Style | ruff formatting, 120 char lines |
| Types | Prefer `@dataclass`, `NamedTuple`, `TypedDict` over `dict[str, Any]` |
| Syntax | Use `list[str]` not `List[str]}}`; use `\|` union syntax |

## Commit Format

| Prefix | Version Bump | Example |
|--------|--------------|---------|
| `feat:` | minor | `feat: add native runner support` |
| `fix:` | patch | `fix: handle empty test files` |
| `feat!:` | major | `feat!: change CLI argument format` |
| `docs:` `test:` `refactor:` `chore:` | none | `chore: update dependencies` |

## Troubleshooting

| Problem | Solution |
|---------|----------|
| Import errors after Rust changes | `uv run maturin develop` |
| Missing rustfmt/clippy | `rustup component add rustfmt clippy` |
| ruff submodule missing | `git submodule update --init --recursive` |
| Stale build | `cargo clean && uv run maturin develop` |
