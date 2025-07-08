#!/usr/bin/env python3
"""Aggregate benchmark results from multiple repositories into a single JSON file."""

import argparse
import json
from glob import glob
from pathlib import Path
from typing import Dict, List, Union, cast


def find_result_files(results_dir: Path) -> List[Path]:
    """Find all benchmark result JSON files in the given directory."""
    pattern = str(results_dir / "*" / "*" / "*.json")
    return [Path(f) for f in glob(pattern)]


def aggregate_results(results_dir: Path, output_file: Path) -> List[Dict[str, Union[str, float]]]:
    """Aggregate all benchmark results into a single list."""
    all_results: List[Dict[str, Union[str, float]]] = []

    # Find all result directories
    result_dirs = sorted([d for d in results_dir.iterdir() if d.is_dir()])

    for result_dir in result_dirs:
        repo_name = result_dir.name.replace("benchmark-results-", "")

        print(f"### {repo_name}")
        print()

        # Find JSON files in the directory
        json_files = list(result_dir.glob("*/*.json"))

        if json_files:
            # Use the first JSON file found
            with open(json_files[0]) as f:
                results = cast(List[Dict[str, Union[str, float]]], json.load(f))
                all_results.extend(results)

            # Pretty print for summary
            print("```json")
            print(json.dumps(results, indent=2))
            print("```")
        else:
            print("No results found")

        print()

    # Write aggregated results
    with open(output_file, "w") as f:
        json.dump(all_results, f)

    return all_results


def main() -> int:
    """Main entry point."""
    parser = argparse.ArgumentParser(description="Aggregate benchmark results from multiple repositories")
    parser.add_argument("results_dir", type=Path, help="Directory containing benchmark results (e.g., all-results/)")
    parser.add_argument("output_file", type=Path, help="Output file for aggregated results")

    args = parser.parse_args()

    results_dir = cast(Path, args.results_dir)
    output_file = cast(Path, args.output_file)

    if not results_dir.exists():
        print(f"Error: Results directory not found: {results_dir}")
        return 1

    aggregate_results(results_dir, output_file)

    print(f"Aggregated results written to: {output_file}")

    return 0


if __name__ == "__main__":
    exit(main())
