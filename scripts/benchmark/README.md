# Static collection benchmarks

The benchmark measures `rtest --collect-only tests` on pinned Click, Flask, and Pydantic source trees. These cover
smaller CLI and web projects and a larger, heavily parametrized suite. Source commits are recorded in
`benchmark_repositories.py` and each run's summary. No upstream packages or test dependencies are installed:
static collection does not import or execute the upstream tests.

## Run locally

Install Git, uv, Rust, and Hyperfine 1.20.0 (the version pinned in CI), then run from the project root:

```bash
git submodule update --init
uv sync --locked --no-install-project
uv run --no-sync maturin develop --release
uv run --no-sync python scripts/benchmark/benchmark_repositories.py

# Select a workload and write to a fresh output directory.
uv run --no-sync python scripts/benchmark/benchmark_repositories.py --repositories click --output dist/click-benchmark
```

The script uses the same Python environment that launched it, so it measures the installed release build.
Each workload gets 3 warmups and 20 measured runs, including CLI startup. Git setup and the initial collection
check are outside the timing. Run on an idle machine for more stable results.

The output directory must be new, to avoid mixing runs. It contains:

- `summary.md`: timings, Python/platform details, source revisions, and collected item counts.
- `<repository>-collection.txt`: the initial collection output, including test IDs.
- `<repository>.json` and `<repository>.md`: unmodified Hyperfine reports.

Any checkout error, failed or empty collection, timeout, or failed timing command fails the job.
Temporary checkouts are removed automatically. CI publishes the summary on success and uploads available
reports even on failure, so collection diagnostics can be inspected.

## Scope

These are static collection timings, not tests of upstream execution or pytest compatibility. Comparing them
with pytest collection would compare different work: pytest imports modules, loads plugins, and evaluates
fixtures and dynamic parameters. Execution comparisons against unsupported fixture-heavy suites and repeated
`--version` benchmarks have been removed.

There is no comparison against an old main-branch artifact or percentage-based regression gate. Different
hosted runners are not a controlled experiment. Use the raw measurements to investigate performance, and
compare builds on the same machine with the same pinned workloads and collected test IDs.

Hyperfine generates the timing tables itself. The harness does not depend on the JSON statistics schema,
which changed in Hyperfine 2.0.
