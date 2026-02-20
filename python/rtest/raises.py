"""rtest.raises context manager for testing expected exceptions."""

from __future__ import annotations

import re
from types import TracebackType


class RaisesContext:
    """Context manager that asserts a block of code raises an expected exception.

    Usage::

        with rtest.raises(ValueError, match="invalid"):
            int("not a number")
    """

    def __init__(
        self,
        expected_exception: type[BaseException] | tuple[type[BaseException], ...],
        *,
        match: str | re.Pattern[str] | None = None,
    ) -> None:
        self.expected_exception = expected_exception
        self.match_expr = match
        self.value: BaseException | None = None

        if self.match_expr is not None:
            try:
                re.compile(self.match_expr)
            except re.error as e:
                raise ValueError(f"Invalid regex pattern provided to 'match': {e}") from e

    def __enter__(self) -> RaisesContext:
        return self

    def __exit__(
        self,
        exc_type: type[BaseException] | None,
        exc_val: BaseException | None,
        _exc_tb: TracebackType | None,
    ) -> bool:
        if exc_type is None:
            expected = self.expected_exception
            if isinstance(expected, tuple):
                names = ", ".join(e.__name__ for e in expected)
                raise AssertionError(f"DID NOT RAISE any of ({names})")
            raise AssertionError(f"DID NOT RAISE {expected.__name__}")

        if not issubclass(exc_type, self.expected_exception):
            return False

        if self.match_expr is not None:
            value_str = str(exc_val)
            if not re.search(self.match_expr, value_str):
                raise AssertionError(
                    f"Regex pattern did not match.\n Regex: {self.match_expr!r}\n Input: {value_str!r}"
                ) from exc_val

        self.value = exc_val
        return True


def raises(
    expected_exception: type[BaseException] | tuple[type[BaseException], ...],
    *,
    match: str | re.Pattern[str] | None = None,
) -> RaisesContext:
    """Assert that a block of code raises the expected exception.

    Args:
        expected_exception: The exception type (or tuple of types) expected.
        match: Optional regex pattern to match against the exception message.

    Returns:
        A context manager. After the ``with`` block, access the caught
        exception via the ``.value`` attribute.
    """
    if isinstance(expected_exception, tuple):
        for exc in expected_exception:
            if not isinstance(exc, type) or not issubclass(exc, BaseException):
                raise TypeError(f"{exc!r} is not a valid exception type")
        if not expected_exception:
            raise ValueError("expected_exception must not be empty")
    elif not isinstance(expected_exception, type) or not issubclass(expected_exception, BaseException):
        raise TypeError(f"{expected_exception!r} is not a valid exception type")

    return RaisesContext(expected_exception, match=match)
