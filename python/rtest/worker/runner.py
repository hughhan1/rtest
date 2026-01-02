"""Test runner implementation for rtest worker."""

from __future__ import annotations

import contextlib
import fnmatch
import importlib.util
import inspect
import io
import json
import sys
import time
import traceback
import warnings
from dataclasses import asdict, dataclass, field
from pathlib import Path
from types import FunctionType, MappingProxyType, ModuleType
from typing import Callable, Literal


@dataclass
class TestResult:
    """Result of a single test execution."""

    nodeid: str
    outcome: Literal["passed", "failed", "error", "skipped"]
    duration_ms: float
    stdout: str = ""
    stderr: str = ""
    error: dict[str, str] | None = None
    error_type: str | None = None  # Exception type name for structured matching


@dataclass
class TestCase:
    """A single test case to execute."""

    nodeid: str
    func: FunctionType
    kwargs: dict[str, object] = field(default_factory=dict)
    skip: bool = False
    skip_reason: str = ""
    test_class: type | None = None


def _import_module_from_file(file_path: Path, root: Path) -> ModuleType:
    """Import a Python module from a file path."""
    try:
        rel_path = file_path.relative_to(root)
    except ValueError:
        rel_path = file_path

    module_name = str(rel_path).replace("/", ".").replace("\\", ".")
    if module_name.endswith(".py"):
        module_name = module_name[:-3]

    # Add prefix to avoid collisions with installed packages
    module_name = f"_rtest_module_.{module_name}"

    spec = importlib.util.spec_from_file_location(module_name, file_path)
    if spec is None or spec.loader is None:
        raise ImportError(f"Cannot load module from {file_path}")

    module = importlib.util.module_from_spec(spec)
    sys.modules[module_name] = module
    spec.loader.exec_module(module)

    return module


def _is_test_function(name: str, obj: object, patterns: list[str]) -> bool:
    """Check if an object is a test function based on patterns."""
    if not inspect.isfunction(obj):
        return False
    return any(fnmatch.fnmatch(name, p) for p in patterns)


def _is_test_class(name: str, obj: object, patterns: list[str]) -> bool:
    """Check if an object is a test class based on patterns."""
    if not inspect.isclass(obj):
        return False
    return any(fnmatch.fnmatch(name, p) for p in patterns)


def _is_skipped(obj: object) -> tuple[bool, str]:
    """Check if an object has the skip marker (rtest or pytest)."""
    from rtest.mark import SKIP_DEPRECATION_MSG

    # Check rtest marker first
    skip_flag: bool = bool(getattr(obj, "__rtest_skip__", False))
    if skip_flag:
        skip_reason_attr: str = str(getattr(obj, "__rtest_skip_reason__", ""))
        return True, skip_reason_attr

    # Check pytest marker (with deprecation warning)
    pytest_marks: list[object] = getattr(obj, "pytestmark", [])
    for pytest_mark in pytest_marks:
        if getattr(pytest_mark, "name", None) == "skip":
            func_name: str = str(getattr(obj, "__name__", str(obj)))
            warnings.warn(
                SKIP_DEPRECATION_MSG.format(func_name=func_name),
                DeprecationWarning,
                stacklevel=4,
            )
            mark_kwargs: MappingProxyType[str, object] = getattr(pytest_mark, "kwargs", MappingProxyType({}))
            reason_raw: object = mark_kwargs.get("reason", "")
            reason: str = str(reason_raw) if reason_raw else ""
            if not reason:
                mark_args: tuple[object, ...] = getattr(pytest_mark, "args", ())
                if mark_args:
                    reason = str(mark_args[0])
            return True, reason

    return False, ""


def _discover_tests_in_module(
    module: ModuleType,
    file_path: Path,
    root: Path,
    python_classes: list[str],
    python_functions: list[str],
) -> list[TestCase]:
    """Discover all test cases in a module."""
    from rtest.mark import expand_parametrize, get_case_nodeid_suffix

    try:
        rel_path = str(file_path.relative_to(root))
    except ValueError:
        rel_path = str(file_path)

    rel_path = rel_path.replace("\\", "/")

    test_cases: list[TestCase] = []

    for name, obj in inspect.getmembers(module):
        if _is_test_function(name, obj, python_functions):
            func_obj: FunctionType = obj  # type: ignore[assignment]
            func_skip, func_skip_reason = _is_skipped(func_obj)

            for case_id, kwargs in expand_parametrize(func_obj):
                suffix = get_case_nodeid_suffix(case_id)
                nodeid = f"{rel_path}::{name}{suffix}"
                test_cases.append(
                    TestCase(
                        nodeid=nodeid,
                        func=func_obj,
                        kwargs=kwargs,
                        skip=func_skip,
                        skip_reason=func_skip_reason,
                    )
                )

        elif _is_test_class(name, obj, python_classes):
            # obj is a class at this point
            class_obj: type = obj
            class_skip, class_skip_reason = _is_skipped(class_obj)

            # Walk MRO explicitly to discover inherited test methods
            seen_methods: set[str] = set()
            for base_class in inspect.getmro(class_obj):
                for method_name in base_class.__dict__:
                    if method_name in seen_methods:
                        continue
                    # Check if method matches any python_functions pattern
                    if not any(fnmatch.fnmatch(method_name, p) for p in python_functions):
                        continue
                    method_obj: object = getattr(class_obj, method_name)
                    if not inspect.isfunction(method_obj):
                        continue
                    method_func: FunctionType = method_obj  # type: ignore[assignment]
                    seen_methods.add(method_name)

                    method_skip, method_skip_reason = _is_skipped(method_func)
                    skip = method_skip or class_skip
                    skip_reason = method_skip_reason if method_skip else class_skip_reason

                    for case_id, kwargs in expand_parametrize(method_func):
                        suffix = get_case_nodeid_suffix(case_id)
                        nodeid = f"{rel_path}::{name}::{method_name}{suffix}"
                        test_cases.append(
                            TestCase(
                                nodeid=nodeid,
                                func=method_func,
                                kwargs=kwargs,
                                skip=skip,
                                skip_reason=skip_reason,
                                test_class=class_obj,
                            )
                        )

    return test_cases


