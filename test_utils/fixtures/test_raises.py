"""Test fixtures for rtest.raises() integration testing."""

import rtest


def test_raises_expected():
    with rtest.raises(ValueError):
        raise ValueError("boom")


def test_raises_with_match():
    with rtest.raises(ValueError, match="bo+m"):
        raise ValueError("boom")


def test_raises_no_exception():
    with rtest.raises(ValueError):
        pass


def test_raises_wrong_exception():
    with rtest.raises(ValueError):
        raise TypeError("wrong type")


def test_raises_match_fails():
    with rtest.raises(ValueError, match="xyz"):
        raise ValueError("boom")


def test_raises_value_attribute():
    with rtest.raises(ValueError) as ctx:
        raise ValueError("captured")
    assert ctx.value is not None
    assert str(ctx.value) == "captured"


def test_raises_exception_tuple():
    with rtest.raises((ValueError, TypeError)):
        raise TypeError("either works")


def test_raises_subclass():
    with rtest.raises(Exception):
        raise ValueError("subclass of Exception")
