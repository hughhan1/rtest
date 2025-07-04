#!/usr/bin/env python3
"""
Sophisticated benchmarking script for rtest vs pytest across multiple repositories.
"""

import argparse
import json
import shutil
import subprocess
import tempfile
import time
from pathlib import Path
from typing import Dict, List, Optional, Tuple

import yaml

# Constants
DEFAULT_TIMEOUT = 300
VENV_NAME = ".benchmark_venv"
PYPI_INDEX = "https://pypi.org/simple"


class RepositoryBenchmark:
    def __init__(self, config_file: str = "repositories.yml", output_dir: str = None):
        self.config_file = config_file
        
        # Use system temp directory for output if not specified
        if output_dir is None:
            self.output_dir = Path(tempfile.mkdtemp(prefix="rtest_benchmark_results_"))
        else:
            # If output_dir is relative, make it absolute relative to temp dir
            if not os.path.isabs(output_dir):
                self.output_dir = Path(tempfile.mkdtemp(prefix="rtest_benchmark_results_")) / output_dir
            else:
                self.output_dir = Path(output_dir)
        
        self.output_dir.mkdir(parents=True, exist_ok=True)
        
        # Always use temp directory for cloning repositories
        self.temp_dir = Path(tempfile.mkdtemp(prefix="rtest_benchmark_repos_"))
        
        self.config = self._load_config()
        
        print(f"Repository clone directory: {self.temp_dir}")
        print(f"Results output directory: {self.output_dir}")
        
    def _load_config(self) -> Dict:
        """Load repository configuration from YAML file."""
        with open(self.config_file, 'r') as f:
            return yaml.safe_load(f)
    
    def _run_command(self, cmd: List[str], cwd: str, timeout: int = DEFAULT_TIMEOUT) -> Tuple[float, str, str, int]:
        """Run a command and return timing, stdout, stderr, and return code."""
        start_time = time.time()
        try:
            result = subprocess.run(
                cmd,
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=timeout,
                check=False
            )
            return time.time() - start_time, result.stdout, result.stderr, result.returncode
        except subprocess.TimeoutExpired:
            return timeout, "", "Command timed out", -1
    
    def _clone_repository(self, repo_config: Dict) -> Optional[Path]:
        """Clone a repository to temporary directory."""
        repo_name = repo_config["name"]
        repo_url = repo_config["url"]
        repo_path = self.temp_dir / repo_name
        
        if repo_path.exists():
            print(f"Repository {repo_name} already exists, skipping clone")
            return repo_path
            
        print(f"Cloning {repo_name} from {repo_url}...")
        duration, stdout, stderr, returncode = self._run_command(
            ["git", "clone", "--depth", "1", repo_url, str(repo_path)],
            str(self.temp_dir),
            timeout=600
        )
        
        if returncode != 0:
            print(f"Failed to clone {repo_name}: {stderr}")
            return None
            
        print(f"Cloned {repo_name} in {duration:.2f}s")
        return repo_path
    
    def _setup_repository(self, repo_config: Dict, repo_path: Path) -> bool:
        """Set up repository dependencies."""
        repo_name = repo_config["name"]
        print(f"Setting up {repo_name}...")
        
        # Create virtual environment 
        venv_path = repo_path / VENV_NAME
        duration, stdout, stderr, returncode = self._run_command(
            ["python3", "-m", "venv", str(venv_path)],
            str(repo_path)
        )
        
        if returncode != 0:
            print(f"Failed to create venv: {stderr}")
            return False
        
        # Install pytest and dependencies with public PyPI
        pip_cmd = str(venv_path / "bin" / "pip")
        
        # Install pytest first
        self._run_command([pip_cmd, "install", "--index-url", PYPI_INDEX, "pytest"], str(repo_path))
        
        # Run repository setup commands
        for cmd in repo_config.get("setup_commands", []):
            cmd_parts = cmd.split()
            if cmd_parts[0] == "pip":
                cmd_parts[0] = pip_cmd
                cmd_parts.extend(["--index-url", PYPI_INDEX])
            
            self._run_command(cmd_parts, str(repo_path), timeout=DEFAULT_TIMEOUT)
        
        return True
    
    def _run_benchmark(self, repo_config: Dict, repo_path: Path, benchmark_config: Dict) -> Dict:
        """Run a specific benchmark configuration on a repository."""
        repo_name = repo_config["name"]
        benchmark_name = benchmark_config["description"]
        test_dir = repo_path / repo_config["test_dir"]
        
        if not test_dir.exists():
            return {
                "error": f"Test directory {test_dir} does not exist"
            }
        
        print(f"Running {benchmark_name} on {repo_name}...")
        
        # Use the venv created in setup
        python_cmd = str(repo_path / ".benchmark_venv" / "bin" / "python")
        
        results = {
            "repository": repo_name,
            "benchmark": benchmark_name,
            "test_directory": str(test_dir),
            "timestamp": time.time()
        }
        
        # Run pytest
        pytest_cmd = [python_cmd, "-m", "pytest"] + benchmark_config["pytest_args"].split() + [str(test_dir)]
        pytest_duration, pytest_stdout, pytest_stderr, pytest_returncode = self._run_command(
            pytest_cmd,
            str(repo_path),
            timeout=benchmark_config.get("timeout", 300)
        )
        
        results["pytest"] = {
            "duration": pytest_duration,
            "return_code": pytest_returncode,
            "stdout_lines": len(pytest_stdout.splitlines()),
            "stderr_lines": len(pytest_stderr.splitlines()),
            "stderr_preview": pytest_stderr[:200] if pytest_stderr else "",
            "success": pytest_returncode == 0
        }
        
        # Run rtest using uv run from the main project
        project_dir = Path(__file__).parent
        rtest_cmd = ["uv", "run", "rtest"] + benchmark_config["rtest_args"].split() + [str(test_dir)]
        rtest_duration, rtest_stdout, rtest_stderr, rtest_returncode = self._run_command(
            rtest_cmd,
            str(project_dir),
            timeout=benchmark_config.get("timeout", 300)
        )
        
        results["rtest"] = {
            "duration": rtest_duration,
            "return_code": rtest_returncode,
            "stdout_lines": len(rtest_stdout.splitlines()),
            "stderr_lines": len(rtest_stderr.splitlines()),
            "stderr_preview": rtest_stderr[:200] if rtest_stderr else "",
            "success": rtest_returncode == 0
        }
        
        # Calculate speedup
        if pytest_duration > 0 and rtest_duration > 0:
            results["speedup"] = pytest_duration / rtest_duration
            results["time_saved"] = pytest_duration - rtest_duration
        
        return results
    
    def run_benchmarks(self, repositories: Optional[List[str]] = None, 
                      benchmarks: Optional[List[str]] = None,
                      skip_setup: bool = False) -> List[Dict]:
        """Run benchmarks on specified repositories."""
        all_results = []
        
        repos_to_test = self.config["repositories"]
        if repositories:
            repos_to_test = [r for r in repos_to_test if r["name"] in repositories]
        
        benchmarks_to_run = self.config["benchmark_configs"]
        if benchmarks:
            benchmarks_to_run = {k: v for k, v in benchmarks_to_run.items() if k in benchmarks}
        
        for repo_config in repos_to_test:
            repo_name = repo_config["name"]
            print(f"\n{'='*50}")
            print(f"Benchmarking {repo_name}")
            print(f"{'='*50}")
            
            # Clone repository
            repo_path = self._clone_repository(repo_config)
            if not repo_path:
                continue
            
            # Set up repository
            if not skip_setup:
                if not self._setup_repository(repo_config, repo_path):
                    print(f"Failed to setup {repo_name}, skipping...")
                    continue
            
            # Run benchmarks
            for benchmark_name, benchmark_config in benchmarks_to_run.items():
                result = self._run_benchmark(repo_config, repo_path, benchmark_config)
                result["benchmark_name"] = benchmark_name
                all_results.append(result)
        
        return all_results
    
    def save_results(self, results: List[Dict], filename: str = None):
        """Save benchmark results to JSON file."""
        if filename is None:
            filename = f"benchmark_results_{int(time.time())}.json"
        
        output_path = self.output_dir / filename
        with open(output_path, 'w') as f:
            json.dump(results, f, indent=2)
        
        print(f"\nResults saved to {output_path}")
    
    def print_summary(self, results: List[Dict]):
        """Print a summary of benchmark results."""
        print(f"\n{'='*60}")
        print("BENCHMARK SUMMARY")
        print(f"{'='*60}")
        
        by_repo = {}
        for result in results:
            repo = result["repository"]
            if repo not in by_repo:
                by_repo[repo] = []
            by_repo[repo].append(result)
        
        for repo, repo_results in by_repo.items():
            print(f"\n{repo.upper()}")
            print("-" * len(repo))
            
            for result in repo_results:
                if "error" in result:
                    print(f"  {result['benchmark']}: ERROR - {result['error']}")
                    continue
                
                pytest_time = result["pytest"]["duration"]
                rtest_time = result["rtest"]["duration"]
                
                if "speedup" in result:
                    speedup = result["speedup"]
                    time_saved = result["time_saved"]
                    print(f"  {result['benchmark']}:")
                    print(f"    pytest: {pytest_time:.2f}s")
                    print(f"    rtest:  {rtest_time:.2f}s")
                    print(f"    speedup: {speedup:.2f}x ({time_saved:.2f}s saved)")
                else:
                    print(f"  {result['benchmark']}: Unable to calculate speedup")
    
    def cleanup(self):
        """Clean up temporary files."""
        if self.temp_dir.exists():
            shutil.rmtree(self.temp_dir)
            print(f"Cleaned up repository clone directory: {self.temp_dir}")
        
        print(f"Results preserved in: {self.output_dir}")


