#!/usr/bin/env python3
# mypy: ignore-errors
"""
Benchmarking script for rtest vs pytest across multiple repositories.

This module provides functionality to clone popular Python repositories,
set up test environments, and benchmark test collection and execution
performance between rtest and pytest using hyperfine.
"""

import argparse
import json
import logging
import os
import platform
import shlex
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass, field
from pathlib import Path
from typing import TypedDict, cast

import yaml


# Type definitions for YAML data
class SetupConfigData(TypedDict, total=False):
    """Type for repository setup configuration from YAML."""

    method: str
    dependency_groups: list[str]
    extra_packages: list[str]
    pip_extras: list[str]  # e.g., ["test", "dev"] for `pip install -e ".[test,dev]"`
    requirements_file: str  # custom requirements file path, e.g., "requirements-tests.txt"


class RepositoryData(TypedDict):
    """Type for repository data from YAML."""

    name: str
    url: str
    category: str
    test_dir: str
    setup: SetupConfigData
    python_version: str


class BenchmarkConfigData(TypedDict, total=False):
    """Type for benchmark config data from YAML."""

    description: str
    pytest_args: str
    rtest_args: str
    timeout: int


class SettingsData(TypedDict, total=False):
    """Type for global settings from YAML."""

    validation_timeout: int


class ConfigData(TypedDict):
    """Type for the full configuration data from YAML."""

    repositories: list[RepositoryData]
    benchmark_configs: dict[str, BenchmarkConfigData]
    settings: SettingsData


# Constants
DEFAULT_TIMEOUT = 300
DEFAULT_VALIDATION_TIMEOUT = 120

# Hyperfine settings (can be overridden by environment variables)
HYPERFINE_MIN_RUNS = int(os.environ.get("HYPERFINE_MIN_RUNS", "20"))
HYPERFINE_MAX_RUNS = int(os.environ.get("HYPERFINE_MAX_RUNS", "20"))
HYPERFINE_WARMUP = int(os.environ.get("HYPERFINE_WARMUP", "3"))

# Configure logging
logging.basicConfig(level=logging.INFO, format="%(asctime)s - %(levelname)s - %(message)s", datefmt="%Y-%m-%d %H:%M:%S")
logger = logging.getLogger(__name__)


def get_venv_executable(venv_path: Path, name: str) -> Path:
    """Get path to an executable in a virtualenv, handling platform differences."""
    bin_dir = "Scripts" if platform.system() == "Windows" else "bin"
    return venv_path / bin_dir / name


@dataclass
class SetupOptions:
    """Configuration for repository setup."""

    method: str  # "uv_sync", "uv_pip_install", "requirements_file"
    dependency_groups: list[str] = field(default_factory=list)
    extra_packages: list[str] = field(default_factory=list)
    pip_extras: list[str] = field(default_factory=list)  # e.g., ["test"] for `pip install -e ".[test]"`
    requirements_file: str = "requirements.txt"  # custom requirements file path

    @classmethod
    def from_dict(cls, data: SetupConfigData) -> "SetupOptions":
        return cls(
            method=data.get("method", "uv_sync"),
            dependency_groups=data.get("dependency_groups", []),
            extra_packages=data.get("extra_packages", []),
            pip_extras=data.get("pip_extras", []),
            requirements_file=data.get("requirements_file", "requirements.txt"),
        )


@dataclass
class RepositoryConfig:
    """Configuration for a repository to benchmark."""

    name: str
    url: str
    category: str
    test_dir: str
    setup: SetupOptions
    python_version: str


@dataclass
class BenchmarkConfig:
    """Configuration for a benchmark type."""

    description: str
    pytest_args: str
    rtest_args: str
    timeout: int = DEFAULT_TIMEOUT


@dataclass
class GlobalSettings:
    """Global benchmark settings from YAML."""

    validation_timeout: int = DEFAULT_VALIDATION_TIMEOUT


@dataclass
class HyperfineResult:
    """Result from a hyperfine benchmark run."""

    mean: float
    stddev: float
    times: list[float]


@dataclass
class MemoryResult:
    """Memory usage result."""

    peak_rss_kb: int  # Peak resident set size in KB
    command: str


@dataclass
class StartupTimeResult:
    """Startup time benchmark result."""

    pytest_mean: float
    pytest_stddev: float
    rtest_mean: float
    rtest_stddev: float
    speedup: float | None


@dataclass
class BenchmarkResult:
    """Result from benchmarking a repository."""

    repository: str
    benchmark: str
    pytest: HyperfineResult
    rtest: HyperfineResult
    speedup: float | None
    # Optional memory metrics
    pytest_memory_kb: int | None = None
    rtest_memory_kb: int | None = None


@dataclass
class ErrorResult:
    """Error result when benchmark fails."""

    repository: str
    benchmark: str
    error: str
    command: str | None = None
    exit_code: int | None = None
    stdout: str | None = None
    stderr: str | None = None


