"""rtest - Python test runner built in Rust."""

import sys

__version__ = "0.1.0"


def main() -> None:
    """CLI entry point for rtest."""
    # Execute the main Rust binary logic directly with proper args
    from rtest._rtest import main_cli_with_args

    main_cli_with_args(sys.argv[1:])


if __name__ == "__main__":
    main()
