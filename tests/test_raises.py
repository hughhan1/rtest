"""Unit tests for rtest.raises."""

import re

import rtest
from rtest.raises import RaisesContext, raises


class TestRaisesBasic:
    def test_suppresses_expected_exception(self) -> None:
        with rtest.raises(ValueError):
            raise ValueError("expected")

    def test_sets_value_attribute(self) -> None:
        with rtest.raises(ValueError) as ctx:
            raise ValueError("captured")
        assert isinstance(ctx.value, ValueError)
        assert str(ctx.value) == "captured"

    def test_value_is_none_before_exit(self) -> None:
        ctx = RaisesContext(ValueError)
        assert ctx.value is None

    def test_no_exception_raises_assertion_error(self) -> None:
        with rtest.raises(AssertionError, match="DID NOT RAISE ValueError"):
            with rtest.raises(ValueError):
                pass

    def test_wrong_exception_propagates(self) -> None:
        with rtest.raises(TypeError):
            with rtest.raises(ValueError):
                raise TypeError("wrong")


class TestRaisesMatch:
    def test_match_succeeds(self) -> None:
        with rtest.raises(ValueError, match="boom"):
            raise ValueError("big boom")

    def test_match_uses_re_search(self) -> None:
        with rtest.raises(ValueError, match="oo"):
            raise ValueError("boom")

    def test_match_regex_pattern(self) -> None:
        with rtest.raises(ValueError, match=r"value \d+"):
            raise ValueError("invalid value 42")

    def test_match_fails_raises_assertion_error(self) -> None:
        with rtest.raises(AssertionError, match="Regex pattern did not match"):
            with rtest.raises(ValueError, match="xyz"):
                raise ValueError("boom")

    def test_invalid_regex_raises_immediately(self) -> None:
        with rtest.raises(ValueError, match="Invalid regex pattern"):
            raises(ValueError, match="[invalid")

    def test_compiled_pattern(self) -> None:
        pattern = re.compile(r"bo+m")
        with rtest.raises(ValueError, match=pattern):
            raise ValueError("boom")


class TestRaisesExceptionTypes:
    def test_tuple_of_exceptions(self) -> None:
        with rtest.raises((ValueError, TypeError)):
            raise TypeError("either")

    def test_subclass_matching(self) -> None:
        with rtest.raises(Exception):
            raise ValueError("subclass")

    def test_base_exception(self) -> None:
        with rtest.raises(KeyboardInterrupt):
            raise KeyboardInterrupt


class TestRaisesValidation:
    def test_rejects_non_exception_type(self) -> None:
        with rtest.raises(TypeError, match="is not a valid exception type"):
            raises(str)  # type: ignore[arg-type]

    def test_rejects_non_type(self) -> None:
        with rtest.raises(TypeError, match="is not a valid exception type"):
            raises(42)  # type: ignore[arg-type]

    def test_rejects_empty_tuple(self) -> None:
        with rtest.raises(ValueError, match="must not be empty"):
            raises(())

    def test_rejects_invalid_type_in_tuple(self) -> None:
        with rtest.raises(TypeError, match="is not a valid exception type"):
            raises((ValueError, str))  # type: ignore[arg-type]
