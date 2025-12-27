"""CLI entry point for the rtest worker.

Usage:
    python -m rtest.worker --root <repo_root> --out <out.jsonl> <file1.py> <file2.py> ...
"""

import argparse
import sys
from pathlib import Path

from rtest.worker.runner import run_tests


def main() -> int:
    """Run the worker CLI."""
    parser = argparse.ArgumentParser(
        description="rtest worker for native test execution",
        prog="python -m rtest.worker",
    )
    parser.add_argument(
        "--root",
        type=Path,
        required=True,
        help="Repository root path for relative imports",
    )
    parser.add_argument(
        "--out",
        type=Path,
        required=True,
        help="Output JSONL file path for results",
    )
    parser.add_argument(
        "files",
        nargs="+",
        type=Path,
        help="Test files to run",
    )

    args = parser.parse_args()

    # Extract typed values from args
    root: Path = args.root
    output_file: Path = args.out
    test_files: list[Path] = args.files

    return run_tests(
        root=root,
        output_file=output_file,
        test_files=test_files,
    )


if __name__ == "__main__":
    sys.exit(main())
