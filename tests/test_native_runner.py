"""Integration tests for the native rtest runner."""

import json
import subprocess
import sys
from pathlib import Path
from typing import TypedDict

import tempfile

import rtest
from rtest.mark import PARAMETRIZE_DEPRECATION_MSG, SKIP_DEPRECATION_MSG

FIXTURES_DIR = Path(__file__).parent.parent / "test_utils" / "fixtures"


class WorkerResultDict(TypedDict, total=False):
    """Type for test result JSON objects from the worker."""

    nodeid: str
    outcome: str
    duration_ms: float
    stdout: str
    stderr: str
    error: dict[str, str] | None
    error_type: str | None


def run_worker(test_file: Path, root: Path, output_file: Path) -> list[WorkerResultDict]:
    """Run the worker on a test file and return results."""
    result = subprocess.run(
        [
            sys.executable,
            "-m",
            "rtest.worker",
            "--root",
            str(root),
            "--out",
            str(output_file),
            str(test_file),
        ],
        capture_output=True,
        text=True,
        cwd=str(root),
    )

    # Parse results
    if not output_file.exists():
        raise AssertionError(f"Output file not created. stderr: {result.stderr}")

    results: list[WorkerResultDict] = []
    with output_file.open() as f:
        for line in f:
            if line.strip():
                parsed: WorkerResultDict = json.loads(line)
                results.append(parsed)
    return results


