"""Test file to compare parametrized test collection between pytest and rtest."""
import pytest


@pytest.mark.parametrize("value", [1, 2, 3])
def test_simple_parametrize(value):
    """Simple parametrized test with integers."""
    assert value > 0


@pytest.mark.parametrize("x,y,expected", [
    (1, 2, 3),
    (5, 5, 10),
    (10, -5, 5),
])
def test_multiple_params(x, y, expected):
    """Parametrized test with multiple parameters."""
    assert x + y == expected


@pytest.mark.parametrize("value", [1, 2, 3])
@pytest.mark.parametrize("multiplier", [10, 20])
def test_stacked_parametrize(value, multiplier):
    """Test with stacked parametrize decorators."""
    assert value * multiplier > 0


class TestParametrizedClass:
    """Test class with parametrized methods."""

    @pytest.mark.parametrize("name", ["alice", "bob", "charlie"])
    def test_names(self, name):
        """Parametrized test in a class."""
        assert len(name) > 0
