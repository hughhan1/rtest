#!/usr/bin/env python3
"""Compare benchmark results between baseline and current run."""

import argparse
import json
import sys
from dataclasses import dataclass
from enum import Enum
from pathlib import Path
from typing import Dict, List, Optional, TypedDict


class BenchmarkStatus(Enum):
    SIGNIFICANT_REGRESSION = "significant_regression"
    REGRESSION = "regression"
    NO_CHANGE = "no_change"
    IMPROVEMENT = "improvement"
    SIGNIFICANT_IMPROVEMENT = "significant_improvement"


class RtestMetrics(TypedDict):
    mean: float
    stddev: float
    times: List[float]


class BenchmarkResult(TypedDict, total=False):
    repository: str
    benchmark: str
    rtest: RtestMetrics
    error: str  # Optional field


@dataclass(frozen=True)
class ComparisonConfig:
    regression_threshold: float
    significant_threshold: float


@dataclass(frozen=True)
class BenchmarkComparison:
    repository: str
    benchmark: str
    baseline_time: Optional[float]
    current_time: Optional[float]
    change_percentage: Optional[float]
    status: BenchmarkStatus

    def is_regression(self) -> bool:
        """Check if this comparison represents a regression."""
        return self.status in (BenchmarkStatus.REGRESSION, BenchmarkStatus.SIGNIFICANT_REGRESSION)


STATUS_SYMBOLS = {
    BenchmarkStatus.SIGNIFICANT_REGRESSION: "[!!]",
    BenchmarkStatus.REGRESSION: "[!]",
    BenchmarkStatus.NO_CHANGE: "[-]",
    BenchmarkStatus.IMPROVEMENT: "[+]",
    BenchmarkStatus.SIGNIFICANT_IMPROVEMENT: "[++]",
}


def load_results(file_path: Path) -> List[BenchmarkResult]:
    """Load benchmark results from JSON file."""
    with open(file_path) as f:
        data: List[BenchmarkResult] = json.load(f)
        return data


def format_time(seconds: float) -> str:
    """Format time in seconds to a readable string."""
    if seconds < 0.001:
        return f"{seconds * 1000000:.0f}μs"
    elif seconds < 1:
        return f"{seconds * 1000:.1f}ms"
    else:
        return f"{seconds:.2f}s"


def calculate_percentage_change(base: float, current: float) -> float:
    """Calculate percentage change. Returns 0 if base is 0."""
    if base == 0:
        return 0.0
    return ((current - base) / base) * 100


def classify_change(percentage: float, config: ComparisonConfig) -> BenchmarkStatus:
    """Classify the change based on percentage and thresholds."""
    if percentage > config.significant_threshold:
        return BenchmarkStatus.SIGNIFICANT_REGRESSION
    elif percentage > config.regression_threshold:
        return BenchmarkStatus.REGRESSION
    elif percentage < -config.significant_threshold:
        return BenchmarkStatus.SIGNIFICANT_IMPROVEMENT
    elif percentage < -config.regression_threshold:
        return BenchmarkStatus.IMPROVEMENT
    else:
        return BenchmarkStatus.NO_CHANGE


def create_result_index(results: List[BenchmarkResult]) -> Dict[str, BenchmarkResult]:
    """Create a lookup dictionary from benchmark results."""
    index = {}
    for result in results:
        if "error" not in result:
            key = f"{result['repository']}:{result['benchmark']}"
            index[key] = result
    return index


def analyze_results(
    baseline: List[BenchmarkResult], current: List[BenchmarkResult], config: ComparisonConfig
) -> List[BenchmarkComparison]:
    """Analyze benchmark results and return comparison data."""
    comparisons = []

    baseline_index = create_result_index(baseline)
    current_index = create_result_index(current)

    for key in sorted(set(baseline_index.keys()) & set(current_index.keys())):
        current_result = current_index[key]
        baseline_result = baseline_index[key]

        repo = current_result["repository"]
        benchmark = current_result["benchmark"]
        current_time = current_result["rtest"]["mean"]
        baseline_time = baseline_result["rtest"]["mean"]

        change_pct = calculate_percentage_change(baseline_time, current_time)
        status = classify_change(change_pct, config)

        comparisons.append(
            BenchmarkComparison(
                repository=repo,
                benchmark=benchmark,
                baseline_time=baseline_time,
                current_time=current_time,
                change_percentage=change_pct,
                status=status,
            )
        )

    return comparisons


def format_comparison_row(comparison: BenchmarkComparison) -> str:
    """Format a single comparison as a table row."""
    repo = comparison.repository
    benchmark = comparison.benchmark
    symbol = STATUS_SYMBOLS[comparison.status]
    # These are guaranteed to be non-None for compared benchmarks
    baseline_str = format_time(comparison.baseline_time)  # type: ignore[arg-type]
    current_str = format_time(comparison.current_time)  # type: ignore[arg-type]
    change_str = f"{comparison.change_percentage:+.1f}%"

    return f"| {repo} | {benchmark} | {baseline_str} | {current_str} | {change_str} | {symbol} |"


