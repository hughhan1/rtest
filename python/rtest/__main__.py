"""Entry point for python -m rtest."""

import sys

from rtest._rtest import main_cli_with_args


def main() -> None:
    """CLI entry point for rtest."""
    main_cli_with_args(sys.argv[1:])


if __name__ == "__main__":
    main()
