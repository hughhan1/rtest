"""Fixture tests for test class discovery."""

import rtest


class TestBasicClass:
    """Basic test class without any marks."""

    def test_method1(self) -> None:
        """First test method."""
        assert True

    def test_method2(self) -> None:
        """Second test method."""
        assert 1 + 1 == 2


class TestParametrizedClass:
    """Test class with parametrized methods."""

    @rtest.mark.cases("x", [1, 2])
    def test_param_method(self, x: int) -> None:
        """Parametrized method."""
        assert x > 0

    def test_normal_method(self) -> None:
        """Normal method."""
        assert True


class TestMixedClass:
    """Test class with mixed skip and normal methods."""

    def test_normal(self) -> None:
        """Normal test."""
        assert True

    @rtest.mark.skip(reason="method skip")
    def test_skipped(self) -> None:
        """Skipped test."""
        assert False


class TestWithSetup:
    """Test class with setup/teardown methods."""

    def setup_method(self) -> None:
        self.counter = 0

    def test_uses_setup(self) -> None:
        assert hasattr(self, "counter")
        assert self.counter == 0
