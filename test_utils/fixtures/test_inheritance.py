"""Fixture tests for test class inheritance."""


class TestBase:
    """Base test class with inherited methods."""

    def test_inherited(self) -> None:
        """This should be discovered in child classes."""
        assert True


class TestChild(TestBase):
    """Child class that inherits test_inherited."""

    def test_own_method(self) -> None:
        """Child's own test method."""
        assert True

