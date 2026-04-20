#!/usr/bin/env python3
"""Benchmark OWL 2 RL reasoners against LUBM-style synthetic graphs.

Reasoners under test:

* ``oxreason`` (native Rust): timed inside a Rust bench binary so Python
  overhead stays out of parse and reasoning durations.
* ``oxreason-eq`` (native Rust): same binary, equality rules enabled.
* ``reasonable`` (native Rust, via its own binding): also timed inside the
  same Rust bench binary against the ``reasonable`` crate directly.
* ``owlrl`` (rdflib + owlrl): the pure Python reference implementation from
  https://github.com/RDFLib/OWL-RL. Timed in process because it has no
  Rust counterpart.

The Rust bench binary lives at ``bench/reasoner/native`` and is invoked
as a subprocess once per (reasoner, size, repeat) cell. It prints a
single JSON line with parse_ms, reason_ms, triples_in, triples_out,
rounds, and firings. This keeps the playing field even between oxreason
and reasonable: both are timed in native code against their own native
index, with no Python parse or insert on the hot path.

Results are written to CSV and to a Matplotlib PNG plot.

Usage::

    # build the Rust bench binary first
    cargo build --release --manifest-path bench/reasoner/native/Cargo.toml

    python bench/reasoner/bench.py --sizes 100 300 1000 3000 10000 30000 100000 \\
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
REPO_ROOT = HERE.parent.parent
NATIVE_BIN_DEFAULT = REPO_ROOT / "bench" / "reasoner" / "native" / "target" / "release" / "reasoner_bench"


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


def make_native_runner(binary: Path, reasoner_key: str) -> Callable[[Path], tuple[float, float, int]]:
    """Return a runner that subprocesses the Rust bench binary.

    ``reasoner_key`` is one of ``oxreason``, ``oxreason-eq``, ``reasonable``.
    The returned callable parses the single JSON line the binary prints
    and returns ``(parse_ms, reason_ms, triples_out)`` to match the other
    adapters.
    """

    def runner(ttl_path: Path) -> tuple[float, float, int]:
        proc = subprocess.run(
            [str(binary), reasoner_key, str(ttl_path)],
            check=True,
            capture_output=True,
            text=True,
        )
        line = proc.stdout.strip().splitlines()[-1]
        payload = json.loads(line)
        return (
            float(payload["parse_ms"]),
            float(payload["reason_ms"]),
            int(payload["triples_out"]),
        )

    return runner


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
        except subprocess.CalledProcessError as exc:
            stderr = (exc.stderr or "").strip()
            print(
                f"  [{reasoner_name}] repeat {i}: FAILED (exit {exc.returncode}): {stderr}",
                file=sys.stderr,
            )
            continue
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
    summary: dict = {}
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
        "oxreason": "#1f77b4",
        "oxreason-eq": "#9467bd",
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
        "--native-bin",
        type=Path,
        default=NATIVE_BIN_DEFAULT,
        help="path to the Rust bench binary (reasoner_bench)",
    )
    parser.add_argument(
        "--only",
        nargs="+",
        default=None,
        help=(
            "restrict to a subset of reasoners from "
            "{oxreason, oxreason-eq, reasonable, owlrl}"
        ),
    )
    args = parser.parse_args()

    all_reasoners = ["oxreason", "oxreason-eq", "reasonable", "owlrl"]
    selected = args.only if args.only is not None else all_reasoners
    for name in selected:
        if name not in all_reasoners:
            parser.error(f"unknown reasoner '{name}'; expected one of {all_reasoners}")

    native_reasoners = {"oxreason", "oxreason-eq", "reasonable"}
    if any(name in native_reasoners for name in selected):
        if not args.native_bin.exists():
            parser.error(
                f"native bench binary not found at {args.native_bin}. "
                "Build it with: cargo build --release --manifest-path "
                "bench/reasoner/native/Cargo.toml"
            )

    runners: dict[str, Callable[[Path], tuple[float, float, int]]] = {}
    for name in selected:
        if name in native_reasoners:
            runners[name] = make_native_runner(args.native_bin, name)
        elif name == "owlrl":
            runners[name] = run_owlrl

    data_dir = args.output_dir / "data"
    data_dir.mkdir(parents=True, exist_ok=True)

    all_results: list[RunResult] = []
    for size in args.sizes:
        ttl = ensure_fixture(size, data_dir)
        actual = count_triples(ttl)
        print(f"\n== target={size} actual_triples={actual} ({ttl.name}) ==")
        for name in selected:
            all_results.extend(
                run_cell(name, runners[name], ttl, size, actual, args.repeats)
            )

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