class TestParametrizeIntegration:
    """Integration tests for parametrize functionality."""

    def test_single_param_generates_correct_nodeids(self) -> None:
        """Single @parametrize generates correct number of test cases."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_parametrize.py",
                FIXTURES_DIR.parent.parent,  # rtest root
                output_file,
            )

            # Find the single param tests
            single_param_results = [r for r in results if "::test_single_param[" in r["nodeid"]]
            assert len(single_param_results) == 3

            # Check nodeids have correct format
            nodeids = [r["nodeid"] for r in single_param_results]
            assert any(n.endswith("[0]") for n in nodeids)
            assert any(n.endswith("[1]") for n in nodeids)
            assert any(n.endswith("[2]") for n in nodeids)

            # All should pass
            assert all(r["outcome"] == "passed" for r in single_param_results)

    def test_multi_param_generates_correct_cases(self) -> None:
        """Multiple parameter @parametrize works correctly."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_parametrize.py",
                FIXTURES_DIR.parent.parent,
                output_file,
            )

            multi_param_results = [r for r in results if "::test_multi_param[" in r["nodeid"]]
            assert len(multi_param_results) == 3
            assert all(r["outcome"] == "passed" for r in multi_param_results)

    def test_stacked_params_cartesian_product(self) -> None:
        """Stacked @parametrize produces cartesian product."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_parametrize.py",
                FIXTURES_DIR.parent.parent,
                output_file,
            )

            stacked_results = [r for r in results if "::test_stacked_params[" in r["nodeid"]]
            # 2 values for a * 2 values for b = 4 cases
            assert len(stacked_results) == 4
            assert all(r["outcome"] == "passed" for r in stacked_results)

            # Check case IDs are like "0-0", "0-1", "1-0", "1-1"
            nodeids = [r["nodeid"] for r in stacked_results]
            assert any(n.endswith("[0-0]") for n in nodeids)
            assert any(n.endswith("[0-1]") for n in nodeids)
            assert any(n.endswith("[1-0]") for n in nodeids)
            assert any(n.endswith("[1-1]") for n in nodeids)

    def test_explicit_ids(self) -> None:
        """Explicit ids are used in nodeids."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_parametrize.py",
                FIXTURES_DIR.parent.parent,
                output_file,
            )

            id_results = [r for r in results if "::test_with_ids[" in r["nodeid"]]
            nodeids = [r["nodeid"] for r in id_results]
            assert any(n.endswith("[one]") for n in nodeids)
            assert any(n.endswith("[two]") for n in nodeids)

    def test_runtime_evaluated_params(self) -> None:
        """Runtime-evaluated Python values work as parameters."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_parametrize.py",
                FIXTURES_DIR.parent.parent,
                output_file,
            )

            # Function call params
            func_results = [r for r in results if "::test_function_call_params[" in r["nodeid"]]
            assert len(func_results) == 3
            assert all(r["outcome"] == "passed" for r in func_results)

            # Object params
            obj_results = [r for r in results if "::test_object_params[" in r["nodeid"]]
            assert len(obj_results) == 2
            assert all(r["outcome"] == "passed" for r in obj_results)

            # Stdlib object params
            dt_results = [r for r in results if "::test_stdlib_object_params[" in r["nodeid"]]
            assert len(dt_results) == 2
            assert all(r["outcome"] == "passed" for r in dt_results)


class TestSkipIntegration:
    """Integration tests for skip functionality."""

    def test_skipped_tests_marked_as_skipped(self) -> None:
        """@skip decorator marks tests as skipped."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_skip.py",
                FIXTURES_DIR.parent.parent,
                output_file,
            )

            skipped_with_reason = [r for r in results if r["nodeid"].endswith("::test_skipped_with_reason")]
            assert len(skipped_with_reason) == 1
            assert skipped_with_reason[0]["outcome"] == "skipped"
            error_dict = skipped_with_reason[0].get("error")
            assert error_dict is not None
            assert error_dict["reason"] == "not implemented"

    def test_class_skip_skips_all_methods(self) -> None:
        """@skip on class skips all methods."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_skip.py",
                FIXTURES_DIR.parent.parent,
                output_file,
            )

            class_methods = [r for r in results if "::TestSkippedClass::" in r["nodeid"]]
            assert len(class_methods) == 2
            assert all(r["outcome"] == "skipped" for r in class_methods)


class TestClassDiscovery:
    """Integration tests for test class discovery."""

    def test_discovers_class_methods(self) -> None:
        """Test methods in classes are discovered."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_class.py",
                FIXTURES_DIR.parent.parent,
                output_file,
            )

            basic_class = [r for r in results if "::TestBasicClass::" in r["nodeid"]]
            assert len(basic_class) == 2
            assert all(r["outcome"] == "passed" for r in basic_class)

    def test_parametrized_class_methods(self) -> None:
        """Parametrized methods in classes work correctly."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_class.py",
                FIXTURES_DIR.parent.parent,
                output_file,
            )

            param_method = [r for r in results if "::TestParametrizedClass::test_param_method[" in r["nodeid"]]
            assert len(param_method) == 2
            assert all(r["outcome"] == "passed" for r in param_method)


class TestOutcomes:
    """Integration tests for different test outcomes."""

    def test_all_outcomes(self) -> None:
        """Validate all test outcome types in single run."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_outcomes.py",
                FIXTURES_DIR.parent.parent,
                output_file,
            )

            # Pass
            passed = [r for r in results if r["nodeid"].endswith("::test_pass")]
            assert len(passed) == 1
            assert passed[0]["outcome"] == "passed"

            # Fail
            failed = [r for r in results if r["nodeid"].endswith("::test_fail")]
            assert len(failed) == 1
            assert failed[0]["outcome"] == "failed"
            assert failed[0]["error_type"] == AssertionError.__name__

            # Error
            error = [r for r in results if r["nodeid"].endswith("::test_error")]
            assert len(error) == 1
            assert error[0]["outcome"] == "error"
            assert error[0]["error_type"] == RuntimeError.__name__

            # Stdout/stderr capture
            with_output = [r for r in results if r["nodeid"].endswith("::test_pass_with_output")]
            assert len(with_output) == 1
            assert with_output[0]["stdout"].strip() == "stdout message"
            assert with_output[0]["stderr"].strip() == "stderr message"