def calculate_summary_stats(comparisons: List[BenchmarkComparison]) -> Dict[str, int]:
    """Calculate summary statistics from comparisons."""
    stats = {"total": len(comparisons), "regressions": 0, "improvements": 0, "no_change": 0}

    for comparison in comparisons:
        if comparison.status in (BenchmarkStatus.REGRESSION, BenchmarkStatus.SIGNIFICANT_REGRESSION):
            stats["regressions"] += 1
        elif comparison.status in (BenchmarkStatus.IMPROVEMENT, BenchmarkStatus.SIGNIFICANT_IMPROVEMENT):
            stats["improvements"] += 1
        else:
            stats["no_change"] += 1

    return stats


def format_report(comparisons: List[BenchmarkComparison], config: ComparisonConfig) -> str:
    """Format comparison results as a markdown report."""
    lines = []

    lines.append("")
    lines.append("## Benchmark Comparison Report")
    lines.append("")
    lines.append("Comparing current results against baseline from `main` branch.")
    lines.append("")

    lines.append("| Repository | Benchmark | Baseline | Current | Change | Status |")
    lines.append("|------------|-----------|----------|---------|--------|--------|")

    for comparison in comparisons:
        lines.append(format_comparison_row(comparison))

    stats = calculate_summary_stats(comparisons)
    lines.append("")
    lines.append("### Summary")
    lines.append("")
    lines.append(f"- Total benchmarks: {stats['total']}")
    lines.append(f"- Regressions (>{config.regression_threshold}% slower): {stats['regressions']}")
    lines.append(f"- Improvements (>{config.regression_threshold}% faster): {stats['improvements']}")
    lines.append(f"- No significant change: {stats['no_change']}")

    lines.append("")
    if stats["regressions"] > 0:
        lines.append("**Performance regressions detected!**")
        lines.append("Please review the results above to ensure these are expected.")
    elif stats["improvements"] > 0:
        lines.append("**Performance improvements detected!**")
    else:
        lines.append("**No significant performance changes detected.**")

    lines.append("")
    lines.append("### Legend")
    sig_imp = STATUS_SYMBOLS[BenchmarkStatus.SIGNIFICANT_IMPROVEMENT]
    lines.append(f"- {sig_imp} Significant improvement (>{config.significant_threshold}% faster)")
    imp = STATUS_SYMBOLS[BenchmarkStatus.IMPROVEMENT]
    lines.append(f"- {imp} Minor improvement ({config.regression_threshold}-{config.significant_threshold}% faster)")
    lines.append(
        f"- {STATUS_SYMBOLS[BenchmarkStatus.NO_CHANGE]} No significant change (±{config.regression_threshold}%)"
    )
    reg = STATUS_SYMBOLS[BenchmarkStatus.REGRESSION]
    lines.append(f"- {reg} Minor regression ({config.regression_threshold}-{config.significant_threshold}% slower)")
    sig_reg = STATUS_SYMBOLS[BenchmarkStatus.SIGNIFICANT_REGRESSION]
    lines.append(f"- {sig_reg} Significant regression (>{config.significant_threshold}% slower)")

    return "\n".join(lines)


def parse_args() -> argparse.Namespace:
    """Parse command line arguments."""
    parser = argparse.ArgumentParser(
        description="Compare benchmark results between baseline and current run.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    parser.add_argument("baseline", type=Path, help="Path to baseline benchmark results JSON")

    parser.add_argument("current", type=Path, help="Path to current benchmark results JSON")

    parser.add_argument(
        "--regression-threshold",
        type=float,
        default=5.0,
        help="Percentage threshold for marking a regression (default: 5.0)",
    )

    parser.add_argument(
        "--significant-threshold",
        type=float,
        default=10.0,
        help="Percentage threshold for marking a significant change (default: 10.0)",
    )

    return parser.parse_args()


def main() -> None:
    """Main entry point."""
    args = parse_args()

    baseline_path: Path = args.baseline
    current_path: Path = args.current

    if not baseline_path.exists():
        print(f"Error: Baseline file not found: {baseline_path}", file=sys.stderr)
        sys.exit(1)

    if not current_path.exists():
        print(f"Error: Current results file not found: {current_path}", file=sys.stderr)
        sys.exit(1)

    # Extract typed values from args
    regression_threshold: float = args.regression_threshold
    significant_threshold: float = args.significant_threshold

    config = ComparisonConfig(regression_threshold=regression_threshold, significant_threshold=significant_threshold)

    try:
        baseline_results = load_results(baseline_path)
        current_results = load_results(current_path)

        comparisons = analyze_results(baseline_results, current_results, config)
        report = format_report(comparisons, config)

        print(report)

        if any(c.is_regression() for c in comparisons):
            sys.exit(1)

    except Exception as e:
        print(f"Error: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()
