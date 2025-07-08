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
import platform
import shutil
import subprocess
import sys
import tempfile
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Dict, List, Optional, Tuple, TypedDict, Union, cast

import yaml


# Type definitions for YAML data
class RepositoryData(TypedDict):
    """Type for repository data from YAML."""

    name: str
    url: str
    category: str
    test_dir: str


class BenchmarkConfigData(TypedDict, total=False):
    """Type for benchmark config data from YAML."""

    description: str
    pytest_args: str
    rtest_args: str
    timeout: int


class ConfigData(TypedDict):
    """Type for the full configuration data from YAML."""

    repositories: list[RepositoryData]
    benchmark_configs: dict[str, BenchmarkConfigData]


# Constants
DEFAULT_TIMEOUT = 300
HYPERFINE_MIN_RUNS = 20
HYPERFINE_MAX_RUNS = 20
HYPERFINE_WARMUP = 3

# Configure logging
logging.basicConfig(level=logging.INFO, format="%(asctime)s - %(levelname)s - %(message)s", datefmt="%Y-%m-%d %H:%M:%S")
logger = logging.getLogger(__name__)


@dataclass
class RepositoryConfig:
    """Configuration for a repository to benchmark."""

    name: str
    url: str
    category: str
    test_dir: str


@dataclass
class BenchmarkConfig:
    """Configuration for a benchmark type."""

    description: str
    pytest_args: str
    rtest_args: str
    timeout: int = DEFAULT_TIMEOUT


@dataclass
class HyperfineResult:
    """Result from a hyperfine benchmark run."""

    mean: float
    stddev: float
    times: List[float]


@dataclass
class BenchmarkResult:
    """Result from benchmarking a repository."""

    repository: str
    benchmark: str
    pytest: HyperfineResult
    rtest: HyperfineResult
    speedup: Optional[float]


@dataclass
class ErrorResult:
    """Error result when benchmark fails."""

    repository: str
    benchmark: str
    error: str


class ConfigLoader:
    """Handles loading and validation of configuration files."""

    @staticmethod
    def load_config(config_path: Path) -> Tuple[List[RepositoryConfig], Dict[str, BenchmarkConfig]]:
        """Load and validate configuration from YAML file."""
        if not config_path.exists():
            raise FileNotFoundError(f"Configuration file not found: {config_path}")

        try:
            with open(config_path, "r") as f:
                # Cast the result to our expected type
                raw_data = cast(ConfigData, yaml.safe_load(f))
        except yaml.YAMLError as e:
            raise ValueError(f"Invalid YAML in configuration file: {e}")

        if not isinstance(raw_data, dict):
            raise ValueError("Configuration must be a dictionary")

        # Validate and convert repositories
        repositories: List[RepositoryConfig] = []
        raw_repos: List[RepositoryData] = raw_data.get("repositories", [])
        if not isinstance(raw_repos, list):
            raise ValueError("Repositories must be a list")

        for repo_data in raw_repos:
            if not isinstance(repo_data, dict):
                raise ValueError(f"Invalid repository configuration: {repo_data}")
            if not all(key in repo_data for key in ["name", "url", "category", "test_dir"]):
                raise ValueError(f"Invalid repository configuration: {repo_data}")
            # Extract only the fields we need
            repositories.append(
                RepositoryConfig(
                    name=repo_data["name"],
                    url=repo_data["url"],
                    category=repo_data["category"],
                    test_dir=repo_data["test_dir"],
                )
            )

        # Validate and convert benchmark configs
        benchmark_configs: Dict[str, BenchmarkConfig] = {}
        raw_benchmarks: Dict[str, BenchmarkConfigData] = raw_data.get("benchmark_configs", {})
        if not isinstance(raw_benchmarks, dict):
            raise ValueError("Benchmark configs must be a dictionary")

        for name, config_data in raw_benchmarks.items():
            if not isinstance(config_data, dict):
                raise ValueError(f"Invalid benchmark configuration: {config_data}")
            if not all(key in config_data for key in ["description", "pytest_args", "rtest_args"]):
                raise ValueError(f"Invalid benchmark configuration: {config_data}")
            # Create BenchmarkConfig with data from the dict
            benchmark_configs[name] = BenchmarkConfig(
                description=config_data["description"],
                pytest_args=config_data["pytest_args"],
                rtest_args=config_data["rtest_args"],
                timeout=config_data.get("timeout", DEFAULT_TIMEOUT),
            )

        return (repositories, benchmark_configs)


