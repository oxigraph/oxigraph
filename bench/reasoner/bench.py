#!/usr/bin/env python3
"""Benchmark OWL 2 RL reasoners against LUBM-style synthetic graphs.

Reasoners under test:

* ``pyoxigraph`` (native): the ``oxreason`` crate exposed through the new
  :py:class:`pyoxigraph.Reasoner` binding.
* ``owlrl`` (rdflib + owlrl): the pure Python reference implementation from
  https://github.com/RDFLib/OWL-RL.
* ``reasonable``: the Rust-backed reasoner from
  https://github.com/gtfierro/reasonable, used via its Python binding.

For each target triple count the script generates a Turtle fixture (or
reuses a cached one), loads it into each reasoner's native data structure,
and times just the reasoning step. Parse time is measured separately for
visibility but is not part of the headline number.

Results are written to CSV and to a Matplotlib PNG plot.

Usage::

    python bench.py --sizes 100 300 1000 3000 10000 30000 100000 \\
                    --repeats 3 \\
                    --output-dir bench/reasoner/out
"""

from __future__ import annotations

import argparse
import csv
import json
import statistics
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

HERE = Path(__file__).resolve().parent
GENERATE = HERE / "generate_lubm.py"


@dataclass
class RunResult:
    reasoner: str
    target_triples: int
    actual_triples: int
    parse_ms: float
    reason_ms: float
    triples_out: int
    repeat_index: int


def ensure_fixture(target_triples: int, data_dir: Path) -> Path:
    """Generate (or reuse) a Turtle fixture sized to `target_triples`."""
    path = data_dir / f"lubm_{target_triples}.ttl"
    if not path.exists():
        subprocess.run(
            [
                sys.executable,
                str(GENERATE),
                "--target-triples",
                str(target_triples),
                "--output",
                str(path),
            ],
            check=True,
        )
    return path


# -----------------------------------------------------------------------------
# Reasoner adapters
# -----------------------------------------------------------------------------


def run_pyoxigraph(ttl_path: Path) -> tuple[float, float, int]:
    """Load the Turtle file into a pyoxigraph Dataset and run oxreason.

    ``parse`` yields Quads with ``DefaultGraph`` as the graph_name for
    Turtle input, so we can insert them straight into the dataset.
    """
    from pyoxigraph import Dataset, RdfFormat, Reasoner, parse

    ds = Dataset()

    parse_start = time.perf_counter()
    with ttl_path.open("rb") as fh:
        for quad in parse(fh, format=RdfFormat.TURTLE):
            ds.add(quad)
    parse_ms = (time.perf_counter() - parse_start) * 1000.0

    reasoner = Reasoner(profile="owl2-rl")
    reason_start = time.perf_counter()
    reasoner.expand(ds)
    reason_ms = (time.perf_counter() - reason_start) * 1000.0

    return parse_ms, reason_ms, len(ds)


def run_owlrl(ttl_path: Path) -> tuple[float, float, int]:
    """Load the Turtle file into rdflib and run owlrl's OWL-RL closure."""
    import owlrl
    from rdflib import Graph

    g = Graph()
    parse_start = time.perf_counter()
    g.parse(str(ttl_path), format="turtle")
    parse_ms = (time.perf_counter() - parse_start) * 1000.0

    reason_start = time.perf_counter()
    owlrl.DeductiveClosure(owlrl.OWLRL_Semantics).expand(g)
    reason_ms = (time.perf_counter() - reason_start) * 1000.0

    return parse_ms, reason_ms, len(g)


def run_reasonable(ttl_path: Path) -> tuple[float, float, int]:
    """Load the Turtle file into reasonable.PyReasoner and run reasoning."""
    import reasonable

    r = reasonable.PyReasoner()
    parse_start = time.perf_counter()
    r.load_file(str(ttl_path))
    parse_ms = (time.perf_counter() - parse_start) * 1000.0

    reason_start = time.perf_counter()
    triples = r.reason()
    reason_ms = (time.perf_counter() - reason_start) * 1000.0

    return parse_ms, reason_ms, len(triples)


REASONERS: dict[str, Callable[[Path], tuple[float, float, int]]] = {
    "pyoxigraph": run_pyoxigraph,
    "owlrl": run_owlrl,
    "reasonable": run_reasonable,
}


# -----------------------------------------------------------------------------
# Orchestration
# -----------------------------------------------------------------------------


def count_triples(ttl_path: Path) -> int:
    """Return the actual triple count of a Turtle file, via rdflib."""
    from rdflib import Graph

    g = Graph()
    g.parse(str(ttl_path), format="turtle")
    return len(g)


