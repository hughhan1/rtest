"""Common test helper functions for rtest tests."""

import tempfile
from contextlib import contextmanager
from pathlib import Path
from typing import Dict, Iterator, List, Optional

from test_utils import run_rtest


class CollectionResult:
    """Result from running test collection.

    Provides flexible access to output as either string or list of lines.
    """

    def __init__(self, returncode: int, stdout: str, stderr: str = ""):
        self.returncode = returncode
        self.stdout = stdout
        self.stderr = stderr
        self._output = stdout + stderr if stderr else stdout
        self._output_lines = self._output.split("\n")

    @property
    def output(self) -> str:
        """Get output as a single string."""
        return self._output

    @property
    def output_lines(self) -> List[str]:
        """Get output as a list of lines."""
        return self._output_lines


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


def run_collection(project_path: Path, env: Optional[Dict[str, str]] = None) -> CollectionResult:
    """Run test collection and return result with flexible output access.

    Args:
        project_path: Path to the project directory
        env: Optional environment variables to use

    Returns:
        CollectionResult with returncode, output as string, and output as lines
    """
    returncode, stdout, stderr = run_rtest(["--collect-only"], cwd=str(project_path), env=env)
    return CollectionResult(returncode, stdout, stderr)


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
