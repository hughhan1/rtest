"""Common test helper functions for rtest tests."""

import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import Dict, Iterator, List, Tuple

from test_utils import run_rtest


@contextmanager
def create_test_project(files: Dict[str, str]) -> Iterator[Path]:
    """Create a temporary project with the specified test files.

    Args:
        files: Dictionary mapping file paths to their content

    Yields:
        Path: The temporary project directory path
    """
    with tempfile.TemporaryDirectory() as temp_dir:
        project_path = Path(temp_dir)

        for file_path, content in files.items():
            full_path = project_path / file_path
            full_path.parent.mkdir(parents=True, exist_ok=True)

            if file_path.endswith(".py"):
                full_path.write_text(content)
            else:
                # Handle non-Python files (like README.md)
                full_path.write_bytes(content.encode() if isinstance(content, str) else content)

        yield project_path


def run_collection(project_path: Path) -> Tuple[str, List[str]]:
    """Run test collection and return output and lines.

    Args:
        project_path: Path to the project directory

    Returns:
        Tuple of (full output string, list of output lines)
    """
    output = run_rtest(["--collect-only"], cwd=str(project_path))
    output_lines = output.split("\n")
    return output, output_lines


def assert_tests_found(output_lines: List[str], expected_tests: List[str]) -> None:
    """Assert that all expected tests are found in the output.

    Args:
        output_lines: List of output lines from test collection
        expected_tests: List of expected test patterns
    """
    for test in expected_tests:
        assert any(test in line for line in output_lines), f"Should find test: {test}"


def assert_patterns_not_found(output: str, patterns: List[str]) -> None:
    """Assert that specified patterns are not found in the output.

    Args:
        output: Full output string from test collection
        patterns: List of patterns that should not be found
    """
    for pattern in patterns:
        assert pattern not in output, f"Should not find: {pattern}"


def count_collected_tests(output_lines: List[str]) -> int:
    """Count the number of collected tests from output lines.

    Args:
        output_lines: List of output lines from test collection

    Returns:
        Number of collected tests
    """
    return len([line for line in output_lines if "::" in line and "test_" in line])


def extract_test_lines(output_lines: List[str]) -> List[str]:
    """Extract and clean test lines from output.

    Args:
        output_lines: List of output lines from test collection

    Returns:
        List of cleaned test lines
    """
    return [line.strip() for line in output_lines if "::" in line and "test_" in line]
