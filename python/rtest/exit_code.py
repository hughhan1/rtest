"""CLI exit codes."""

from typing import Final, Literal

ExitCode = Literal[0, 1, 4]


class ExitCodeValues:
    """CLI exit code values."""

    OK: Final = 0
    """Successful execution."""

    TESTS_FAILED: Final = 1
    """Tests were collected and run but some failed."""

    USAGE_ERROR: Final = 4
    """File or directory not found."""