class TestWorkerExitCode:
    """Tests for worker exit code behavior."""

    @rtest.mark.cases(
        "fixture,expected_code",
        [
            ("test_parametrize.py", 0),  # all pass
            ("test_outcomes.py", 1),  # has failures
        ],
    )
    def test_exit_code(self, fixture: str, expected_code: int) -> None:
        """Worker exit code reflects test results."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest.worker",
                    "--root",
                    str(FIXTURES_DIR.parent.parent),
                    "--out",
                    str(output_file),
                    str(FIXTURES_DIR / fixture),
                ],
                capture_output=True,
                text=True,
            )
            assert result.returncode == expected_code

    def test_exit_code_zero_on_skip_only(self) -> None:
        """Worker exits with 0 when tests are only skipped (no failures)."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            skip_only = tmp_path / "test_skip_only.py"
            skip_only.write_text(
                'import rtest\n\n@rtest.mark.skip(reason="skip")\ndef test_skipped():\n    assert False\n'
            )
            output_file = tmp_path / "results.jsonl"
            result = subprocess.run(
                [
                    sys.executable,
                    "-m",
                    "rtest.worker",
                    "--root",
                    str(tmp_path),
                    "--out",
                    str(output_file),
                    str(skip_only),
                ],
                capture_output=True,
                text=True,
            )
            assert result.returncode == 0


class TestNativeRunnerCLI:
    """Integration tests for the native runner via CLI."""

    def test_native_runner_collect_only(self) -> None:
        """--runner native --collect-only shows discovered tests."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            test_file = tmp_path / "test_example.py"
            test_file.write_text("def test_one(): pass\ndef test_two(): pass\n")

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "--collect-only"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == 0
            assert "test_one" in result.stdout
            assert "test_two" in result.stdout

    def test_native_runner_passes_with_passing_tests(self) -> None:
        """--runner native exits 0 when all tests pass."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            test_file = tmp_path / "test_pass.py"
            test_file.write_text("def test_pass(): assert True\n")

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == 0
            assert "PASSED" in result.stdout

    def test_native_runner_fails_with_failing_tests(self) -> None:
        """--runner native exits 1 when tests fail."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            test_file = tmp_path / "test_fail.py"
            test_file.write_text("def test_fail(): assert False\n")

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == 1
            assert "FAILED" in result.stdout

    def test_native_runner_with_multiple_workers(self) -> None:
        """--runner native distributes work across multiple workers."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            # Create multiple test files
            for i in range(4):
                test_file = tmp_path / f"test_file{i}.py"
                test_file.write_text(f"def test_{i}(): assert True\n")

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "2"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == 0
            assert "4 passed" in result.stdout or "PASSED" in result.stdout

    def test_native_runner_with_parametrize(self) -> None:
        """--runner native supports @rtest.mark.cases decorator."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            test_file = tmp_path / "test_param.py"
            test_file.write_text('import rtest\n\n@rtest.mark.cases("x", [1, 2, 3])\ndef test_param(x): assert x > 0\n')

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == 0
            assert "3 passed" in result.stdout or "PASSED" in result.stdout

    def test_native_runner_with_skip(self) -> None:
        """--runner native supports @rtest.mark.skip decorator."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            test_file = tmp_path / "test_skip.py"
            test_file.write_text(
                'import rtest\n\n@rtest.mark.skip(reason="test skip")\ndef test_skipped(): assert False\n'
                "\ndef test_pass(): assert True\n"
            )

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == 0
            assert "1 passed" in result.stdout
            assert "1 skipped" in result.stdout

    def test_native_runner_no_tests_found(self) -> None:
        """--runner native handles empty test directory gracefully."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == 0
            assert "No tests found" in result.stdout

    def test_native_runner_class_tests(self) -> None:
        """--runner native discovers and runs test class methods."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            test_file = tmp_path / "test_class.py"
            test_file.write_text("class TestExample:\n    def test_method(self): assert True\n")

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == 0
            assert "1 passed" in result.stdout


