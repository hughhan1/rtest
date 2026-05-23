"""Integration tests for CLI functionality."""

import subprocess
import sys
import tempfile
from pathlib import Path

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

    def test_help_shows_selection_flags(self) -> None:
        result = subprocess.run(
            [sys.executable, "-m", "rtest", "--help"],
            capture_output=True,
            text=True,
        )
        assert result.returncode == ExitCodeValues.OK
        assert "-k" in result.stdout
        assert "--lf" in result.stdout
        assert "--ff" in result.stdout


class TestKeywordSelection:
    """Tests for -k expression filtering."""

    def test_collect_only_keyword_filter(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            (tmp_path / "test_alpha.py").write_text("def test_one(): pass\n")
            (tmp_path / "test_beta.py").write_text("def test_two(): pass\n")

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest",
                    "--collect-only",
                    "-k",
                    "alpha",
                ],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.OK
            assert "test_alpha.py::test_one" in result.stdout
            assert "test_beta.py" not in result.stdout
            assert "deselected" in result.stdout

    def test_keyword_expression_and_not(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            (tmp_path / "test_items.py").write_text("def test_fast(): pass\ndef test_slow(): pass\n")

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest",
                    "--collect-only",
                    "-k",
                    "fast and not slow",
                ],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.OK
            assert "test_fast" in result.stdout
            assert "test_slow" not in result.stdout

    def test_invalid_keyword_expression(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            (tmp_path / "test_x.py").write_text("def test_x(): pass\n")

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest",
                    "--collect-only",
                    "-k",
                    "(((",
                ],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.USAGE_ERROR
            assert "Invalid -k expression" in result.stderr


class TestLastFailedSelection:
    """Tests for --lf / --ff failure cache."""

    def test_last_failed_runs_only_previous_failure(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            (tmp_path / "test_lf.py").write_text("def test_passes(): assert True\ndef test_fails(): assert False\n")

            first = subprocess.run(
                [sys.executable, "-m", "rtest"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert first.returncode == ExitCodeValues.TESTS_FAILED

            second = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest",
                    "--collect-only",
                    "--lf",
                ],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert second.returncode == ExitCodeValues.OK
            assert "test_lf.py::test_fails" in second.stdout
            assert "test_lf.py::test_passes" not in second.stdout

    def test_lfnf_none_with_empty_cache_exits_zero(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            (tmp_path / "test_ok.py").write_text("def test_ok(): assert True\n")

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest",
                    "--lf",
                    "--lfnf",
                    "none",
                ],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.OK


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
            assert "No tests" in result.stdout

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

    def test_native_runner_keyword_filter_collect_only(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            (tmp_path / "test_alpha.py").write_text("def test_one(): pass\n")
            (tmp_path / "test_beta.py").write_text("def test_two(): pass\n")

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest",
                    "--runner",
                    "native",
                    "--collect-only",
                    "-k",
                    "beta",
                ],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.OK
            assert "test_beta.py::test_two" in result.stdout
            assert "test_alpha.py" not in result.stdout

    def test_native_runner_keyword_filter_execution(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            (tmp_path / "test_alpha.py").write_text("def test_one(): assert True\n")
            (tmp_path / "test_beta.py").write_text("def test_other(): assert False\n")

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest",
                    "--runner",
                    "native",
                    "-k",
                    "alpha",
                    "-n",
                    "1",
                ],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.OK
            assert "1 passed" in result.stdout
            assert "FAILED" not in result.stdout

    def test_native_runner_keyword_filter_single_file(self) -> None:
        """`-k` must not run deselected tests in the same file."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            (tmp_path / "test_mixed.py").write_text("def test_passes(): assert True\ndef test_fails(): assert False\n")

            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest",
                    "--runner",
                    "native",
                    "-k",
                    "passes",
                    "-n",
                    "1",
                ],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.OK
            assert "1 passed" in result.stdout
            assert "test_fails" not in result.stdout
            assert "FAILED" not in result.stdout