class ConfigLoader:
    """Handles loading and validation of configuration files."""

    @staticmethod
    def load_config(config_path: Path) -> tuple[list[RepositoryConfig], dict[str, BenchmarkConfig], GlobalSettings]:
        """Load and validate configuration from YAML file."""
        if not config_path.exists():
            raise FileNotFoundError(f"Configuration file not found: {config_path}")

        try:
            with open(config_path, "r") as f:
                raw_data = cast(ConfigData, yaml.safe_load(f))
        except yaml.YAMLError as e:
            raise ValueError(f"Invalid YAML in configuration file: {e}")

        if not isinstance(raw_data, dict):
            raise ValueError("Configuration must be a dictionary")

        # Validate and convert repositories
        repositories: list[RepositoryConfig] = []
        raw_repos: list[RepositoryData] = raw_data.get("repositories", [])
        if not isinstance(raw_repos, list):
            raise ValueError("Repositories must be a list")

        for repo_data in raw_repos:
            if not isinstance(repo_data, dict):
                raise ValueError(f"Invalid repository configuration: {repo_data}")
            if not all(key in repo_data for key in ["name", "url", "category", "test_dir"]):
                raise ValueError(f"Invalid repository configuration: {repo_data}")

            # Parse setup options with defaults
            setup_data = repo_data.get("setup", {})
            if not isinstance(setup_data, dict):
                setup_data = {}

            repositories.append(
                RepositoryConfig(
                    name=repo_data["name"],
                    url=repo_data["url"],
                    category=repo_data["category"],
                    test_dir=repo_data["test_dir"],
                    setup=SetupOptions.from_dict(cast(SetupConfigData, setup_data)),
                    python_version=repo_data.get("python_version", "3.9"),
                )
            )

        # Validate and convert benchmark configs
        benchmark_configs: dict[str, BenchmarkConfig] = {}
        raw_benchmarks: dict[str, BenchmarkConfigData] = raw_data.get("benchmark_configs", {})
        if not isinstance(raw_benchmarks, dict):
            raise ValueError("Benchmark configs must be a dictionary")

        for name, config_data in raw_benchmarks.items():
            if not isinstance(config_data, dict):
                raise ValueError(f"Invalid benchmark configuration: {config_data}")
            if not all(key in config_data for key in ["description", "pytest_args", "rtest_args"]):
                raise ValueError(f"Invalid benchmark configuration: {config_data}")
            benchmark_configs[name] = BenchmarkConfig(
                description=config_data["description"],
                pytest_args=config_data["pytest_args"],
                rtest_args=config_data["rtest_args"],
                timeout=config_data.get("timeout", DEFAULT_TIMEOUT),
            )

        # Parse global settings
        raw_settings = raw_data.get("settings", {})
        if not isinstance(raw_settings, dict):
            raw_settings = {}

        settings = GlobalSettings(
            validation_timeout=raw_settings.get("validation_timeout", DEFAULT_VALIDATION_TIMEOUT),
        )

        return (repositories, benchmark_configs, settings)


def run_command(cmd: list[str], cwd: str, timeout: int = DEFAULT_TIMEOUT) -> subprocess.CompletedProcess[str]:
    """Execute a command and return the result."""
    logger.debug(f"Running command: {' '.join(cmd)} in {cwd}")
    try:
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        result: subprocess.CompletedProcess[str] = subprocess.CompletedProcess(cmd, -1)
        result.stdout = ""
        result.stderr = f"Command timed out after {timeout}s"
        return result


