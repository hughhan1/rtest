"""Integration tests for the native rtest runner."""

import json
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import TypedDict

from rtest.exit_code import ExitCodeValues
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


def run_worker(test_file: Path, root: Path, output_file: Path, expected_exit: int = 0) -> list[WorkerResultDict]:
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

    assert result.returncode == expected_exit, result.stderr

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

    def test_empty_and_hyphenated_ids_remain_real_id_components(self) -> None:
        from test_helpers import extract_test_lines, run_collection

        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            test_file = root / "test_ids.py"
            test_file.write_text(
                "import rtest\n"
                "@rtest.mark.cases('x', [''])\n"
                "def test_empty(x): assert x == ''\n"
                "@rtest.mark.cases('x', [1], ids=[''])\n"
                "def test_explicit(x): assert x == 1\n"
                "@rtest.mark.cases('text,number', [('', -2), ('a-b', -3)])\n"
                "def test_row(text, number): assert number < 0\n"
                "def test_plain(): pass\n"
            )
            expected = [
                "test_ids.py::test_empty[]",
                "test_ids.py::test_explicit[]",
                "test_ids.py::test_plain",
                "test_ids.py::test_row[--2]",
                "test_ids.py::test_row[a-b--3]",
            ]
            collected = run_collection(root)
            assert collected.returncode == 0, collected.output
            assert sorted(extract_test_lines(collected.output_lines)) == expected
            results = run_worker(test_file, root, root / "results.jsonl")
            assert [result["nodeid"] for result in results] == expected
            assert all(result["outcome"] == "passed" for result in results)

    def test_partial_and_extra_ids(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            root = Path(tmp_dir)
            test_file = root / "test_ids.py"
            test_file.write_text(
                "import rtest\n"
                "@rtest.mark.cases('x', [1, 1, 2], ids=['1'])\n"
                "def test_partial(x): assert x > 0\n"
                "@rtest.mark.cases('x', [1], ids=['used', 'unused'])\n"
                "def test_extra(x): assert x == 1\n"
                "@rtest.mark.cases('x', [3], ids=[])\n"
                "def test_empty_ids(x): assert x == 3\n"
                "@rtest.mark.cases('x', [], ids=['unused'])\n"
                "def test_no_values(x): assert False\n"
            )
            results = run_worker(test_file, root, root / "results.jsonl")
            assert [result["nodeid"] for result in results] == [
                "test_ids.py::test_empty_ids[3]",
                "test_ids.py::test_extra[used]",
                "test_ids.py::test_partial[1]",
                "test_ids.py::test_partial[1_1]",
                "test_ids.py::test_partial[2]",
            ]
            assert all(result["outcome"] == "passed" for result in results)

    def test_parameterized_fixture_ids_and_execution(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            results = run_worker(
                FIXTURES_DIR / "test_parametrize.py", FIXTURES_DIR.parent.parent, Path(tmp_dir) / "results.jsonl"
            )

        by_test: dict[str, list[str]] = {}
        for result in results:
            name = result["nodeid"].split("::", 1)[1]
            by_test.setdefault(name.partition("[")[0], []).append(name)
        assert {name: len(ids) for name, ids in by_test.items()} == {
            "test_single_param": 3,
            "test_multi_param": 3,
            "test_stacked_params": 4,
            "test_with_ids": 2,
            "test_no_params": 1,
            "test_function_call_params": 3,
            "test_object_params": 2,
            "test_stdlib_object_params": 2,
        }
        for name, suffixes in {
            "test_single_param": ["1", "2", "3"],
            "test_multi_param": ["1-2", "3-4", "5-6"],
            "test_stacked_params": ["1-10", "1-20", "2-10", "2-20"],
            "test_with_ids": ["one", "two"],
            "test_function_call_params": ["4", "6", "8"],
        }.items():
            assert by_test[name] == [f"{name}[{suffix}]" for suffix in suffixes]
        assert by_test["test_no_params"] == ["test_no_params"]
        assert all(result["outcome"] == "passed" for result in results)


class TestSkipIntegration:
    """Integration tests for skip functionality."""

    def test_function_and_class_skips_preserve_reasons(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            results = run_worker(
                FIXTURES_DIR / "test_skip.py", FIXTURES_DIR.parent.parent, Path(tmp_dir) / "results.jsonl"
            )
        assert len(results) == 5
        assert {
            result["nodeid"].split("::", 1)[1]: (result["outcome"], (result.get("error") or {}).get("reason"))
            for result in results
        } == {
            "TestSkippedClass::test_method1": ("skipped", "entire class skipped"),
            "TestSkippedClass::test_method2": ("skipped", "entire class skipped"),
            "test_skipped_with_reason": ("skipped", "not implemented"),
            "test_skipped_without_reason": ("skipped", ""),
            "test_not_skipped": ("passed", None),
        }


class TestClassDiscovery:
    """Integration tests for test class discovery."""

    def test_class_methods_parameters_skips_and_setup(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            results = run_worker(
                FIXTURES_DIR / "test_class.py", FIXTURES_DIR.parent.parent, Path(tmp_dir) / "results.jsonl"
            )
        assert len(results) == 8
        assert {result["nodeid"].split("::", 1)[1]: result["outcome"] for result in results} == {
            "TestBasicClass::test_method1": "passed",
            "TestBasicClass::test_method2": "passed",
            "TestParametrizedClass::test_param_method[1]": "passed",
            "TestParametrizedClass::test_param_method[2]": "passed",
            "TestParametrizedClass::test_normal_method": "passed",
            "TestMixedClass::test_normal": "passed",
            "TestMixedClass::test_skipped": "skipped",
            "TestWithSetup::test_uses_setup": "passed",
        }


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
                expected_exit=1,
            )

            assert len(results) == 4
            by_name = {result["nodeid"].split("::", 1)[1]: result for result in results}
            assert {name: result["outcome"] for name, result in by_name.items()} == {
                "test_pass": "passed",
                "test_fail": "failed",
                "test_error": "error",
                "test_pass_with_output": "passed",
            }
            assert by_name["test_fail"]["error_type"] == AssertionError.__name__
            assert by_name["test_error"]["error_type"] == RuntimeError.__name__
            assert by_name["test_pass_with_output"]["stdout"].strip() == "stdout message"
            assert by_name["test_pass_with_output"]["stderr"].strip() == "stderr message"


class TestRaisesIntegration:
    def test_raises_outcomes(self) -> None:
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            output_file = tmp_path / "results.jsonl"
            results = run_worker(
                FIXTURES_DIR / "test_raises.py",
                FIXTURES_DIR.parent.parent,
                output_file,
                expected_exit=1,
            )

            assert len(results) == 8
            assert {result["nodeid"].split("::", 1)[1]: result["outcome"] for result in results} == {
                "test_raises_expected": "passed",
                "test_raises_with_match": "passed",
                "test_raises_no_exception": "failed",
                "test_raises_wrong_exception": "error",
                "test_raises_match_fails": "failed",
                "test_raises_value_attribute": "passed",
                "test_raises_exception_tuple": "passed",
                "test_raises_subclass": "passed",
            }


class TestWorkerExitCode:
    """Tests for worker exit code behavior."""

    def test_exit_code_zero_on_skip_only(self) -> None:
        """Worker exits with 0 when tests are only skipped (no failures)."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)
            skip_only = tmp_path / "test_skip_only.py"
            skip_only.write_text(
                'import rtest\n\n@rtest.mark.skip(reason="skip")\ndef test_skipped():\n    assert False\n'
            )
            output_file = tmp_path / "results.jsonl"
            results = run_worker(skip_only, tmp_path, output_file)
            assert len(results) == 1
            assert results[0]["outcome"] == "skipped"


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
            assert result.returncode == ExitCodeValues.OK
            assert "test_one" in result.stdout
            assert "test_two" in result.stdout

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
            assert result.returncode == ExitCodeValues.OK
            assert "4 passed" in result.stdout

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
            assert result.returncode == ExitCodeValues.OK
            assert "3 passed" in result.stdout

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
            assert result.returncode == ExitCodeValues.OK
            assert "1 passed" in result.stdout
            assert "1 skipped" in result.stdout


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

            assert result.returncode == ExitCodeValues.OK
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

            assert result.returncode == ExitCodeValues.OK
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

            assert result.returncode == ExitCodeValues.OK
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

            assert result.returncode == ExitCodeValues.OK
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

            assert result.returncode == ExitCodeValues.OK
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
                "def check_validation(): assert True\n\n"
                "def user_test(): assert True\n\n"
                "def test_standard(): assert True\n"
            )

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )

            assert result.returncode == ExitCodeValues.OK
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

            assert result.returncode == ExitCodeValues.OK
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

            assert result.returncode == ExitCodeValues.OK
            assert "2 passed" in result.stdout


class TestSysPathBehavior:
    """Tests for sys.path setup during test execution."""

    def test_sibling_module_imports(self) -> None:
        """Test files can import from sibling modules in the same directory."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            tmp_path = Path(tmp_dir)

            # Create a helper module (not a test file)
            helper = tmp_path / "helper_module.py"
            helper.write_text("def helper_func(): return 42\n")

            # Create a test file that imports from the sibling module
            test_file = tmp_path / "test_uses_helper.py"
            test_file.write_text(
                "from helper_module import helper_func\n\ndef test_import_works():\n    assert helper_func() == 42\n"
            )

            result = subprocess.run(
                [sys.executable, "-m", "rtest", "--runner", "native", "-n", "1"],
                capture_output=True,
                text=True,
                cwd=str(tmp_path),
            )
            assert result.returncode == ExitCodeValues.OK
            assert "1 passed" in result.stdout
