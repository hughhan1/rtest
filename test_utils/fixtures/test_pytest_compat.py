"""Fixture tests for pytest marker compatibility (deprecated)."""

import pytest


@pytest.mark.parametrize("x", [1, 2, 3])
def test_pytest_parametrize(x: int) -> None:
    """Test with pytest.mark.parametrize (deprecated)."""
    assert x > 0


@pytest.mark.skip(reason="pytest skip")
def test_pytest_skip() -> None:
    """Test with pytest.mark.skip (deprecated)."""
    assert False


def test_normal() -> None:
    """Normal test without markers."""
    assert True
