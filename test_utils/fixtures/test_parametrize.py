"""Fixture tests for parametrize functionality."""

from datetime import datetime

import rtest


def double(x: int) -> int:
    """Helper function for testing function call params."""
    return x * 2


class Point:
    """Helper class for testing object params."""

    def __init__(self, x: int, y: int) -> None:
        self.x = x
        self.y = y


@rtest.mark.cases("x", [1, 2, 3])
def test_single_param(x: int) -> None:
    """Test with single parameter."""
    assert x > 0


@rtest.mark.cases("x,y", [(1, 2), (3, 4), (5, 6)])
def test_multi_param(x: int, y: int) -> None:
    """Test with multiple parameters."""
    assert x < y


@rtest.mark.cases("b", [10, 20])
@rtest.mark.cases("a", [1, 2])
def test_stacked_params(a: int, b: int) -> None:
    """Test with stacked parametrize (cartesian product)."""
    assert a < b


@rtest.mark.cases("x", [1, 2], ids=["one", "two"])
def test_with_ids(x: int) -> None:
    """Test with explicit IDs."""
    assert x > 0


def test_no_params() -> None:
    """Test without parametrize."""
    assert True


@rtest.mark.cases("val", [double(2), double(3), double(4)])
def test_function_call_params(val: int) -> None:
    """Test with function calls as parameter values."""
    assert val in [4, 6, 8]


@rtest.mark.cases("pt", [Point(1, 2), Point(3, 4)])
def test_object_params(pt: Point) -> None:
    """Test with custom object instances as parameters."""
    assert pt.x < pt.y


@rtest.mark.cases("dt", [datetime(2020, 1, 1), datetime(2021, 6, 15)])
def test_stdlib_object_params(dt: datetime) -> None:
    """Test with stdlib objects as parameters."""
    assert dt.year >= 2020

