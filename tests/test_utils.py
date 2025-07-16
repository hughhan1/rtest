"""Common test utilities for rtest tests."""

import os
import subprocess
import sys
from typing import Dict, List, Optional


def run_rtest(args: List[str], cwd: Optional[str] = None, env: Optional[Dict[str, str]] = None) -> tuple[int, str, str]:
    """Helper to run rtest binary and capture output.

    Args:
        args: List of command line arguments
        cwd: Working directory for the subprocess
        env: Environment variables to use (if None, uses current environment)

    Returns:
        tuple: (returncode, stdout, stderr)
    """

    # On Windows, console scripts are installed in the Scripts subdirectory
    if sys.platform == "win32":
        exe_dir = os.path.dirname(sys.executable)
        # Check if we're already in Scripts directory
        if os.path.basename(exe_dir).lower() == "scripts":
            scripts_dir = exe_dir
        else:
            scripts_dir = os.path.join(exe_dir, "Scripts")
        rtest_cmd = os.path.join(scripts_dir, "rtest.exe")
    else:
        rtest_cmd = os.path.join(os.path.dirname(sys.executable), "rtest")

    print(f"Running rtest command: {rtest_cmd} with args: {args} in cwd: {cwd}")

    result = subprocess.run(
        [rtest_cmd] + args,
        capture_output=True,
        text=True,
        cwd=cwd,
        env=env,
    )
    return result.returncode, result.stdout, result.stderr