class TestPytestMarkerCompatibility:
    """Tests for pytest marker compatibility with deprecation warnings."""

    def test_pytest_parametrize_works_with_deprecation(self) -> None:
        """@pytest.mark.parametrize works but emits deprecation warning."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            result = subprocess.run(
                [
                    sys.executable,
                    "-W",
                    "always",
                    "-m",
                    "rtest.worker",
                    "--root",
                    str(FIXTURES_DIR.parent.parent),
                    "--out",
                    str(output_file),
                    str(FIXTURES_DIR / "test_pytest_compat.py"),
                ],
                capture_output=True,
                text=True,
                cwd=str(FIXTURES_DIR.parent.parent),
            )

            # Should work
            assert output_file.exists()
            results: list[WorkerResultDict] = []
            with output_file.open() as f:
                for line in f:
                    if line.strip():
                        parsed: WorkerResultDict = json.loads(line)
                        results.append(parsed)

            # Parametrized tests should run
            param_results = [r for r in results if "::test_pytest_parametrize[" in r["nodeid"]]
            assert len(param_results) == 3
            assert all(r["outcome"] == "passed" for r in param_results)

            # Should emit deprecation warning with exact message
            expected_warning = PARAMETRIZE_DEPRECATION_MSG.format(func_name="test_pytest_parametrize")
            assert expected_warning in result.stderr

    def test_pytest_skip_works_with_deprecation(self) -> None:
        """@pytest.mark.skip works but emits deprecation warning."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            result = subprocess.run(
                [
                    sys.executable,
                    "-W",
                    "always",
                    "-m",
                    "rtest.worker",
                    "--root",
                    str(FIXTURES_DIR.parent.parent),
                    "--out",
                    str(output_file),
                    str(FIXTURES_DIR / "test_pytest_compat.py"),
                ],
                capture_output=True,
                text=True,
                cwd=str(FIXTURES_DIR.parent.parent),
            )

            # Should work
            assert output_file.exists()
            results: list[WorkerResultDict] = []
            with output_file.open() as f:
                for line in f:
                    if line.strip():
                        parsed: WorkerResultDict = json.loads(line)
                        results.append(parsed)

            # Skip test should be skipped
            skip_results = [r for r in results if r["nodeid"].endswith("::test_pytest_skip")]
            assert len(skip_results) == 1
            assert skip_results[0]["outcome"] == "skipped"

            # Should emit deprecation warning with exact message
            expected_warning = SKIP_DEPRECATION_MSG.format(func_name="test_pytest_skip")
            assert expected_warning in result.stderr


class TestPythonFilesConfiguration:
    """Tests for python_files configuration from pyproject.toml."""

    def test_native_runner_respects_custom_python_files(self) -> None:
        """Native runner uses python_files patterns from pyproject.toml."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            pyproject = tmp_path / "pyproject.toml"
            pyproject.write_text('[tool.pytest.ini_options]\npython_files = ["check_*.py", "*_spec.py"]\n')

            (tmp_path / "check_validation.py").write_text("def test_one(): assert True\n")
            (tmp_path / "user_spec.py").write_text("def test_two(): assert True\n")
            (tmp_path / "test_standard.py").write_text("def test_three(): assert True\n")

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )

            assert result.returncode == 0
            assert "Running 2 test file(s)" in result.stdout
            assert "2 passed" in result.stdout

    def test_native_runner_uses_defaults_without_config(self) -> None:
        """Native runner uses default patterns when python_files not configured."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            (tmp_path / "test_example.py").write_text("def test_default(): assert True\n")
            (tmp_path / "example_test.py").write_text("def test_suffix(): assert True\n")
            (tmp_path / "check_other.py").write_text("def test_nonstandard(): assert True\n")

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )

            assert result.returncode == 0
            assert "Running 2 test file(s)" in result.stdout
            assert "check_other.py" not in result.stdout


