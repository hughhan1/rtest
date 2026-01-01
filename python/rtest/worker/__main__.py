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
        "--python-classes",
        nargs="+",
        default=["Test*"],
        help="Glob patterns for test class names using fnmatch syntax (default: Test*)",
    )
    parser.add_argument(
        "--python-functions",
        nargs="+",
        default=["test*"],
        help="Glob patterns for test function/method names using fnmatch syntax (default: test*)",
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
    python_classes: list[str] = args.python_classes
    python_functions: list[str] = args.python_functions

    return run_tests(
        root=root,
        output_file=output_file,
        test_files=test_files,
        python_classes=python_classes,
        python_functions=python_functions,
    )


if __name__ == "__main__":
    sys.exit(main())
