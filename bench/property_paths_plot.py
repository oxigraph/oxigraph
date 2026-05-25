#!/usr/bin/env python3
"""
Render a composite PNG comparing the standard SPARQL evaluator against the
DataFusion evaluator on the property paths micro-benchmark.

Run the benchmark first:

    cargo bench -p oxigraph --features "rocksdb,datafusion" --bench property_paths

Then:

    python bench/property_paths_plot.py [--criterion-dir target/criterion] \\
                                        [--output bench/out/property_paths.png]

The driver walks target/criterion, reads each estimates.json, and groups
samples by (query_name, evaluator, input_size). One subplot per query, each
with two lines (standard, datafusion) over input size on a log-log scale.
"""

import argparse
import json
import math
import re
import sys
from pathlib import Path

import matplotlib.pyplot as plt


# Criterion's BenchmarkId names look like "one_or_more_unbound standard"
# (a query name, then a single-space evaluator label).
NAME_RE = re.compile(r"^(?P<query>.+) (?P<evaluator>standard|datafusion)$")

GROUP_DIR = "property paths"  # the criterion_group! label in the bench file


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument(
        "--criterion-dir",
        type=Path,
        default=Path("target/criterion"),
        help="Path to criterion's output directory (default: target/criterion)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        default=Path("bench/out/property_paths.png"),
        help="Where to write the PNG (default: bench/out/property_paths.png)",
    )
    return parser.parse_args()


def collect(criterion_dir: Path) -> dict[str, dict[str, list[tuple[int, float]]]]:
    """
    Returns {query_name: {evaluator: [(size, median_ns), ...]}}.
    """
    root = criterion_dir / GROUP_DIR
    if not root.is_dir():
        sys.exit(f"No criterion output at {root}. Run cargo bench first.")

    out: dict[str, dict[str, list[tuple[int, float]]]] = {}

    for bench_dir in sorted(root.iterdir()):
        if not bench_dir.is_dir():
            continue
        m = NAME_RE.match(bench_dir.name)
        if not m:
            continue
        query = m.group("query")
        evaluator = m.group("evaluator")

        for size_dir in sorted(bench_dir.iterdir()):
            if not size_dir.is_dir():
                continue
            try:
                size = int(size_dir.name)
            except ValueError:
                continue
            estimates = size_dir / "new" / "estimates.json"
            if not estimates.is_file():
                continue
            with estimates.open() as f:
                data = json.load(f)
            median_ns = data.get("median", {}).get("point_estimate")
            if median_ns is None:
                continue
            out.setdefault(query, {}).setdefault(evaluator, []).append((size, float(median_ns)))

    for query, by_eval in out.items():
        for evaluator, samples in by_eval.items():
            samples.sort(key=lambda x: x[0])

    return out


def render(data: dict, output: Path) -> None:
    queries = sorted(data.keys())
    if not queries:
        sys.exit("No samples found. Did the benchmark run?")

    cols = 2
    rows = math.ceil(len(queries) / cols)
    fig, axes = plt.subplots(rows, cols, figsize=(11, 3.4 * rows), squeeze=False)

    colors = {"standard": "tab:blue", "datafusion": "tab:red"}

    for idx, query in enumerate(queries):
        ax = axes[idx // cols][idx % cols]
        by_eval = data[query]
        for evaluator in ("standard", "datafusion"):
            samples = by_eval.get(evaluator, [])
            if not samples:
                continue
            xs = [s for s, _ in samples]
            ys = [ns / 1e6 for _, ns in samples]  # ms
            ax.plot(xs, ys, marker="o", label=evaluator, color=colors[evaluator])
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_title(query)
        ax.set_xlabel("input triples (log)")
        ax.set_ylabel("median wall clock ms (log)")
        ax.grid(True, which="both", linestyle=":", linewidth=0.5)
        ax.legend(loc="best", fontsize=8)

    # Hide unused subplots
    total = rows * cols
    for k in range(len(queries), total):
        axes[k // cols][k % cols].axis("off")

    fig.suptitle("SPARQL property path evaluation: standard vs DataFusion", fontsize=12)
    fig.tight_layout(rect=(0, 0, 1, 0.97))

    output.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(output, dpi=130)
    print(f"Wrote {output}")


def main() -> None:
    args = parse_args()
    data = collect(args.criterion_dir)
    render(data, args.output)


if __name__ == "__main__":
    main()
