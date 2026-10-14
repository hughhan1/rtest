"""Regressions for benchmark failures that previously produced green CI jobs."""

import subprocess
import tempfile
from pathlib import Path
from unittest.mock import patch

import rtest
from scripts.benchmark.benchmark_repositories import Repository, benchmark_repository, checkout_repository


@rtest.mark.cases(
    "stdout, returncode",
    [("collected 1 item\n", 1), ("No tests collected.\n", 0)],
)
def test_failed_or_empty_collection_is_not_timed(stdout: str, returncode: int) -> None:
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory)
        result = subprocess.CompletedProcess(["rtest"], returncode, stdout, "diagnostic")
        with patch("scripts.benchmark.benchmark_repositories.subprocess.run", return_value=result) as run:
            with rtest.raises((subprocess.CalledProcessError, ValueError)):
                benchmark_repository("example", output, output)
        assert run.call_count == 1
        assert (output / "example-collection.txt").read_text() == stdout + "diagnostic"


def test_timing_failure_propagates_after_successful_collection() -> None:
    collection = subprocess.CompletedProcess(["rtest"], 0, "collected 1 item\n", "")
    failure = subprocess.CalledProcessError(1, ["hyperfine"])
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory)
        with patch("scripts.benchmark.benchmark_repositories.subprocess.run", side_effect=[collection, failure]):
            with rtest.raises(subprocess.CalledProcessError) as error:
                benchmark_repository("example", output, output)
        assert error.value is failure


def test_checkout_failure_is_not_skipped() -> None:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)
        repository = Repository(str(root / "missing-repository"), "a" * 40)
        with rtest.raises(subprocess.CalledProcessError):
            checkout_repository(repository, root / "checkout")


def test_collection_warnings_do_not_reject_a_nonempty_workload() -> None:
    with tempfile.TemporaryDirectory() as directory:
        output = Path(directory)
        (output / "example.md").write_text("Hyperfine timing table\n", encoding="utf-8")
        collection = subprocess.CompletedProcess(["rtest"], 0, "collected 639 items / 1 warning\n", "")
        timing = subprocess.CompletedProcess(["hyperfine"], 0)
        with patch("scripts.benchmark.benchmark_repositories.subprocess.run", side_effect=[collection, timing]):
            report = benchmark_repository("example", output, output)
        assert "Collected 639 items." in report
        assert "Hyperfine timing table" in report
