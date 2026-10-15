"""Test runner implementation for rtest worker."""

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
from dataclasses import asdict, dataclass, field, replace
from pathlib import Path
from types import FunctionType, MappingProxyType, ModuleType
from typing import Callable, Literal

from rtest.mark import ExpandedCase, SkipInfo


@dataclass(frozen=True)
class Passed:
    outcome: Literal["passed"] = field(default="passed", init=False)


@dataclass(frozen=True)
class TracebackInfo:
    traceback: str


@dataclass(frozen=True)
class Failed:
    error: TracebackInfo
    error_type: str
    outcome: Literal["failed"] = field(default="failed", init=False)


@dataclass(frozen=True)
class Error:
    error: TracebackInfo
    error_type: str
    outcome: Literal["error"] = field(default="error", init=False)


@dataclass(frozen=True)
class Skipped:
    error: SkipInfo
    outcome: Literal["skipped"] = field(default="skipped", init=False)


Outcome = Passed | Failed | Error | Skipped


@dataclass(frozen=True)
class TestResult:
    """Common execution metadata paired with one complete outcome."""

    nodeid: str
    status: Outcome
    duration_ms: float
    stdout: str = ""
    stderr: str = ""

    def to_json(self) -> str:
        data = asdict(self)
        data.update(data.pop("status"))
        return json.dumps(data)


@dataclass(frozen=True)
class MethodTarget:
    test_class: type
    name: str


@dataclass(frozen=True)
class TestCase:
    """A single test case to execute."""

    nodeid: str
    target: FunctionType | MethodTarget
    kwargs: dict[str, object] = field(default_factory=dict)
    skip: SkipInfo | None = None


def _relative_path(file_path: Path, root: Path) -> str:
    try:
        return str(file_path.relative_to(root))
    except ValueError:
        return str(file_path)


def _import_module_from_file(file_path: Path, root: Path) -> ModuleType:
    """Import a Python module from a file path."""
    module_name = _relative_path(file_path, root).replace("/", ".").replace("\\", ".").removesuffix(".py")

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


def _skip_info(obj: object) -> SkipInfo | None:
    """Check if an object has the skip marker (rtest or pytest)."""
    from rtest.mark import SKIP_DEPRECATION_MSG

    # Check rtest marker first
    skip = getattr(obj, "__rtest_skip__", None)
    if isinstance(skip, SkipInfo):
        return skip

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
            return SkipInfo(reason)

    return None


def _parametrize_test_case(test_case: TestCase, cases: list[ExpandedCase] | None) -> list[TestCase]:
    if cases is None:
        return [test_case]
    return [replace(test_case, nodeid=f"{test_case.nodeid}[{case_id}]", kwargs=kwargs) for case_id, kwargs in cases]


def _discover_tests_in_module(
    module: ModuleType,
    file_path: Path,
    root: Path,
    python_classes: list[str],
    python_functions: list[str],
) -> list[TestCase]:
    """Discover all test cases in a module."""
    from rtest.mark import expand_parametrize

    rel_path = _relative_path(file_path, root).replace("\\", "/")

    test_cases: list[TestCase] = []

    for name, obj in inspect.getmembers(module):
        if _is_test_function(name, obj, python_functions):
            func_obj: FunctionType = obj  # type: ignore[assignment]
            func_skip = _skip_info(func_obj)

            test_cases.extend(
                _parametrize_test_case(
                    TestCase(
                        nodeid=f"{rel_path}::{name}",
                        target=func_obj,
                        skip=func_skip,
                    ),
                    expand_parametrize(func_obj),
                )
            )

        elif _is_test_class(name, obj, python_classes):
            # obj is a class at this point
            class_obj: type = obj
            class_skip = _skip_info(class_obj)

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

                    skip = _skip_info(method_func) or class_skip

                    test_cases.extend(
                        _parametrize_test_case(
                            TestCase(
                                nodeid=f"{rel_path}::{name}::{method_name}",
                                target=MethodTarget(class_obj, method_name),
                                skip=skip,
                            ),
                            expand_parametrize(method_func),
                        )
                    )

    return test_cases


def _run_single_test(test_case: TestCase) -> TestResult:
    """Execute a single test case and capture results."""
    if test_case.skip is not None:
        return TestResult(
            nodeid=test_case.nodeid,
            status=Skipped(test_case.skip),
            duration_ms=0.0,
        )

    stdout_capture = io.StringIO()
    stderr_capture = io.StringIO()

    status: Outcome = Passed()
    start_time = time.perf_counter()

    try:
        with contextlib.redirect_stdout(stdout_capture), contextlib.redirect_stderr(stderr_capture):
            if isinstance(test_case.target, MethodTarget):
                instance: object = test_case.target.test_class()
                setup: Callable[[], object] | None = getattr(instance, "setup_method", None) or getattr(
                    instance, "setUp", None
                )
                if setup is not None:
                    setup()
                try:
                    bound_method: Callable[..., object] = getattr(instance, test_case.target.name)
                    bound_method(**test_case.kwargs)
                finally:
                    teardown: Callable[[], object] | None = getattr(instance, "teardown_method", None) or getattr(
                        instance, "tearDown", None
                    )
                    if teardown is not None:
                        teardown()
            else:
                test_case.target(**test_case.kwargs)
    except AssertionError as e:
        status = Failed(TracebackInfo(traceback.format_exc()), type(e).__name__)
    except Exception as e:
        status = Error(TracebackInfo(traceback.format_exc()), type(e).__name__)

    duration_ms = (time.perf_counter() - start_time) * 1000

    return TestResult(
        nodeid=test_case.nodeid,
        status=status,
        duration_ms=duration_ms,
        stdout=stdout_capture.getvalue(),
        stderr=stderr_capture.getvalue(),
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

    # Add the root, then test directories, preserving sibling-import precedence.
    test_dirs = {str(f.parent.resolve()) for f in test_files}
    for test_dir in [str(root.resolve()), *sorted(test_dirs)]:
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
            import_errors.append(
                TestResult(
                    nodeid=f"{_relative_path(file_path, root)}::IMPORT_ERROR",
                    status=Error(TracebackInfo(traceback.format_exc()), type(e).__name__),
                    duration_ms=0.0,
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
            f.write(result.to_json() + "\n")

    has_failures = any(isinstance(r.status, (Failed, Error)) for r in results)
    return 1 if has_failures else 0
