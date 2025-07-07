"""Common test utilities for rtest tests."""

import os
import subprocess
import sys
from typing import List, Optional


def run_rtest(args: List[str], cwd: Optional[str] = None) -> str:
    """Helper to run rtest binary and capture output.

    Args:
        args: List of command line arguments
        cwd: Working directory for the subprocess

    Returns:
        str: The stdout output from rtest
    """

    # On Windows, console scripts are installed in the Scripts subdirectory
    if sys.platform == "win32":
        scripts_dir = os.path.join(os.path.dirname(sys.executable), "Scripts")
        rtest_cmd = os.path.join(scripts_dir, "rtest.exe")
    else:
        rtest_cmd = os.path.join(os.path.dirname(sys.executable), "rtest")

    result = subprocess.run(
        [rtest_cmd] + args,
        capture_output=True,
        text=True,
        cwd=cwd,
    )
    return result.stdout
