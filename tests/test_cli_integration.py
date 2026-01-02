"""Integration tests for CLI functionality."""

import subprocess
import sys
from pathlib import Path


class TestCLIBasics:
    """Basic CLI tests."""

    def test_help_shows_usage(self) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--help"],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
        assert "Usage" in result.stdout or "usage" in result.stdout
        assert "--runner" in result.stdout
        assert "--env" in result.stdout
        assert "-n" in result.stdout or "--numprocesses" in result.stdout

    def test_version_shows_version(self) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--version"],
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0
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

    def test_nonexistent_file(self, tmp_path: Path) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--collect-only", "nonexistent.py"],
            capture_output=True,
            text=True,
            cwd=str(tmp_path),
        )
        combined = result.stdout + result.stderr
        assert "No tests" in combined or "not found" in combined or result.returncode == 0

    def test_invalid_dist_mode(self, tmp_path: Path) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--dist", "invalid_mode_xyz"],
            capture_output=True,
            text=True,
            cwd=str(tmp_path),
        )
        assert result.returncode != 0


class TestNativeRunnerEndToEnd:
    """End-to-end tests for native runner."""

    def test_native_runner_basic_flow(self, tmp_path: Path) -> None:
        test_file = tmp_path / "test_example.py"
        test_file.write_text("def test_pass(): assert True\ndef test_fail(): assert False\n")

        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
            capture_output=True,
            text=True,
            cwd=str(tmp_path),
        )
        assert result.returncode == 1
        assert "1 passed" in result.stdout
        assert "1 failed" in result.stdout

    def test_native_runner_empty_directory(self, tmp_path: Path) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--runner", "native"],
            capture_output=True,
            text=True,
            cwd=str(tmp_path),
        )
        assert result.returncode == 0
        assert "No tests" in result.stdout

    def test_native_runner_import_error(self, tmp_path: Path) -> None:
        test_file = tmp_path / "test_bad.py"
        test_file.write_text("import nonexistent_module_xyz_abc\n\ndef test_never_runs(): pass\n")

        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
            capture_output=True,
            text=True,
            cwd=str(tmp_path),
        )
        assert result.returncode == 1
        combined = result.stdout + result.stderr
        assert "error" in combined.lower()