class RepositoryManager:
    """Manages repository cloning and setup."""

    def __init__(self, temp_dir: Path, project_root: Path, rtest_source: str):
        self.temp_dir = temp_dir
        self.project_root = project_root
        self.rtest_source = rtest_source

    def clone_repository(self, repo_config: RepositoryConfig) -> Path | None:
        """Clone a repository to temporary directory."""
        repo_path = self.temp_dir / repo_config.name

        # Clean up pre-existing directory
        if repo_path.exists():
            logger.info(f"[CLONE] Removing existing directory for {repo_config.name}")
            shutil.rmtree(repo_path)

        logger.info(f"[CLONE] Cloning {repo_config.name} from {repo_config.url}...")

        # Clone without --recurse-submodules (faster, more reliable for benchmarking)
        result = run_command(
            ["git", "clone", "--depth", "1", repo_config.url, str(repo_path)],
            str(self.temp_dir),
            timeout=300,
        )

        if result.returncode != 0:
            logger.error(f"[CLONE] Failed to clone {repo_config.name}: {result.stderr}")
            return None

        logger.info(f"[CLONE] Cloned {repo_config.name}")
        return repo_path

    def setup_repository(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Set up repository dependencies based on its configuration."""
        logger.info(f"[SETUP] Setting up {repo_config.name} with method: {repo_config.setup.method}")

        method = repo_config.setup.method

        # For uv_sync, let uv manage its own venv
        # For other methods, we create the venv ourselves
        if method == "uv_sync":
            # Step 1: Install dependencies (uv sync creates and manages .venv)
            if not self._install_dependencies(repo_config, repo_path):
                return False

            # Step 2: Install pip extras (e.g., .[test]) into the project's venv
            if not self._install_pip_extras(repo_config, repo_path):
                return False

            # Step 3: Install extra packages into the project's venv
            if not self._install_extra_packages(repo_config, repo_path):
                return False

            # Step 4: Install pytest and rtest into the project's venv
            if not self._install_test_tools(repo_config, repo_path):
                return False
        else:
            # Step 1: Create venv with uv
            if not self._create_venv(repo_config, repo_path):
                return False

            # Step 2: Install repository dependencies based on method
            if not self._install_dependencies(repo_config, repo_path):
                return False

            # Step 3: Install extra packages
            if not self._install_extra_packages(repo_config, repo_path):
                return False

            # Step 4: Install pytest and rtest
            if not self._install_test_tools(repo_config, repo_path):
                return False

        # Final step: Validate installation
        if not self._validate_installation(repo_config, repo_path):
            return False

        logger.info(f"[SETUP] Successfully set up {repo_config.name}")
        return True

    def _create_venv(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Create virtual environment using uv."""
        venv_path = repo_path / ".venv"

        # Remove existing venv
        if venv_path.exists():
            logger.info(f"[VENV] Removing existing .venv for {repo_config.name}")
            shutil.rmtree(venv_path)

        logger.info(f"[VENV] Creating venv with Python {repo_config.python_version}")
        result = run_command(
            ["uv", "venv", "--python", repo_config.python_version, str(venv_path)],
            str(repo_path),
            timeout=60,
        )

        if result.returncode != 0:
            logger.error(f"[VENV] Failed to create venv: {result.stderr}")
            return False

        return True

    def _install_dependencies(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Install repository dependencies based on setup method."""
        method = repo_config.setup.method

        if method == "uv_sync":
            return self._setup_with_uv_sync(repo_config, repo_path)
        elif method == "uv_pip_install":
            return self._setup_with_uv_pip_install(repo_config, repo_path)
        elif method == "requirements_file":
            return self._setup_with_requirements_file(repo_config, repo_path)
        else:
            logger.error(f"[DEPS] Unknown setup method: {method}")
            return False

    def _setup_with_uv_sync(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Set up using uv sync (for projects with pyproject.toml)."""
        # Build base command
        cmd = ["uv", "sync", "--frozen"]

        # Add dependency groups (or --all-groups if none specified)
        if repo_config.setup.dependency_groups:
            for group in repo_config.setup.dependency_groups:
                cmd.extend(["--group", group])
        else:
            # Install all groups by default to get test dependencies
            cmd.append("--all-groups")

        logger.info(f"[DEPS] Running: {' '.join(cmd)}")
        result = run_command(cmd, str(repo_path), timeout=300)

        if result.returncode != 0:
            # Fallback: try without --frozen if lockfile doesn't exist
            logger.warning("[DEPS] uv sync --frozen failed, trying without --frozen")
            cmd = ["uv", "sync"]
            if repo_config.setup.dependency_groups:
                for group in repo_config.setup.dependency_groups:
                    cmd.extend(["--group", group])
            else:
                cmd.append("--all-groups")

            logger.info(f"[DEPS] Running: {' '.join(cmd)}")
            result = run_command(cmd, str(repo_path), timeout=300)

            if result.returncode != 0:
                logger.error(f"[DEPS] uv sync failed: {result.stderr}")
                return False

        return True

    def _setup_with_uv_pip_install(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Set up using uv pip install for the project."""
        venv_path = repo_path / ".venv"

        # Install the project in editable mode
        result = run_command(
            ["uv", "pip", "install", "--python", str(venv_path / "bin" / "python"), "-e", "."],
            str(repo_path),
            timeout=300,
        )

        if result.returncode != 0:
            logger.error(f"[DEPS] uv pip install failed: {result.stderr}")
            return False

        return True

    def _setup_with_requirements_file(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Set up using a requirements file (default: requirements.txt)."""
        requirements_filename = repo_config.setup.requirements_file
        requirements_path = repo_path / requirements_filename
        venv_path = repo_path / ".venv"

        if not requirements_path.exists():
            logger.warning(f"[DEPS] No {requirements_filename} found for {repo_config.name}")
            return True  # Not a failure, just nothing to install

        logger.info(f"[DEPS] Installing from {requirements_filename}")
        result = run_command(
            ["uv", "pip", "install", "--python", str(venv_path / "bin" / "python"), "-r", requirements_filename],
            str(repo_path),
            timeout=300,
        )

        if result.returncode != 0:
            logger.error(f"[DEPS] Failed to install requirements: {result.stderr}")
            return False

        return True

    def _install_pip_extras(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Install the project with pip extras (e.g., .[test,dev])."""
        pip_extras = repo_config.setup.pip_extras
        if not pip_extras:
            return True

        venv_path = repo_path / ".venv"
        extras_str = ",".join(pip_extras)
        install_spec = f".[{extras_str}]"

        logger.info(f"[DEPS] Installing project with extras: {install_spec}")
        result = run_command(
            ["uv", "pip", "install", "--python", str(venv_path / "bin" / "python"), "-e", install_spec],
            str(repo_path),
            timeout=300,
        )

        if result.returncode != 0:
            logger.error(f"[DEPS] Failed to install pip extras: {result.stderr}")
            return False

        return True

    def _install_extra_packages(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Install any extra packages specified in config."""
        extra_packages = repo_config.setup.extra_packages
        if not extra_packages:
            return True

        venv_path = repo_path / ".venv"

        logger.info(f"[DEPS] Installing extra packages: {extra_packages}")
        result = run_command(
            ["uv", "pip", "install", "--python", str(venv_path / "bin" / "python")] + extra_packages,
            str(repo_path),
            timeout=120,
        )

        if result.returncode != 0:
            logger.error(f"[DEPS] Failed to install extra packages: {result.stderr}")
            return False

        return True

    def _install_test_tools(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Install pytest and rtest."""
        venv_path = repo_path / ".venv"
        packages = ["pytest", "pytest-xdist"]

        # Determine rtest source
        if self.rtest_source == "local":
            rtest_spec = f"rtest @ file://{self.project_root}"
        elif self.rtest_source.startswith("wheel:"):
            # Install from pre-built wheel file
            wheel_path = Path(self.rtest_source[6:]).absolute()
            if not wheel_path.exists():
                logger.error(f"[TOOLS] Wheel file not found: {wheel_path}")
                return False
            rtest_spec = str(wheel_path)
        else:
            # Treat as a version specifier
            rtest_spec = f"rtest=={self.rtest_source}"

        packages.append(rtest_spec)

        logger.info(f"[TOOLS] Installing test tools: {packages}")
        result = run_command(
            ["uv", "pip", "install", "--python", str(venv_path / "bin" / "python")] + packages,
            str(repo_path),
            timeout=300,  # Longer timeout for local builds
        )

        if result.returncode != 0:
            logger.error(f"[TOOLS] Failed to install test tools: {result.stderr}")
            return False

        return True

    def _validate_installation(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Validate that pytest and rtest are installed and working."""
        venv_path = repo_path / ".venv"
        pytest_exe = get_venv_executable(venv_path, "pytest")
        rtest_exe = get_venv_executable(venv_path, "rtest")

        # Check executables exist
        if not pytest_exe.exists():
            logger.error(f"[VALIDATE] pytest executable not found at {pytest_exe}")
            return False

        if not rtest_exe.exists():
            logger.error(f"[VALIDATE] rtest executable not found at {rtest_exe}")
            return False

        # Verify pytest version
        result = run_command([str(pytest_exe), "--version"], str(repo_path), timeout=30)
        if result.returncode != 0:
            logger.error(f"[VALIDATE] pytest --version failed: {result.stderr}")
            return False
        logger.info(f"[VALIDATE] pytest: {result.stdout.strip().split(chr(10))[0]}")

        # Verify rtest version
        result = run_command([str(rtest_exe), "--version"], str(repo_path), timeout=30)
        if result.returncode != 0:
            logger.error(f"[VALIDATE] rtest --version failed: {result.stderr}")
            return False
        logger.info(f"[VALIDATE] rtest: {result.stdout.strip().split(chr(10))[0]}")

        return True


class HyperfineRunner:
    """Runs benchmarks using hyperfine."""

    def __init__(self, validation_timeout: int = DEFAULT_VALIDATION_TIMEOUT):
        self.validation_timeout = validation_timeout

    def measure_memory(self, cmd: str, cwd: Path, timeout: int = DEFAULT_TIMEOUT) -> int | None:
        """Measure peak memory usage of a command using /usr/bin/time.

        Returns peak RSS in KB, or None if measurement failed.
        """
        if platform.system() != "Linux":
            return None

        try:
            # Use GNU time with verbose output to get memory stats
            time_cmd = f"/usr/bin/time -v {cmd}"
            result = subprocess.run(time_cmd, shell=True, cwd=str(cwd), capture_output=True, text=True, timeout=timeout)
            # GNU time outputs to stderr
            for line in result.stderr.split("\n"):
                if "Maximum resident set size" in line:
                    # Extract the number from the line
                    parts = line.split(":")
                    if len(parts) >= 2:
                        return int(parts[1].strip())
        except (subprocess.TimeoutExpired, ValueError, IndexError):
            pass
        return None

    def run_startup_benchmark(self, repo_config: RepositoryConfig, repo_path: Path) -> StartupTimeResult | ErrorResult:
        """Benchmark CLI startup time (--version command)."""
        venv_path = repo_path / ".venv"
        pytest_executable = get_venv_executable(venv_path, "pytest")
        rtest_executable = get_venv_executable(venv_path, "rtest")

        if not pytest_executable.exists() or not rtest_executable.exists():
            return ErrorResult(
                repository=repo_config.name,
                benchmark="Startup time",
                error="Executables not found",
            )

        logger.info(f"[STARTUP] Benchmarking startup time for {repo_config.name}")

        # Build commands
        pytest_cmd = f"{shlex.quote(str(pytest_executable))} --version"
        rtest_cmd = f"{shlex.quote(str(rtest_executable))} --version"

        json_output = f"{repo_config.name}_startup.json"
        hyperfine_cmd = [
            "hyperfine",
            "--warmup",
            str(HYPERFINE_WARMUP),
            "--min-runs",
            str(HYPERFINE_MIN_RUNS),
            "--max-runs",
            str(HYPERFINE_MAX_RUNS),
            "--export-json",
            json_output,
            "--command-name",
            "pytest",
            "--command-name",
            "rtest",
            pytest_cmd,
            rtest_cmd,
        ]

        result = run_command(hyperfine_cmd, str(repo_path), timeout=120)
        if result.returncode != 0:
            return ErrorResult(
                repository=repo_config.name,
                benchmark="Startup time",
                error=f"Hyperfine failed: {result.stderr}",
            )

        # Parse results
        json_path = repo_path / json_output
        try:
            with open(json_path) as f:
                data = json.load(f)

            results = data["results"]
            pytest_data = results[0]
            rtest_data = results[1]

            pytest_mean = float(pytest_data["mean"])
            pytest_stddev = float(pytest_data["stddev"])
            rtest_mean = float(rtest_data["mean"])
            rtest_stddev = float(rtest_data["stddev"])
            speedup = pytest_mean / rtest_mean if rtest_mean > 0 else None

            return StartupTimeResult(
                pytest_mean=pytest_mean,
                pytest_stddev=pytest_stddev,
                rtest_mean=rtest_mean,
                rtest_stddev=rtest_stddev,
                speedup=speedup,
            )
        except (FileNotFoundError, json.JSONDecodeError, KeyError, IndexError) as e:
            return ErrorResult(
                repository=repo_config.name,
                benchmark="Startup time",
                error=f"Failed to parse results: {e}",
            )
        finally:
            if json_path.exists():
                json_path.unlink()

    def run_benchmark(
        self,
        repo_config: RepositoryConfig,
        repo_path: Path,
        benchmark_config: BenchmarkConfig,
        show_output: bool = False,
        ignore_failures: bool = False,
    ) -> BenchmarkResult | ErrorResult:
        """Run a benchmark using hyperfine."""
        test_dir_path = repo_path / repo_config.test_dir
        if not test_dir_path.exists():
            return ErrorResult(
                repository=repo_config.name,
                benchmark=benchmark_config.description,
                error=f"Test directory '{repo_config.test_dir}' does not exist",
            )

        logger.info(f"[BENCHMARK] Running {benchmark_config.description} on {repo_config.name}")

        # Get executable paths
        venv_path = repo_path / ".venv"
        pytest_executable = get_venv_executable(venv_path, "pytest")
        rtest_executable = get_venv_executable(venv_path, "rtest")

        # Verify executables exist before benchmarking
        if not pytest_executable.exists():
            return ErrorResult(
                repository=repo_config.name,
                benchmark=benchmark_config.description,
                error=f"pytest executable not found: {pytest_executable}",
            )

        if not rtest_executable.exists():
            return ErrorResult(
                repository=repo_config.name,
                benchmark=benchmark_config.description,
                error=f"rtest executable not found: {rtest_executable}",
            )

        # Build commands with proper quoting
        pytest_cmd = self._build_command(str(pytest_executable), benchmark_config.pytest_args, repo_config.test_dir)
        rtest_cmd = self._build_command(str(rtest_executable), benchmark_config.rtest_args, repo_config.test_dir)

        # Pre-flight validation with configurable timeout
        if not ignore_failures:
            for name, cmd in [("pytest", pytest_cmd), ("rtest", rtest_cmd)]:
                ok, stdout, stderr, code = self._validate_command(cmd, repo_path, timeout=self.validation_timeout)
                if not ok:
                    logger.error(f"[BENCHMARK] {name} command failed validation for {repo_config.name}")
                    return ErrorResult(
                        repository=repo_config.name,
                        benchmark=benchmark_config.description,
                        error=f"{name} command failed during validation (exit {code})",
                        command=cmd,
                        exit_code=code,
                        stdout=stdout,
                        stderr=stderr,
                    )

        # Run hyperfine
        json_output = f"{repo_config.name}_{benchmark_config.description.replace(' ', '_')}.json"
        hyperfine_cmd = self._build_hyperfine_command(pytest_cmd, rtest_cmd, json_output, show_output, ignore_failures)

        result = run_command(hyperfine_cmd, str(repo_path), timeout=benchmark_config.timeout)

        if result.returncode != 0:
            logger.error(f"[BENCHMARK] Hyperfine failed for {repo_config.name}:")
            if result.stderr:
                logger.error(f"  stderr: {result.stderr}")
            return ErrorResult(
                repository=repo_config.name,
                benchmark=benchmark_config.description,
                error=f"Hyperfine failed: {result.stderr}",
                command=" ".join(hyperfine_cmd),
                exit_code=result.returncode,
                stdout=result.stdout,
                stderr=result.stderr,
            )

        # Measure memory usage (Linux only)
        pytest_memory = self.measure_memory(pytest_cmd, repo_path, timeout=self.validation_timeout)
        rtest_memory = self.measure_memory(rtest_cmd, repo_path, timeout=self.validation_timeout)

        if pytest_memory:
            logger.info(f"[MEMORY] pytest peak RSS: {pytest_memory} KB")
        if rtest_memory:
            logger.info(f"[MEMORY] rtest peak RSS: {rtest_memory} KB")

        # Parse results with proper command name matching
        return self._parse_results(
            repo_path,
            json_output,
            repo_config.name,
            benchmark_config.description,
            pytest_memory_kb=pytest_memory,
            rtest_memory_kb=rtest_memory,
        )

    def _build_command(self, executable: str, args: str, test_dir: str) -> str:
        """Build command string for hyperfine with proper quoting."""
        # Parse args properly (handles quoted strings, etc.)
        arg_list = shlex.split(args) if args else []

        # Build and properly quote the command
        cmd_parts = [shlex.quote(executable)] + [shlex.quote(a) for a in arg_list] + [shlex.quote(test_dir)]
        return " ".join(cmd_parts)

    def _validate_command(
        self, cmd: str, cwd: Path, timeout: int = DEFAULT_VALIDATION_TIMEOUT
    ) -> tuple[bool, str | None, str | None, int]:
        """Run command once to validate it works before benchmarking.

        Returns: (success, stdout, stderr, exit_code)
        """
        try:
            result = subprocess.run(cmd, shell=True, cwd=str(cwd), capture_output=True, text=True, timeout=timeout)
            return (result.returncode == 0, result.stdout, result.stderr, result.returncode)
        except subprocess.TimeoutExpired:
            return (False, None, f"Command timed out after {timeout}s", -1)

    def _build_hyperfine_command(
        self,
        pytest_cmd: str,
        rtest_cmd: str,
        json_output: str,
        show_output: bool = False,
        ignore_failures: bool = False,
    ) -> list[str]:
        """Build hyperfine command."""
        cmd = [
            "hyperfine",
            "--warmup",
            str(HYPERFINE_WARMUP),
            "--min-runs",
            str(HYPERFINE_MIN_RUNS),
            "--max-runs",
            str(HYPERFINE_MAX_RUNS),
            "--export-json",
            json_output,
            "--command-name",
            "pytest",
            "--command-name",
            "rtest",
        ]

        if show_output:
            cmd.append("--show-output")
        if ignore_failures:
            cmd.append("-i")

        cmd.extend([pytest_cmd, rtest_cmd])
        return cmd

    def _parse_results(
        self,
        repo_path: Path,
        json_output: str,
        repo_name: str,
        benchmark_desc: str,
        pytest_memory_kb: int | None = None,
        rtest_memory_kb: int | None = None,
    ) -> BenchmarkResult | ErrorResult:
        """Parse hyperfine JSON output with proper command name matching."""
        json_path = repo_path / json_output

        try:
            with open(json_path) as f:
                data = json.load(f)

            results = data["results"]

            # Match results by command name, not by position
            pytest_data = None
            rtest_data = None

            for result in results:
                # hyperfine uses "command" field for the actual command
                # and we can match by our --command-name labels
                command_name = result.get("command", "")

                # Check if this result matches pytest or rtest
                # hyperfine stores our --command-name in the result
                if "pytest" in command_name.lower() and "rtest" not in command_name.lower():
                    pytest_data = result
                elif "rtest" in command_name.lower():
                    rtest_data = result

            # If we couldn't match by command content, fall back to order
            # (hyperfine preserves order of --command-name args)
            if pytest_data is None and len(results) >= 1:
                pytest_data = results[0]
            if rtest_data is None and len(results) >= 2:
                rtest_data = results[1]

            if pytest_data is None:
                return ErrorResult(
                    repository=repo_name,
                    benchmark=benchmark_desc,
                    error="Could not find pytest results in hyperfine output",
                )

            if rtest_data is None:
                return ErrorResult(
                    repository=repo_name,
                    benchmark=benchmark_desc,
                    error="Could not find rtest results in hyperfine output",
                )

            # Extract values with type validation
            pytest_result = self._extract_hyperfine_result(pytest_data)
            rtest_result = self._extract_hyperfine_result(rtest_data)

            if isinstance(pytest_result, str):
                return ErrorResult(repository=repo_name, benchmark=benchmark_desc, error=pytest_result)
            if isinstance(rtest_result, str):
                return ErrorResult(repository=repo_name, benchmark=benchmark_desc, error=rtest_result)

            speedup = pytest_result.mean / rtest_result.mean if rtest_result.mean > 0 else None

            return BenchmarkResult(
                repository=repo_name,
                benchmark=benchmark_desc,
                pytest=pytest_result,
                rtest=rtest_result,
                speedup=speedup,
                pytest_memory_kb=pytest_memory_kb,
                rtest_memory_kb=rtest_memory_kb,
            )
        except (FileNotFoundError, json.JSONDecodeError, KeyError) as e:
            return ErrorResult(repository=repo_name, benchmark=benchmark_desc, error=f"Failed to parse results: {e}")
        finally:
            if json_path.exists():
                json_path.unlink()

    def _extract_hyperfine_result(self, data: dict) -> HyperfineResult | str:
        """Extract HyperfineResult from hyperfine data dict. Returns error string on failure."""
        mean = data.get("mean")
        stddev = data.get("stddev")
        times = data.get("times", [])

        if not isinstance(mean, (int, float)):
            return f"Invalid mean value: {mean}"
        if not isinstance(stddev, (int, float)):
            return f"Invalid stddev value: {stddev}"
        if not isinstance(times, list):
            return f"Invalid times value: {times}"

        return HyperfineResult(
            mean=float(mean),
            stddev=float(stddev),
            times=cast(list[float], times),
        )


class ResultFormatter:
    """Formats and displays benchmark results."""

    @staticmethod
    def print_summary(results: list[BenchmarkResult | ErrorResult]) -> None:
        """Print a formatted summary of results."""
        print("\n" + "=" * 60)
        print("BENCHMARK SUMMARY")
        print("=" * 60)

        for result in results:
            if isinstance(result, ErrorResult):
                print(f"\n{result.repository.upper()}: ERROR - {result.error}")
                if result.command:
                    print(f"  Command: {result.command}")
                if result.exit_code is not None:
                    print(f"  Exit code: {result.exit_code}")
                if result.stderr:
                    print(f"  stderr: {result.stderr}")
                continue

            print(f"\n{result.repository.upper()}")
            print("-" * len(result.repository))

            if result.speedup:
                print(f"  {result.benchmark} ({len(result.pytest.times)} runs):")
                print(f"    pytest: {result.pytest.mean:.3f}s ± {result.pytest.stddev:.3f}s")
                print(f"    rtest:  {result.rtest.mean:.3f}s ± {result.rtest.stddev:.3f}s")
                time_saved = result.pytest.mean - result.rtest.mean
                print(f"    speedup: {result.speedup:.2f}x ({time_saved:.3f}s saved)")
                # Show memory if available
                if result.pytest_memory_kb and result.rtest_memory_kb:
                    memory_saved = result.pytest_memory_kb - result.rtest_memory_kb
                    pytest_mem = result.pytest_memory_kb
                    rtest_mem = result.rtest_memory_kb
                    print(f"    memory: pytest {pytest_mem}KB, rtest {rtest_mem}KB ({memory_saved:+d}KB)")
            else:
                print(f"  {result.benchmark}: Unable to calculate speedup")

    @staticmethod
    def save_results(results: list[BenchmarkResult | ErrorResult], output_path: Path) -> None:
        """Save results to JSON file."""
        serializable_results = []
        for result in results:
            if isinstance(result, BenchmarkResult):
                result_data: dict = {
                    "repository": result.repository,
                    "benchmark": result.benchmark,
                    "pytest": {
                        "mean": result.pytest.mean,
                        "stddev": result.pytest.stddev,
                        "runs": len(result.pytest.times),
                    },
                    "rtest": {
                        "mean": result.rtest.mean,
                        "stddev": result.rtest.stddev,
                        "runs": len(result.rtest.times),
                    },
                    "speedup": result.speedup,
                }
                # Add memory data if available
                if result.pytest_memory_kb is not None:
                    result_data["pytest"]["memory_kb"] = result.pytest_memory_kb
                if result.rtest_memory_kb is not None:
                    result_data["rtest"]["memory_kb"] = result.rtest_memory_kb
                serializable_results.append(result_data)
            else:
                error_data: dict = {
                    "repository": result.repository,
                    "benchmark": result.benchmark,
                    "error": result.error,
                }
                if result.command:
                    error_data["command"] = result.command
                if result.exit_code is not None:
                    error_data["exit_code"] = result.exit_code
                if result.stdout:
                    error_data["stdout"] = result.stdout
                if result.stderr:
                    error_data["stderr"] = result.stderr
                serializable_results.append(error_data)

        with open(output_path, "w") as f:
            json.dump(serializable_results, f, indent=2)

        logger.info(f"Results saved to {output_path}")


class BenchmarkOrchestrator:
    """Orchestrates the entire benchmarking process."""

    def __init__(
        self,
        config_path: Path,
        rtest_source: str,
        show_output: bool = False,
        ignore_failures: bool = False,
    ):
        self.repositories, self.benchmark_configs, self.settings = ConfigLoader.load_config(config_path)
        self.output_dir = Path(tempfile.mkdtemp(prefix="rtest_benchmark_results_"))
        self.temp_dir = Path(tempfile.mkdtemp(prefix="rtest_benchmark_repos_"))
        self.show_output = show_output
        self.ignore_failures = ignore_failures
        self.rtest_source = rtest_source

        # Get project root (2 levels up from scripts/benchmark/)
        self.project_root = config_path.parent.parent.parent.absolute()

        self.repo_manager = RepositoryManager(self.temp_dir, self.project_root, rtest_source)
        self.hyperfine_runner = HyperfineRunner(self.settings.validation_timeout)

        logger.info(f"Repository clone directory: {self.temp_dir}")
        logger.info(f"Results output directory: {self.output_dir}")
        logger.info(f"rtest source: {rtest_source}")

    def run_benchmarks(
        self, repositories: list[str] | None = None, benchmark_types: list[str] | None = None
    ) -> list[BenchmarkResult | ErrorResult]:
        """Run benchmarks on specified repositories."""
        # Filter repositories
        repos = self.repositories
        if repositories:
            repos = [r for r in repos if r.name in repositories]

        # Filter benchmarks
        benchmarks = self.benchmark_configs
        if benchmark_types:
            benchmarks = {k: v for k, v in benchmarks.items() if k in benchmark_types}

        results: list[BenchmarkResult | ErrorResult] = []
        for repo in repos:
            logger.info(f"\n{'=' * 50}\nBenchmarking {repo.name}\n{'=' * 50}")

            # Clone and setup
            repo_path = self.repo_manager.clone_repository(repo)
            if not repo_path:
                results.append(
                    ErrorResult(repository=repo.name, benchmark="Clone failed", error="Failed to clone repository")
                )
                continue

            if not self.repo_manager.setup_repository(repo, repo_path):
                logger.warning(f"Failed to setup {repo.name}, skipping...")
                results.append(
                    ErrorResult(
                        repository=repo.name, benchmark="Setup failed", error="Failed to setup repository environment"
                    )
                )
                continue

            # Run benchmarks
            for _, benchmark_config in benchmarks.items():
                results.append(
                    self.hyperfine_runner.run_benchmark(
                        repo, repo_path, benchmark_config, self.show_output, self.ignore_failures
                    )
                )

        return results

    def cleanup(self) -> None:
        """Clean up temporary directories."""
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir)
            logger.info(f"Cleaned up repository clone directory: {self.temp_dir}")
        logger.info(f"Results preserved in: {self.output_dir}")


def parse_arguments() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="Benchmark rtest vs pytest across multiple repositories",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="Examples:\n"
        "  %(prog)s --source local --list-repos\n"
        "  %(prog)s --source local --repositories flask click\n"
        "  %(prog)s --source 0.0.37 --repositories fastapi",
    )

    parser.add_argument(
        "--source",
        required=True,
        help="rtest source: 'local' for project root, 'wheel:/path/to/wheel.whl' for pre-built wheel, "
        "or a version number for PyPI (e.g., '0.0.37')",
    )
    parser.add_argument("--repositories", nargs="+", help="Specific repositories to benchmark")
    parser.add_argument("--list-repos", action="store_true", help="List available repositories")
    parser.add_argument(
        "--log-level", choices=["DEBUG", "INFO", "WARNING", "ERROR"], default="INFO", help="Set logging level"
    )
    parser.add_argument(
        "--debug", action="store_true", help="Show command output during benchmarks (passes --show-output to hyperfine)"
    )
    parser.add_argument(
        "--ignore-failures",
        action="store_true",
        help="Continue benchmarking even if commands fail (passes -i to hyperfine)",
    )

    return parser.parse_args()


def main() -> None:
    """Main entry point."""
    args = parse_arguments()

    # Configure logging
    log_level = getattr(logging, args.log_level)
    logging.getLogger().setLevel(log_level)

    # Get config path
    config_path = Path(__file__).parent / "repositories.yml"

    # Handle list repos command early without creating orchestrator
    if args.list_repos:
        try:
            repositories, _, _ = ConfigLoader.load_config(config_path)
            print("Available repositories:")
            for repo in repositories:
                print(f"  {repo.name} - {repo.category} (Python {repo.python_version})")
        except Exception as e:
            logger.error(f"Failed to load configuration: {e}")
            sys.exit(1)
        return

    try:
        orchestrator = BenchmarkOrchestrator(config_path, args.source, args.debug, args.ignore_failures)

        # Run all benchmark types defined in config
        results = orchestrator.run_benchmarks(args.repositories)
        ResultFormatter.print_summary(results)
        filename = f"benchmark_results_{int(time.time())}.json"
        output_path = orchestrator.output_dir / filename
        ResultFormatter.save_results(results, output_path)

    except KeyboardInterrupt:
        logger.info("\nBenchmark interrupted by user")
        sys.exit(1)
    except Exception as e:
        logger.error(f"Benchmark failed: {e}")
        sys.exit(1)
    finally:
        if "orchestrator" in locals():
            orchestrator.cleanup()


if __name__ == "__main__":
    main()
