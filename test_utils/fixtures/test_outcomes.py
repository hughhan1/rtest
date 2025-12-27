"""Test fixtures for outcome testing."""

import sys


def test_pass():
    """A test that passes."""
    assert True


def test_fail():
    """A test that fails."""
    assert False


def test_error():
    """A test that raises an exception."""
    raise RuntimeError("intentional error")


def test_pass_with_output():
    """A test that passes and produces output."""
    print("stdout message")
    print("stderr message", file=sys.stderr)
    assert True
