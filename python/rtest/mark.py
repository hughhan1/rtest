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
from collections.abc import Iterable
from dataclasses import dataclass
from types import FunctionType, MappingProxyType
from typing import Callable, NamedTuple, Sequence, TypeVar, cast

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


class ExpandedCase(NamedTuple):
    """A single expanded test case with its ID and kwargs."""

    case_id: str
    kwargs: dict[str, object]


@dataclass(frozen=True)
class SkipInfo:
    reason: str


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

        cases = _expand_cases(parsed_argnames, tuple(argvalues), materialized_ids)

        def decorator(func: F) -> F:
            cases_list: list[tuple[ExpandedCase, ...]] | None = getattr(func, "__rtest_cases__", None)
            if not cases_list:
                cases_list = []
                setattr(func, "__rtest_cases__", cases_list)
            cases_list.append(cases)
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
            setattr(func_or_class, "__rtest_skip__", SkipInfo(reason))
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


def _value_to_id_string(value: object) -> str:
    """Convert a parameter value to its pytest-compatible string ID."""
    if isinstance(value, str):
        return value
    elif value is None or isinstance(value, (int, float, bool)):
        return str(value)
    elif isinstance(value, (list, tuple)):
        return "-".join(_value_to_id_string(v) for v in value)
    elif hasattr(value, "__name__"):
        return str(getattr(value, "__name__"))
    else:
        return repr(value)


def _expand_cases(
    argnames: tuple[str, ...], argvalues: tuple[object, ...], ids: tuple[str, ...] | None
) -> tuple[ExpandedCase, ...]:
    """Validate external rows once, then retain only executable argument mappings."""
    if not argnames or any(not name for name in argnames):
        raise ValueError("argnames must contain at least one nonempty name")
    num_args = len(argnames)

    raw_ids = list(ids or ())[: len(argvalues)]
    raw_ids.extend(_value_to_id_string(value) for value in argvalues[len(raw_ids) :])

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

    return tuple(entries)


def _get_case_groups(func: FunctionType) -> list[tuple[ExpandedCase, ...]]:
    """Get parametrize specs from rtest or pytest markers."""
    # Check rtest markers first
    rtest_specs: list[tuple[ExpandedCase, ...]] = getattr(func, "__rtest_cases__", [])
    if rtest_specs:
        return rtest_specs

    # Check pytest markers (with deprecation warning)
    groups: list[tuple[ExpandedCase, ...]] = []
    pytest_marks: list[object] = getattr(func, "pytestmark", [])
    for pytest_mark in pytest_marks:
        if getattr(pytest_mark, "name", None) == "parametrize":
            warnings.warn(
                PARAMETRIZE_DEPRECATION_MSG.format(func_name=func.__name__),
                DeprecationWarning,
                stacklevel=4,
            )
            # Normalize the external pytest marker into executable cases
            mark_args: tuple[object, ...] = getattr(pytest_mark, "args", ())
            mark_kwargs: MappingProxyType[str, object] = getattr(pytest_mark, "kwargs", MappingProxyType({}))
            args = iter(mark_args)
            argnames_raw, argvalues_raw = next(args, ""), next(args, [])
            ids_raw: object = mark_kwargs.get("ids")

            if isinstance(argnames_raw, str):
                parsed_argnames = tuple(name.strip() for name in argnames_raw.split(","))
            else:
                parsed_argnames = tuple(str(n) for n in cast(Iterable[object], argnames_raw))

            # Convert argvalues to tuple
            argvalues_seq: Sequence[object] = argvalues_raw if isinstance(argvalues_raw, (list, tuple)) else []

            # Handle ids
            ids_tuple: tuple[str, ...] | None = None
            if isinstance(ids_raw, (list, tuple)):
                ids_tuple = tuple(str(i) for i in ids_raw)

            groups.append(_expand_cases(parsed_argnames, tuple(argvalues_seq), ids_tuple))

    return groups


def expand_parametrize(func: FunctionType) -> list[ExpandedCase] | None:
    """Expand test cases for a test function.

    If the function has no @rtest.mark.cases decorators, returns None.
    An empty list means a decorated test has no parameter cases.

    For multiple stacked @cases decorators, computes the cartesian product.

    Args:
        func: The test function (may have __rtest_cases__ attribute).

    Returns:
        List of ExpandedCase entries where:
        - case_id is the string identifier for the test case (e.g., "0", "a-1")
        - kwargs is the dict of arguments to pass to the function
    """
    groups = _get_case_groups(func)
    if not groups:
        return None

    result: list[ExpandedCase] = []
    for combo in itertools.product(*groups):
        tokens: list[str] = []
        merged_kwargs: dict[str, object] = {}
        for case in combo:
            tokens.append(case.case_id)
            merged_kwargs.update(case.kwargs)
        result.append(ExpandedCase("-".join(tokens), merged_kwargs))

    return result
