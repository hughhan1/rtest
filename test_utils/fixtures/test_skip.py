"""Fixture tests for skip functionality."""

import rtest


@rtest.mark.skip(reason="not implemented")
def test_skipped_with_reason() -> None:
    """This test should be skipped."""
    assert False  # Should never run


@rtest.mark.skip()
def test_skipped_without_reason() -> None:
    """This test should also be skipped."""
    assert False


@rtest.mark.skip(reason="entire class skipped")
class TestSkippedClass:
    """All tests in this class should be skipped."""

    def test_method1(self) -> None:
        """Skipped via class."""
        assert False

    def test_method2(self) -> None:
        """Also skipped via class."""
        assert False


def test_not_skipped() -> None:
    """This test should run normally."""
    assert True