class TestPythonClassesConfiguration:
    """Tests for python_classes pattern matching with fnmatch."""

    def test_native_runner_respects_custom_python_classes(self) -> None:
        """Native runner uses python_classes patterns from pyproject.toml."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            pyproject = tmp_path / "pyproject.toml"
            pyproject.write_text('[tool.pytest.ini_options]\npython_classes = ["Check*", "*Suite"]\n')

            test_file = tmp_path / "test_classes.py"
            test_file.write_text(
                "class CheckValidation:\n"
                "    def test_one(self): assert True\n\n"
                "class UserSuite:\n"
                "    def test_two(self): assert True\n\n"
                "class TestStandard:\n"
                "    def test_three(self): assert True\n"
            )

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )

            assert result.returncode == 0
            # Should find CheckValidation and UserSuite, but NOT TestStandard
            assert "2 passed" in result.stdout

    def test_default_python_classes_pattern(self) -> None:
        """Default python_classes pattern matches Test* prefix."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            test_file = tmp_path / "test_default.py"
            test_file.write_text(
                "class TestValid:\n"
                "    def test_one(self): assert True\n\n"
                "class CheckInvalid:\n"
                "    def test_two(self): assert True\n"
            )

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )

            assert result.returncode == 0
            # Should only find TestValid with default Test* pattern
            assert "1 passed" in result.stdout

    def test_fnmatch_glob_patterns(self) -> None:
        """fnmatch supports full glob patterns including contains matching."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            pyproject = tmp_path / "pyproject.toml"
            pyproject.write_text('[tool.pytest.ini_options]\npython_classes = ["*Test*"]\n')

            test_file = tmp_path / "test_glob.py"
            test_file.write_text(
                "class MyTestCase:\n"
                "    def test_one(self): assert True\n\n"
                "class TestStandard:\n"
                "    def test_two(self): assert True\n\n"
                "class SuiteTestRunner:\n"
                "    def test_three(self): assert True\n\n"
                "class NoMatch:\n"
                "    def test_four(self): assert True\n"
            )

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )

            assert result.returncode == 0
            # Should match MyTestCase, TestStandard, SuiteTestRunner but NOT NoMatch
            assert "3 passed" in result.stdout


class TestPythonFunctionsConfiguration:
    """Tests for python_functions pattern matching with fnmatch."""

    def test_native_runner_respects_custom_python_functions(self) -> None:
        """Native runner uses python_functions patterns from pyproject.toml."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            pyproject = tmp_path / "pyproject.toml"
            pyproject.write_text('[tool.pytest.ini_options]\npython_functions = ["check_*", "*_test"]\n')

            test_file = tmp_path / "test_functions.py"
            test_file.write_text(
                "def check_validation(): assert True\n\ndef user_test(): assert True\n\ndef test_standard(): assert True\n"
            )

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )

            assert result.returncode == 0
            assert "2 passed" in result.stdout

    def test_default_python_functions_pattern(self) -> None:
        """Default python_functions pattern matches test* prefix."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            test_file = tmp_path / "test_default.py"
            test_file.write_text("def test_valid(): assert True\n\ndef check_invalid(): assert True\n")

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )

            assert result.returncode == 0
            assert "1 passed" in result.stdout

    def test_python_functions_in_classes(self) -> None:
        """python_functions patterns apply to methods in test classes."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            pyproject = tmp_path / "pyproject.toml"
            pyproject.write_text('[tool.pytest.ini_options]\npython_functions = ["check_*", "verify_*"]\n')

            test_file = tmp_path / "test_class_methods.py"
            test_file.write_text(
                "class TestValidation:\n"
                "    def check_one(self): assert True\n"
                "    def verify_two(self): assert True\n"
                "    def test_three(self): assert True\n"
            )

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )

            assert result.returncode == 0
            assert "2 passed" in result.stdout