def run_cell(
    reasoner_name: str,
    fn: Callable[[Path], tuple[float, float, int]],
    ttl_path: Path,
    target: int,
    actual: int,
    repeats: int,
) -> list[RunResult]:
    results: list[RunResult] = []
    for i in range(repeats):
        try:
            parse_ms, reason_ms, triples_out = fn(ttl_path)
        except Exception as exc:
            print(
                f"  [{reasoner_name}] repeat {i}: FAILED: {exc}",
                file=sys.stderr,
            )
            continue
        results.append(
            RunResult(
                reasoner=reasoner_name,
                target_triples=target,
                actual_triples=actual,
                parse_ms=parse_ms,
                reason_ms=reason_ms,
                triples_out=triples_out,
                repeat_index=i,
            )
        )
        print(
            f"  [{reasoner_name}] repeat {i}: "
            f"parse={parse_ms:.1f}ms reason={reason_ms:.1f}ms "
            f"out={triples_out}"
        )
    return results


def write_csv(results: list[RunResult], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.writer(fh)
        writer.writerow(
            [
                "reasoner",
                "target_triples",
                "actual_triples",
                "repeat",
                "parse_ms",
                "reason_ms",
                "triples_out",
            ]
        )
        for r in results:
            writer.writerow(
                [
                    r.reasoner,
                    r.target_triples,
                    r.actual_triples,
                    r.repeat_index,
                    f"{r.parse_ms:.3f}",
                    f"{r.reason_ms:.3f}",
                    r.triples_out,
                ]
            )


def summarise(results: list[RunResult]) -> dict:
    """Collapse repeated runs to median reason_ms per (reasoner, size)."""
    by_key: dict[tuple[str, int], list[float]] = {}
    actuals: dict[int, int] = {}
    outputs: dict[tuple[str, int], int] = {}
    for r in results:
        by_key.setdefault((r.reasoner, r.target_triples), []).append(r.reason_ms)
        actuals[r.target_triples] = r.actual_triples
        outputs[(r.reasoner, r.target_triples)] = r.triples_out
    summary = {}
    for (reasoner, target), samples in by_key.items():
        summary.setdefault(reasoner, []).append(
            {
                "target_triples": target,
                "actual_triples": actuals[target],
                "reason_ms_median": statistics.median(samples),
                "reason_ms_min": min(samples),
                "reason_ms_max": max(samples),
                "triples_out": outputs[(reasoner, target)],
                "repeats": len(samples),
            }
        )
    for rows in summary.values():
        rows.sort(key=lambda x: x["target_triples"])
    return summary


def plot(summary: dict, path: Path) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, ax = plt.subplots(figsize=(8, 5))
    colors = {
        "pyoxigraph": "#1f77b4",
        "owlrl": "#d62728",
        "reasonable": "#2ca02c",
    }
    for reasoner, rows in summary.items():
        xs = [row["actual_triples"] for row in rows]
        ys = [row["reason_ms_median"] for row in rows]
        ax.plot(
            xs,
            ys,
            marker="o",
            label=reasoner,
            color=colors.get(reasoner),
        )

    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("input triples (log scale)")
    ax.set_ylabel("reasoning wall clock (ms, log scale)")
    ax.set_title("OWL 2 RL reasoner throughput on LUBM-style fixtures")
    ax.grid(True, which="both", alpha=0.3)
    ax.legend()
    fig.tight_layout()
    path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(path, dpi=150)
    print(f"plot written to {path}")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--sizes",
        type=int,
        nargs="+",
        default=[100, 300, 1000, 3000, 10000, 30000, 100000],
        help="target triple counts to benchmark",
    )
    parser.add_argument(
        "--repeats",
        type=int,
        default=3,
        help="repeats per (reasoner, size) cell (default 3)",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=HERE / "out",
        help="directory for generated fixtures, CSV, JSON, and PNG",
    )
    parser.add_argument(
        "--only",
        nargs="+",
        choices=list(REASONERS.keys()),
        default=list(REASONERS.keys()),
        help="restrict to a subset of reasoners",
    )
    args = parser.parse_args()

    data_dir = args.output_dir / "data"
    data_dir.mkdir(parents=True, exist_ok=True)

    all_results: list[RunResult] = []
    for size in args.sizes:
        ttl = ensure_fixture(size, data_dir)
        actual = count_triples(ttl)
        print(f"\n== target={size} actual_triples={actual} ({ttl.name}) ==")
        for name in args.only:
            fn = REASONERS[name]
            all_results.extend(run_cell(name, fn, ttl, size, actual, args.repeats))

    csv_path = args.output_dir / "results.csv"
    json_path = args.output_dir / "summary.json"
    plot_path = args.output_dir / "reasoner_comparison.png"

    write_csv(all_results, csv_path)
    summary = summarise(all_results)
    json_path.write_text(json.dumps(summary, indent=2), encoding="utf-8")

    try:
        plot(summary, plot_path)
    except ImportError as exc:
        print(f"skipping plot: matplotlib not installed ({exc})", file=sys.stderr)

    print(f"\nCSV:  {csv_path}")
    print(f"JSON: {json_path}")
    print(f"PNG:  {plot_path}")


if __name__ == "__main__":
    main()