def main():
    parser = argparse.ArgumentParser(description="Benchmark rtest vs pytest across multiple repositories")
    parser.add_argument("--repositories", nargs="+", help="Specific repositories to benchmark")
    parser.add_argument("--benchmarks", nargs="+", help="Specific benchmarks to run")
    parser.add_argument("--config", default="repositories.yml", help="Configuration file")
    parser.add_argument("--output-dir", default=None, help="Output directory (default: system temp directory)")
    parser.add_argument("--skip-setup", action="store_true", help="Skip repository setup")
    parser.add_argument("--save-results", help="Save results to specific filename")
    parser.add_argument("--list-repos", action="store_true", help="List available repositories")
    parser.add_argument("--list-benchmarks", action="store_true", help="List available benchmarks")
    
    args = parser.parse_args()
    
    benchmark = RepositoryBenchmark(args.config, args.output_dir)
    
    if args.list_repos:
        print("Available repositories:")
        for repo in benchmark.config["repositories"]:
            print(f"  {repo['name']} - {repo['category']}")
        return
    
    if args.list_benchmarks:
        print("Available benchmarks:")
        for name, config in benchmark.config["benchmark_configs"].items():
            print(f"  {name} - {config['description']}")
        return
    
    benchmark = RepositoryBenchmark(args.config, args.output_dir)
    
    try:
        results = benchmark.run_benchmarks(
            repositories=args.repositories,
            benchmarks=args.benchmarks,
            skip_setup=args.skip_setup
        )
        
        benchmark.print_summary(results)
        benchmark.save_results(results, args.save_results)
        
    except KeyboardInterrupt:
        print("\nBenchmark interrupted by user")
    finally:
        benchmark.cleanup()


if __name__ == "__main__":
    main()