def run_command(cmd: List[str], cwd: str, timeout: int = DEFAULT_TIMEOUT) -> subprocess.CompletedProcess[str]:
    """Execute a command and return the result."""
    logger.debug(f"Running command: {' '.join(cmd)} in {cwd}")
    try:
        return subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, timeout=timeout, check=False)
    except subprocess.TimeoutExpired:
        # Return a fake CompletedProcess for timeout
        result: subprocess.CompletedProcess[str] = subprocess.CompletedProcess(cmd, -1)
        result.stdout = ""
        result.stderr = f"Command timed out after {timeout}s"
        return result


class RepositoryManager:
    """Manages repository cloning and setup."""

    def __init__(self, temp_dir: Path, project_root: Path):
        self.temp_dir = temp_dir
        self.project_root = project_root

    def clone_repository(self, repo_config: RepositoryConfig) -> Optional[Path]:
        """Clone a repository to temporary directory."""
        repo_path = self.temp_dir / repo_config.name

        if repo_path.exists():
            logger.info(f"Repository {repo_config.name} already exists, skipping clone")
            return repo_path

        logger.info(f"Cloning {repo_config.name} from {repo_config.url}...")
        # Note: Using --recurse-submodules with --depth 1 for efficiency
        result = run_command(
            ["git", "clone", "--depth", "1", "--recurse-submodules", repo_config.url, str(repo_path)],
            str(self.temp_dir),
            timeout=600,
        )

        if result.returncode != 0:
            logger.error(f"Failed to clone {repo_config.name}: {result.stderr}")
            return None

        logger.info(f"Cloned {repo_config.name}")
        return repo_path

    def setup_repository(self, repo_config: RepositoryConfig, repo_path: Path) -> bool:
        """Set up repository dependencies using venv and pip."""
        logger.info(f"Setting up {repo_config.name}...")

        # Check if requirements.txt exists, if not compile from pyproject.toml
        requirements_path = repo_path / "requirements.txt"
        if not requirements_path.exists():
            pyproject_path = repo_path / "pyproject.toml"
            if pyproject_path.exists():
                logger.info("No requirements.txt found, compiling from pyproject.toml...")
                result = run_command(
                    ["uv", "pip", "compile", "pyproject.toml", "-o", "requirements.txt"], str(repo_path)
                )
                if result.returncode != 0:
                    logger.error(f"Failed to compile requirements.txt: {result.stderr}")
                    return False
            else:
                logger.warning(f"No requirements.txt or pyproject.toml found for {repo_config.name}")
                # Create a minimal requirements.txt
                requirements_path.write_text("")

        # Create virtual environment
        venv_path = repo_path / ".venv"
        logger.info("Creating virtual environment...")
        result = run_command(["python3", "-m", "venv", str(venv_path)], str(repo_path))
        if result.returncode != 0:
            logger.error(f"Failed to create virtual environment: {result.stderr}")
            return False

        # Determine pip executable path based on OS
        if platform.system() == "Windows":
            pip_executable = venv_path / "Scripts" / "pip3"
        else:
            pip_executable = venv_path / "bin" / "pip3"

        # Install requirements
        logger.info("Installing requirements...")
        result = run_command([str(pip_executable), "install", "-r", "requirements.txt"], str(repo_path))
        if result.returncode != 0:
            raise Exception(f"Failed to install requirements: {result.stderr}")

        logger.info("Installing pytest, pytest-xdist, and rtest...")
        # Install rtest from the project root
        result = run_command(
            [str(pip_executable), "install", "pytest", "pytest-xdist", f"rtest @ file://{self.project_root}"],
            str(repo_path),
        )
        if result.returncode != 0:
            logger.error(f"Failed to install pytest, pytest-xdist, and rtest: {result.stderr}")
            return False

        return True


