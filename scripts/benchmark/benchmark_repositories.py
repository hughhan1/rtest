"""Measure rtest's static collection on fixed upstream source trees."""

import argparse
import platform
import re
import shlex
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Repository:
    url: str
    revision: str


REPOSITORIES = {
    # Click 8.1.8, Flask 3.1.0, and Pydantic 2.10.6. Pin commits, not moving tags.
    "click": Repository("https://github.com/pallets/click.git", "934813e4d421071a1b3db3973c02fe2721359a6e"),
    "flask": Repository("https://github.com/pallets/flask.git", "ab8149664182b662453a563161aa89013c806dc9"),
    "pydantic": Repository("https://github.com/pydantic/pydantic.git", "df05e69a8a3fb37628a0e3a33518ca0425334bc9"),
}


def checkout_repository(repository: Repository, destination: Path) -> None:
    """Fetch only the pinned revision; let Git failures fail the benchmark."""
    subprocess.run(["git", "init", "--quiet", str(destination)], check=True, timeout=30)
    for command in (
        ["git", "fetch", "--depth", "1", repository.url, repository.revision],
        ["git", "checkout", "--quiet", "--detach", "FETCH_HEAD"],
        ["git", "submodule", "update", "--init"],
    ):
        subprocess.run(command, cwd=destination, check=True, timeout=300)


def benchmark_repository(name: str, checkout: Path, output: Path) -> str:
    """Require a nonempty collection, then preserve Hyperfine's own reports."""
    command = [sys.executable, "-m", "rtest", "--collect-only", "tests"]
    collection = subprocess.run(command, cwd=checkout, capture_output=True, text=True, timeout=300)
    (output / f"{name}-collection.txt").write_text(collection.stdout + collection.stderr, encoding="utf-8")
    collection.check_returncode()
    match = re.search(r"^collected ([1-9][0-9]*) items?\b", collection.stdout, re.MULTILINE)
    if match is None:
        raise ValueError(f"{name}: expected a nonempty collection; see {output / f'{name}-collection.txt'}")

    report = output / f"{name}.md"
    subprocess.run(
        [
            "hyperfine",
            "--warmup",
            "3",
            "--runs",
            "20",
            "--shell=none",
            "--export-json",
            str(output / f"{name}.json"),
            "--export-markdown",
            str(report),
            "--command-name",
            f"rtest collection: {name}",
            shlex.join(command),
        ],
        cwd=checkout,
        check=True,
        timeout=300,
    )
    return f"## {name}\n\nCollected {match[1]} items.\n\n{report.read_text(encoding='utf-8')}\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repositories", nargs="+", choices=REPOSITORIES, default=list(REPOSITORIES))
    parser.add_argument(
        "--output", type=Path, default=Path("dist/benchmarks"), help="New directory for benchmark reports"
    )
    args = parser.parse_args()
    output = args.output.resolve()
    # Refuse to mix this run with stale reports from an earlier run.
    output.mkdir(parents=True)

    summary = [
        "# Static collection benchmarks\n",
        f"Python {platform.python_version()} on {platform.platform()}.\n",
        "20 runs after 3 warmups per repository. Timings include CLI startup.\n",
        "These are measurements of static discovery, not pytest equivalence or cross-run regression checks.\n",
    ]
    with tempfile.TemporaryDirectory(prefix="rtest-benchmarks-") as directory:
        for name in args.repositories:
            repository = REPOSITORIES[name]
            checkout = Path(directory) / name
            checkout_repository(repository, checkout)
            summary.append(benchmark_repository(name, checkout, output))
            summary.append(f"Source: {repository.url} at `{repository.revision}`.\n")

    (output / "summary.md").write_text("\n".join(summary), encoding="utf-8")
    print(f"Reports saved to {output}")


if __name__ == "__main__":
    main()
