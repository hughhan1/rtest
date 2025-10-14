"""Real parametrized tests for collection testing."""

import pytest


@pytest.mark.parametrize("value", [1, 2, 3])
def test_simple_numbers(value: int) -> None:
    """Simple parametrized test with single parameter."""
    assert value > 0


@pytest.mark.parametrize(
    "x,y,expected",
    [
        (1, 2, 3),
        (5, 5, 10),
        (10, -5, 5),
    ],
)
def test_addition(x: int, y: int, expected: int) -> None:
    """Parametrized test with multiple parameters."""
    assert x + y == expected


@pytest.mark.parametrize(
    "x,y, expected",
    [
        (1, 2, 3),
        (5, 5, 10),
        (10, -5, 5),
    ],
)
def test_whitespace(x: int, y: int, expected: int) -> None:
    """Parametrized test with multiple parameters."""
    assert x + y == expected


@pytest.mark.parametrize(
    ("x", "y", "expected"),
    [
        (1, 2, 3),
        (5, 5, 10),
        (10, -5, 5),
    ],
)
def test_typle_format(x: int, y: int, expected: int) -> None:
    """Parametrized test with multiple parameters."""
    assert x + y == expected


@pytest.mark.parametrize("value", [1, 2])
@pytest.mark.parametrize("multiplier", [10, 20])
def test_stacked(value: int, multiplier: int) -> None:
    """Test with stacked parametrize decorators."""
    assert value * multiplier > 0


class TestParametrizedClass:
    """Test class with parametrized methods."""

    @pytest.mark.parametrize("name", ["alice", "bob", "charlie"])
    def test_names(self, name: str) -> None:
        """Parametrized test method in a class."""
        assert len(name) > 0

    def test_regular(self) -> None:
        """Regular non-parametrized test for comparison."""
        assert True


def test_regular_function() -> None:
    """Regular non-parametrized function for comparison."""
    assert True
