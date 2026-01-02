"""rtest mark decorators for test cases and skipping.

This module provides the @rtest.mark.cases and @rtest.mark.skip decorators
for native test execution.

Example usage:
    import rtest

    @rtest.mark.cases("x", [1, 2, 3])
    def test_single_param(x):
        assert x > 0

    @rtest.mark.cases("x,y", [(1, 2), (3, 4)])
    def test_multi_param(x, y):
        assert x < y

    @rtest.mark.skip(reason="not implemented")
    def test_skipped():
        pass
"""

import itertools
import warnings
from dataclasses import dataclass
from types import FunctionType, MappingProxyType
from typing import Callable, NamedTuple, Sequence, TypeVar

# TypeVar for decorators that preserve function signatures
# The bound includes "type" to support decorating classes as well as functions
F = TypeVar("F", bound=Callable[..., object] | type)

# Deprecation warning messages for pytest marker compatibility
PARAMETRIZE_DEPRECATION_MSG = (
    "@pytest.mark.parametrize on {func_name} is deprecated. "
    "Use @rtest.mark.cases instead. pytest.mark support will be removed in a future version."
)

SKIP_DEPRECATION_MSG = (
    "@pytest.mark.skip on {func_name} is deprecated. "
    "Use @rtest.mark.skip instead. pytest.mark support will be removed in a future version."
)


@dataclass
class ParametrizeSpec:
    """Specification for a single @parametrize decorator."""

    argnames: tuple[str, ...]
    argvalues: tuple[object, ...]
    ids: tuple[str, ...] | None


class PytestMarkerArgs(NamedTuple):
    """Extracted arguments from a pytest marker."""

    argnames: object
    argvalues: object


class ExpandedCase(NamedTuple):
    """A single expanded test case with its ID and kwargs."""

    case_id: str
    kwargs: dict[str, object]


class Mark:
    """Container for rtest mark decorators."""

    def cases(
        self,
        argnames: str | Sequence[str],
        argvalues: Sequence[object],
        *,
        ids: Sequence[str] | None = None,
    ) -> Callable[[F], F]:
        """Decorator to define test cases for a test function.

        Args:
            argnames: Comma-separated string or sequence of argument names.
            argvalues: Sequence of argument values. For single argname, each item
                is the value. For multiple argnames, each item should be a tuple/list.
            ids: Optional sequence of string IDs for each parameter set.

        Returns:
            Decorated function with test case metadata attached.

        Example:
            @rtest.mark.cases("x", [1, 2, 3])
            def test_single(x):
                pass

            @rtest.mark.cases("x,y", [(1, 2), (3, 4)])
            def test_multi(x, y):
                pass
        """
        if isinstance(argnames, str):
            parsed_argnames = tuple(name.strip() for name in argnames.split(","))
        else:
            parsed_argnames = tuple(argnames)

        materialized_ids: tuple[str, ...] | None = tuple(ids) if ids is not None else None

        spec = ParametrizeSpec(
            argnames=parsed_argnames,
            argvalues=tuple(argvalues),
            ids=materialized_ids,
        )

        def decorator(func: F) -> F:
            cases_list: list[ParametrizeSpec] = getattr(func, "__rtest_cases__", None) or []
            if not getattr(func, "__rtest_cases__", None):
                func.__rtest_cases__ = cases_list  # type: ignore[union-attr]
            cases_list.append(spec)
            return func

        return decorator

    def skip(self, reason: str = "") -> Callable[[F], F]:
        """Decorator to skip a test function or class.

        Args:
            reason: Optional reason for skipping.

        Returns:
            Decorated function/class with skip metadata attached.

        Example:
            @rtest.mark.skip(reason="not implemented yet")
            def test_something():
                pass

            @rtest.mark.skip("WIP")
            class TestClass:
                def test_method(self):
                    pass
        """

        def decorator(func_or_class: F) -> F:
            func_or_class.__rtest_skip__ = True  # type: ignore[union-attr]
            func_or_class.__rtest_skip_reason__ = reason  # type: ignore[union-attr]
            return func_or_class

        return decorator


mark = Mark()


def _deduplicate_ids(ids: list[str]) -> list[str]:
    """Deduplicate IDs by adding suffixes _1, _2, etc. for duplicates."""
    seen: dict[str, int] = {}
    result: list[str] = []

    for id_ in ids:
        if id_ not in seen:
            seen[id_] = 0
            result.append(id_)
        else:
            seen[id_] += 1
            result.append(f"{id_}_{seen[id_]}")

    return result


