"""Rustic - A fast Python test runner built with Rust."""

import sys

from rustic._rustic import run_tests

__version__ = "0.1.0"


def main() -> None:
    """CLI entry point for rustic."""
    run_tests(pytest_args=sys.argv[1:])


if __name__ == "__main__":
    main()