def _run_single_test(test_case: TestCase) -> TestResult:
    """Execute a single test case and capture results."""
    if test_case.skip:
        return TestResult(
            nodeid=test_case.nodeid,
            outcome="skipped",
            duration_ms=0.0,
            error={"reason": test_case.skip_reason} if test_case.skip_reason else None,
        )

    stdout_capture = io.StringIO()
    stderr_capture = io.StringIO()

    outcome = "passed"
    error_info: dict[str, str] | None = None
    error_type: str | None = None
    start_time = time.perf_counter()

    try:
        with contextlib.redirect_stdout(stdout_capture), contextlib.redirect_stderr(stderr_capture):
            if test_case.test_class is not None:
                instance: object = test_case.test_class()
                setup: Callable[[], object] | None = getattr(instance, "setup_method", None) or getattr(
                    instance, "setUp", None
                )
                if setup is not None:
                    setup()
                try:
                    bound_method: Callable[..., object] = getattr(instance, test_case.func.__name__)
                    bound_method(**test_case.kwargs)
                finally:
                    teardown: Callable[[], object] | None = getattr(instance, "teardown_method", None) or getattr(
                        instance, "tearDown", None
                    )
                    if teardown is not None:
                        teardown()
            else:
                test_case.func(**test_case.kwargs)
    except AssertionError as e:
        outcome = "failed"
        error_info = {"traceback": traceback.format_exc()}
        error_type = type(e).__name__
    except Exception as e:
        outcome = "error"
        error_info = {"traceback": traceback.format_exc()}
        error_type = type(e).__name__

    duration_ms = (time.perf_counter() - start_time) * 1000

    return TestResult(
        nodeid=test_case.nodeid,
        outcome=outcome,
        duration_ms=duration_ms,
        stdout=stdout_capture.getvalue(),
        stderr=stderr_capture.getvalue(),
        error=error_info,
        error_type=error_type,
    )


def run_tests(
    root: Path,
    output_file: Path,
    test_files: list[Path],
    python_classes: list[str] | None = None,
    python_functions: list[str] | None = None,
) -> int:
    """Run tests from the given files and write results to JSONL.

    Args:
        root: Repository root path.
        output_file: Path to write JSONL results.
        test_files: List of test files to run.
        python_classes: Patterns for test class names (default: ["Test*"]).
        python_functions: Patterns for test function/method names (default: ["test*"]).

    Returns:
        Exit code: 0 if all passed/skipped, 1 if any failed/error.
    """
    if python_classes is None:
        python_classes = ["Test*"]
    if python_functions is None:
        python_functions = ["test*"]

    root_str = str(root.resolve())
    if root_str not in sys.path:
        sys.path.insert(0, root_str)

    # Add all test directories to sys.path for sibling imports
    test_dirs = {str(f.parent.resolve()) for f in test_files}
    for test_dir in sorted(test_dirs):
        if test_dir not in sys.path:
            sys.path.insert(0, test_dir)

    all_test_cases: list[TestCase] = []
    import_errors: list[TestResult] = []
    loaded_modules: list[str] = []

    for file_path in test_files:
        try:
            module: ModuleType = _import_module_from_file(file_path.resolve(), root.resolve())
            module_name: str = module.__name__
            loaded_modules.append(module_name)
            test_cases = _discover_tests_in_module(module, file_path, root, python_classes, python_functions)
            all_test_cases.extend(test_cases)
        except Exception as e:
            try:
                rel_path = str(file_path.relative_to(root))
            except ValueError:
                rel_path = str(file_path)

            import_errors.append(
                TestResult(
                    nodeid=f"{rel_path}::IMPORT_ERROR",
                    outcome="error",
                    duration_ms=0.0,
                    error={"traceback": traceback.format_exc()},
                    error_type=type(e).__name__,
                )
            )

    all_test_cases.sort(key=lambda tc: tc.nodeid)

    results: list[TestResult] = []
    results.extend(import_errors)

    for test_case in all_test_cases:
        result = _run_single_test(test_case)
        results.append(result)

    for loaded_module_name in loaded_modules:
        sys.modules.pop(loaded_module_name, None)

    output_file.parent.mkdir(parents=True, exist_ok=True)
    with output_file.open("w") as f:
        for result in results:
            result_dict: dict[str, object] = asdict(result)
            f.write(json.dumps(result_dict) + "\n")

    has_failures = any(r.outcome in ("failed", "error") for r in results)
    return 1 if has_failures else 0
