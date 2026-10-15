"""Integration tests for CLI functionality."""

import subprocess
import sys
import tempfile
from pathlib import Path

import rtest
from rtest.exit_code import ExitCodeValues


class TestCLIBasics:
    """Basic CLI tests."""

    def test_help_shows_usage(self) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--help"],
            capture_output=True,
            text=True,
        )
        assert result.returncode == ExitCodeValues.OK
        assert "Usage:" in result.stdout
        assert "--runner" in result.stdout
        assert "--env" in result.stdout
        assert "-n" in result.stdout

    def test_version_shows_version(self) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--version"],
            capture_output=True,
            text=True,
        )
        assert result.returncode == ExitCodeValues.OK
        assert "rtest" in result.stdout.lower()

    def test_invalid_flag_rejected(self) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--invalid-flag-xyz"],
            capture_output=True,
            text=True,
        )
        assert result.returncode != 0


class TestCLIErrorHandling:
    """Tests for error handling."""

    def test_nonexistent_file(self) -> None:
        """Nonexistent file should return exit code 4 (matching pytest behavior)."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--collect-only", "nonexistent.py"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.USAGE_ERROR
            assert "file or directory not found" in result.stderr

    def test_invalid_dist_mode(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--dist", "invalid_mode_xyz"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode != 0

    @rtest.mark.cases("runner", ["native", "pytest"])
    @rtest.mark.cases("option", ["-n", "--maxprocesses"])
    def test_zero_worker_configuration_rejected(self, runner: str, option: str) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            (root / "test_failure.py").write_text("def test_failure(): assert False\n")
            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", runner, option, "0"],
                cwd=root,
                capture_output=True,
                text=True,
            )
            assert result.returncode != 0
            assert "invalid value" in result.stderr


class TestNativeRunnerEndToEnd:
    """End-to-end tests for native runner."""

    def test_native_runner_basic_flow(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            test_file = tmp_path / "test_example.py"
            test_file.write_text("def test_pass(): assert True\ndef test_fail(): assert False\n")

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.TESTS_FAILED
            assert "1 passed" in result.stdout
            assert "1 failed" in result.stdout
            assert "FAILED" in result.stdout

    def test_native_runner_empty_directory(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.OK
            assert "No tests found" in result.stdout

    def test_native_runner_import_error(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            test_file = tmp_path / "test_bad.py"
            test_file.write_text("import nonexistent_module_xyz_abc\n\ndef test_never_runs(): pass\n")

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.TESTS_FAILED
            assert "ModuleNotFoundError" in result.stdout

    @rtest.mark.cases(
        "source",
        [
            "import os\ndef test_exit(): os._exit(0)\n",
            "from rtest.worker.runner import TestResult\n"
            "TestResult.to_json = lambda self: '{invalid'\n"
            "def test_pass(): pass\n",
        ],
    )
    def test_missing_or_malformed_worker_results_fail_the_run(self, source: str) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            (root / "test_worker.py").write_text(source)
            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native"],
                cwd=root,
                capture_output=True,
                text=True,
            )
            assert result.returncode == ExitCodeValues.TESTS_FAILED
            assert "FAILED" in result.stdout
            assert "1 worker error" in result.stdout


class TestPytestEnvironment:
    """Environment options reach both sequential and parallel pytest processes."""

    @rtest.mark.cases("workers", [1, 2])
    def test_environment_values_and_invalid_option_warning(self, workers: int) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            (tmp_path / "test_env.py").write_text(
                "import os\n"
                "def test_value(): assert os.environ['RTEST_VALUE'] == 'a=b'\n"
                "def test_empty(): assert os.environ['RTEST_EMPTY'] == ''\n"
            )
            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest",
                    "--runner",
                    "pytest",
                    "-n",
                    str(workers),
                    "--env",
                    "RTEST_VALUE=old",
                    "--env",
                    "RTEST_VALUE=a=b",
                    "--env",
                    "RTEST_EMPTY=",
                    "--env",
                    "INVALID_NO_EQUALS",
                ],
                capture_output=True,
                text=True,
                cwd=tmp_path,
            )
            assert result.returncode == ExitCodeValues.OK, result.stdout + result.stderr
            assert result.stderr.count("Warning: Invalid environment variable format: INVALID_NO_EQUALS") == 1