def _expand_single_spec(spec: ParametrizeSpec) -> list[ExpandedCase]:
    """Expand a single parametrize spec into ExpandedCase entries."""
    argnames = spec.argnames
    argvalues = spec.argvalues
    num_args = len(argnames)

    raw_ids: list[str] = []
    for i, value in enumerate(argvalues):
        if spec.ids is not None and i < len(spec.ids):
            raw_ids.append(spec.ids[i])
        else:
            raw_ids.append(str(i))

    unique_ids = _deduplicate_ids(raw_ids)

    entries: list[ExpandedCase] = []
    for i, value in enumerate(argvalues):
        token = unique_ids[i]

        if num_args == 1:
            kwargs: dict[str, object] = {argnames[0]: value}
        else:
            if not isinstance(value, (list, tuple)):
                raise ValueError(
                    f"For multiple argnames {argnames}, argvalue must be a tuple/list, "
                    f"got {type(value).__name__}: {value!r}"
                )
            if len(value) != num_args:
                raise ValueError(f"Expected {num_args} values for argnames {argnames}, got {len(value)}: {value!r}")
            kwargs = dict(zip(argnames, value))

        entries.append(ExpandedCase(token, kwargs))

    return entries


def _extract_pytest_marker_args(mark_args: tuple[object, ...]) -> PytestMarkerArgs:
    """Extract argnames and argvalues from pytest marker args.

    Returns:
        PytestMarkerArgs with defaults for missing values.
    """
    if len(mark_args) >= 2:
        return PytestMarkerArgs(mark_args[0], mark_args[1])
    elif len(mark_args) == 1:
        return PytestMarkerArgs(mark_args[0], [])
    return PytestMarkerArgs("", [])


def _get_parametrize_specs(func: FunctionType) -> list[ParametrizeSpec]:
    """Get parametrize specs from rtest or pytest markers."""
    # Check rtest markers first
    rtest_specs: list[ParametrizeSpec] = getattr(func, "__rtest_cases__", [])
    if rtest_specs:
        return rtest_specs

    # Check pytest markers (with deprecation warning)
    specs: list[ParametrizeSpec] = []
    pytest_marks: list[object] = getattr(func, "pytestmark", [])
    for pytest_mark in pytest_marks:
        if getattr(pytest_mark, "name", None) == "parametrize":
            warnings.warn(
                PARAMETRIZE_DEPRECATION_MSG.format(func_name=func.__name__),
                DeprecationWarning,
                stacklevel=4,
            )
            # Convert pytest marker to ParametrizeSpec
            mark_args: tuple[object, ...] = getattr(pytest_mark, "args", ())
            mark_kwargs: MappingProxyType[str, object] = getattr(pytest_mark, "kwargs", MappingProxyType({}))
            argnames_raw, argvalues_raw = _extract_pytest_marker_args(mark_args)
            ids_raw: object = mark_kwargs.get("ids")

            if isinstance(argnames_raw, str):
                parsed_argnames = tuple(name.strip() for name in argnames_raw.split(","))
            else:
                # Assume it's a sequence of strings - need type ignore for iteration on object
                parsed_argnames = tuple(str(n) for n in argnames_raw)  # type: ignore[attr-defined]

            # Convert argvalues to tuple
            argvalues_seq: Sequence[object] = argvalues_raw if isinstance(argvalues_raw, (list, tuple)) else []

            # Handle ids
            ids_tuple: tuple[str, ...] | None = None
            if ids_raw is not None and isinstance(ids_raw, (list, tuple)):
                ids_tuple = tuple(str(i) for i in ids_raw)

            specs.append(
                ParametrizeSpec(
                    argnames=parsed_argnames,
                    argvalues=tuple(argvalues_seq),
                    ids=ids_tuple,
                )
            )

    return specs


def expand_parametrize(func: FunctionType) -> list[ExpandedCase]:
    """Expand test cases for a test function.

    If the function has no @rtest.mark.cases decorators, returns a single
    entry with empty case_id and empty kwargs.

    For multiple stacked @cases decorators, computes the cartesian product.

    Args:
        func: The test function (may have __rtest_cases__ attribute).

    Returns:
        List of ExpandedCase entries where:
        - case_id is the string identifier for the test case (e.g., "0", "a-1")
        - kwargs is the dict of arguments to pass to the function
    """
    specs = _get_parametrize_specs(func)

    if not specs:
        return [ExpandedCase("", {})]

    expanded_specs: list[list[ExpandedCase]] = [_expand_single_spec(spec) for spec in specs]

    result: list[ExpandedCase] = []
    for combo in itertools.product(*expanded_specs):
        tokens: list[str] = []
        merged_kwargs: dict[str, object] = {}
        for case in combo:
            tokens.append(case.case_id)
            merged_kwargs.update(case.kwargs)
        result.append(ExpandedCase("-".join(tokens), merged_kwargs))

    return result


def get_case_nodeid_suffix(case_id: str) -> str:
    """Get the nodeid suffix for a case ID.

    Returns empty string if no case_id, otherwise [case_id].
    """
    return f"[{case_id}]" if case_id else ""