class HyperfineRunner:
    """Runs benchmarks using hyperfine."""

    def run_benchmark(
        self, repo_config: RepositoryConfig, repo_path: Path, benchmark_config: BenchmarkConfig
    ) -> Union[BenchmarkResult, ErrorResult]:
        """Run a benchmark using hyperfine."""
        test_dir_path = repo_path / repo_config.test_dir
        if not test_dir_path.exists():
            return ErrorResult(
                repository=repo_config.name,
                benchmark=benchmark_config.description,
                error=f"Test directory {repo_config.test_dir} does not exist",
            )

        logger.info(f"Running {benchmark_config.description} on {repo_config.name}")

        # Use the venv executables directly
        venv_path = repo_path / ".venv"
        if platform.system() == "Windows":
            pytest_executable = venv_path / "Scripts" / "pytest"
            rtest_executable = venv_path / "Scripts" / "rtest"
        else:
            pytest_executable = venv_path / "bin" / "pytest"
            rtest_executable = venv_path / "bin" / "rtest"

        # Build commands
        pytest_cmd = self._build_command(str(pytest_executable), benchmark_config.pytest_args, repo_config.test_dir)
        rtest_cmd = self._build_command(str(rtest_executable), benchmark_config.rtest_args, repo_config.test_dir)

        # Run hyperfine
        json_output = f"{repo_config.name}_benchmark.json"
        hyperfine_cmd = self._build_hyperfine_command(pytest_cmd, rtest_cmd, json_output)

        result = run_command(hyperfine_cmd, str(repo_path), timeout=benchmark_config.timeout)

        if result.returncode != 0:
            return ErrorResult(
                repository=repo_config.name,
                benchmark=benchmark_config.description,
                error=f"Hyperfine failed: {result.stderr}",
            )

        # Parse results
        return self._parse_results(repo_path, json_output, repo_config.name, benchmark_config.description)

    def _build_command(self, executable: str, args: str, test_dir: str) -> str:
        """Build command string for hyperfine."""
        cmd_parts = [executable] + args.split() + [test_dir]
        return " ".join(cmd_parts)

    def _build_hyperfine_command(self, pytest_cmd: str, rtest_cmd: str, json_output: str) -> List[str]:
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

        cmd.extend([pytest_cmd, rtest_cmd])
        return cmd

    def _parse_results(
        self, repo_path: Path, json_output: str, repo_name: str, benchmark_desc: str
    ) -> Union[BenchmarkResult, ErrorResult]:
        """Parse hyperfine JSON output."""
        json_path = repo_path / json_output

        try:
            with open(json_path) as f:
                data = json.load(f)

            results = data["results"]
            pytest_data = results[0]
            rtest_data = results[1]

            # Extract values with proper type handling
            pytest_mean = pytest_data.get("mean", 0.0)
            pytest_stddev = pytest_data.get("stddev", 0.0)
            pytest_times = pytest_data.get("times", [])

            rtest_mean = rtest_data.get("mean", 0.0)
            rtest_stddev = rtest_data.get("stddev", 0.0)
            rtest_times = rtest_data.get("times", [])

            if not isinstance(pytest_mean, (int, float)) or not isinstance(pytest_stddev, (int, float)):
                raise ValueError("Invalid numeric data in pytest results")
            if not isinstance(rtest_mean, (int, float)) or not isinstance(rtest_stddev, (int, float)):
                raise ValueError("Invalid numeric data in rtest results")
            if not isinstance(pytest_times, list) or not isinstance(rtest_times, list):
                raise ValueError("Invalid times data in results")

            pytest_result = HyperfineResult(
                mean=float(pytest_mean),
                stddev=float(pytest_stddev),
                times=cast(List[float], pytest_times),
            )

            rtest_result = HyperfineResult(
                mean=float(rtest_mean),
                stddev=float(rtest_stddev),
                times=cast(List[float], rtest_times),
            )

            speedup = pytest_result.mean / rtest_result.mean if rtest_result.mean > 0 else None

            return BenchmarkResult(
                repository=repo_name,
                benchmark=benchmark_desc,
                pytest=pytest_result,
                rtest=rtest_result,
                speedup=speedup,
            )
        except (FileNotFoundError, json.JSONDecodeError, KeyError) as e:
            return ErrorResult(repository=repo_name, benchmark=benchmark_desc, error=f"Failed to parse results: {e}")
        finally:
            if json_path.exists():
                json_path.unlink()


class ResultFormatter:
    """Formats and displays benchmark results."""

    @staticmethod
    def print_summary(results: List[Union[BenchmarkResult, ErrorResult]]) -> None:
        """Print a formatted summary of results."""
        print("\n" + "=" * 60)
        print("BENCHMARK SUMMARY")
        print("=" * 60)

        for result in results:
            if isinstance(result, ErrorResult):
                print(f"\n{result.repository.upper()}: ERROR - {result.error}")
                continue

            print(f"\n{result.repository.upper()}")
            print("-" * len(result.repository))

            if result.speedup:
                print(f"  {result.benchmark} ({len(result.pytest.times)} runs):")
                print(f"    pytest: {result.pytest.mean:.3f}s ± {result.pytest.stddev:.3f}s")
                print(f"    rtest:  {result.rtest.mean:.3f}s ± {result.rtest.stddev:.3f}s")
                time_saved = result.pytest.mean - result.rtest.mean
                print(f"    speedup: {result.speedup:.2f}x ({time_saved:.3f}s saved)")
            else:
                print(f"  {result.benchmark}: Unable to calculate speedup")

    @staticmethod
    def save_results(results: List[Union[BenchmarkResult, ErrorResult]], output_path: Path) -> None:
        """Save results to JSON file."""
        serializable_results = []
        for result in results:
            if isinstance(result, BenchmarkResult):
                serializable_results.append(
                    {
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
                )
            else:
                serializable_results.append({"repository": result.repository, "error": result.error})

        with open(output_path, "w") as f:
            json.dump(serializable_results, f, indent=2)

        logger.info(f"Results saved to {output_path}")


class BenchmarkOrchestrator:
    """Orchestrates the entire benchmarking process."""

    def __init__(self, config_path: Path):
        self.repositories, self.benchmark_configs = ConfigLoader.load_config(config_path)
        self.output_dir = Path(tempfile.mkdtemp(prefix="rtest_benchmark_results_"))
        self.temp_dir = Path(tempfile.mkdtemp(prefix="rtest_benchmark_repos_"))

        # Get project root (2 levels up from scripts/benchmark/)
        self.project_root = config_path.parent.parent.parent.absolute()

        self.repo_manager = RepositoryManager(self.temp_dir, self.project_root)
        self.hyperfine_runner = HyperfineRunner()

        logger.info(f"Repository clone directory: {self.temp_dir}")
        logger.info(f"Results output directory: {self.output_dir}")

    def run_benchmarks(
        self, repositories: Optional[List[str]] = None, benchmark_types: Optional[List[str]] = None
    ) -> List[Union[BenchmarkResult, ErrorResult]]:
        """Run benchmarks on specified repositories."""
        # Filter repositories
        repos = self.repositories
        if repositories:
            repos = [r for r in repos if r.name in repositories]

        # Filter benchmarks
        benchmarks = self.benchmark_configs
        if benchmark_types:
            benchmarks = {k: v for k, v in benchmarks.items() if k in benchmark_types}

        results: List[Union[BenchmarkResult, ErrorResult]] = []
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
                results.append(self.hyperfine_runner.run_benchmark(repo, repo_path, benchmark_config))

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
        "  %(prog)s --list-repos\n"
        "  %(prog)s --repositories fastapi flask\n"
        "  %(prog)s --collect-only\n"
        "  %(prog)s --repositories click --collect-only",
    )

    parser.add_argument("--repositories", nargs="+", help="Specific repositories to benchmark")
    parser.add_argument(
        "--collect-only", action="store_true", help="Run only test collection benchmarks (skip execution)"
    )
    parser.add_argument("--list-repos", action="store_true", help="List available repositories")
    parser.add_argument(
        "--log-level", choices=["DEBUG", "INFO", "WARNING", "ERROR"], default="INFO", help="Set logging level"
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
            repositories, _ = ConfigLoader.load_config(config_path)
            print("Available repositories:")
            for repo in repositories:
                print(f"  {repo.name} - {repo.category}")
        except Exception as e:
            logger.error(f"Failed to load configuration: {e}")
            sys.exit(1)
        return

    try:
        orchestrator = BenchmarkOrchestrator(config_path)

        # Run benchmarks
        benchmark_types = ["collect_only"] if args.collect_only else ["collect_only", "execution"]
        results = orchestrator.run_benchmarks(args.repositories, benchmark_types)
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
