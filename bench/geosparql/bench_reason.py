#!/usr/bin/env python3
"""Drive the spatial reasoning benchmark and plot the results.

Invokes ``spargeo_bench reason <entities> <profile>`` over a sweep of
entity counts and reasoning profiles, collects the JSON line each run
prints on stdout, writes a CSV and a JSON summary, and produces a
multi panel matplotlib figure.

The Rust binary loads an in-process synthetic Polish geodata graph,
optionally runs OWL 2 RL or RDFS forward chaining via
``Store::reason``, then executes a point-in-bbox spatial query against
Warsaw. See ``bench/geosparql/native/src/main.rs`` for the measurement
phases and exact JSON payload.

Usage::

    cargo build --release --manifest-path bench/geosparql/native/Cargo.toml

    python bench/geosparql/bench_reason.py \\
        --sizes 1000 3000 10000 30000 100000 \\
        --profiles none rdfs owl2rl owl2rl-eq \\
        --repeats 3 \\
        --output-dir bench/geosparql/out_reason
"""

from __future__ import annotations

import argparse
import csv
import json
import statistics
import subprocess
import sys
from dataclasses import asdict, dataclass
from pathlib import Path

HERE = Path(__file__).resolve().parent
REPO_ROOT = HERE.parent.parent
NATIVE_BIN_DEFAULT = REPO_ROOT / "bench" / "geosparql" / "native" / "target" / "release" / "spargeo_bench"

PROFILES_ALL = ["none", "rdfs", "owl2rl", "owl2rl-eq"]

# Colour per profile, reused across every subplot so visual identity
# stays consistent in the rendered PNG.
PROFILE_COLORS = {
    "none": "#7f7f7f",
    "rdfs": "#1f77b4",
    "owl2rl": "#2ca02c",
    "owl2rl-eq": "#d62728",
}


@dataclass
class ReasonResult:
    """One row of the raw results table, one per binary invocation."""

    entities: int
    profile: str
    repeat_index: int
    load_ms: float
    reason_ms: float
    added: int
    rounds: int
    firings: int
    peak_rss_mb: float
    on_disk_mb_before: float
    on_disk_mb_after: float
    query_ms: float
    query_matches: int
    geometries_scanned: int


def run_cell(
    binary: Path,
    entities: int,
    profile: str,
    repeats: int,
) -> list[ReasonResult]:
    """Run the bench binary ``repeats`` times for one (size, profile) cell.

    The binary is restarted per repeat so peak RSS and on-disk size are
    measured from a cold process each time. Failures are logged and
    the cell continues to the next repeat so one bad run does not lose
    the whole sweep.
    """
    results: list[ReasonResult] = []
    for i in range(repeats):
        try:
            proc = subprocess.run(
                [str(binary), "reason", str(entities), profile],
                check=True,
                capture_output=True,
                text=True,
            )
        except subprocess.CalledProcessError as exc:
            stderr = (exc.stderr or "").strip()
            print(
                f"  [{profile}/{entities}] repeat {i}: FAILED (exit {exc.returncode}): {stderr}",
                file=sys.stderr,
            )
            continue

        line = proc.stdout.strip().splitlines()[-1]
        payload = json.loads(line)
        row = ReasonResult(
            entities=int(payload["entities"]),
            profile=str(payload["profile"]),
            repeat_index=i,
            load_ms=float(payload["load_ms"]),
            reason_ms=float(payload["reason_ms"]),
            added=int(payload["added"]),
            rounds=int(payload["rounds"]),
            firings=int(payload["firings"]),
            peak_rss_mb=float(payload["peak_rss_mb"]),
            on_disk_mb_before=float(payload["on_disk_mb_before"]),
            on_disk_mb_after=float(payload["on_disk_mb_after"]),
            query_ms=float(payload["query_ms"]),
            query_matches=int(payload["query_matches"]),
            geometries_scanned=int(payload["geometries_scanned"]),
        )
        results.append(row)
        print(
            f"  [{profile}/{entities}] repeat {i}: "
            f"load={row.load_ms:.1f}ms "
            f"reason={row.reason_ms:.1f}ms "
            f"added={row.added} "
            f"rss={row.peak_rss_mb:.1f}MiB "
            f"query={row.query_ms:.1f}ms "
            f"matches={row.query_matches}"
        )
    return results


def write_csv(results: list[ReasonResult], path: Path) -> None:
    """Write the raw per-run table. One row per binary invocation."""
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.writer(fh)
        if not results:
            writer.writerow(list(ReasonResult.__dataclass_fields__.keys()))
            return
        writer.writerow(list(asdict(results[0]).keys()))
        for r in results:
            writer.writerow(list(asdict(r).values()))


def summarise(results: list[ReasonResult]) -> dict:
    """Aggregate raw rows into (profile, entities) medians plus min/max.

    One entry per (profile, entities) cell; numeric fields get a median
    summary, categorical counters (added, rounds, firings) are kept
    from the median sample so they remain integer valued.
    """
    grouped: dict[tuple[str, int], list[ReasonResult]] = {}
    for r in results:
        grouped.setdefault((r.profile, r.entities), []).append(r)

    summary: dict[str, list[dict]] = {}
    for (profile, entities), rows in grouped.items():
        load_samples = [r.load_ms for r in rows]
        reason_samples = [r.reason_ms for r in rows]
        query_samples = [r.query_ms for r in rows]
        rss_samples = [r.peak_rss_mb for r in rows]
        db_before = [r.on_disk_mb_before for r in rows]
        db_after = [r.on_disk_mb_after for r in rows]
        summary.setdefault(profile, []).append(
            {
                "entities": entities,
                "repeats": len(rows),
                "load_ms_median": statistics.median(load_samples),
                "reason_ms_median": statistics.median(reason_samples),
                "reason_ms_min": min(reason_samples),
                "reason_ms_max": max(reason_samples),
                "query_ms_median": statistics.median(query_samples),
                "peak_rss_mb_median": statistics.median(rss_samples),
                "on_disk_mb_before_median": statistics.median(db_before),
                "on_disk_mb_after_median": statistics.median(db_after),
                "added": rows[0].added,
                "rounds": rows[0].rounds,
                "firings": rows[0].firings,
                "query_matches": rows[0].query_matches,
                "geometries_scanned": rows[0].geometries_scanned,
            }
        )
    for rows in summary.values():
        rows.sort(key=lambda x: x["entities"])
    return summary


def plot(summary: dict, path: Path) -> None:
    """Render the 2 by 2 diagnostic figure used in the README.

    Panels, left to right, top to bottom:
      1. reason_ms vs entities (log log).
      2. peak_rss_mb vs entities (log log).
      3. on_disk_mb_after vs entities (log log).
      4. added triples vs entities (log log).

    The ``none`` profile appears as a reference: its reason_ms is
    zero (plotted at 0.01ms so log scale stays happy) and its added
    is zero. The other profiles tell the real story.
    """
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, axes = plt.subplots(2, 2, figsize=(12, 9))
    (ax_reason, ax_rss), (ax_disk, ax_added) = axes

    def sorted_profiles(keys) -> list[str]:
        # Show profiles in a stable, meaningful order.
        ordered = [p for p in PROFILES_ALL if p in keys]
        return ordered + [p for p in keys if p not in PROFILES_ALL]

    for profile in sorted_profiles(summary.keys()):
        rows = summary[profile]
        xs = [row["entities"] for row in rows]
        color = PROFILE_COLORS.get(profile)

        reason_values = [max(row["reason_ms_median"], 0.01) for row in rows]
        ax_reason.plot(xs, reason_values, marker="o", label=profile, color=color)

        rss_values = [row["peak_rss_mb_median"] for row in rows]
        ax_rss.plot(xs, rss_values, marker="o", label=profile, color=color)

        disk_values = [row["on_disk_mb_after_median"] for row in rows]
        ax_disk.plot(xs, disk_values, marker="o", label=profile, color=color)

        added_values = [max(row["added"], 1) for row in rows]
        ax_added.plot(xs, added_values, marker="o", label=profile, color=color)

    for ax in (ax_reason, ax_rss, ax_disk, ax_added):
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_xlabel("entities (log scale)")
        ax.grid(True, which="both", alpha=0.3)
        ax.legend(fontsize=8)

    ax_reason.set_ylabel("reason_ms (log scale)")
    ax_reason.set_title("Reasoning wall clock")
    ax_rss.set_ylabel("peak RSS (MiB, log scale)")
    ax_rss.set_title("Peak resident memory (V1 in memory materialisation)")
    ax_disk.set_ylabel("RocksDB size after reasoning (MiB, log scale)")
    ax_disk.set_title("On-disk footprint after write back")
    ax_added.set_ylabel("inferred triples (log scale)")
    ax_added.set_title("Materialised closure size")

    fig.suptitle(
        "Oxigraph Store::reason over synthetic Polish geodata "
        "(Building/Parcel/Road ABox + OWL 2 RL T-Box)",
        fontsize=13,
    )
    fig.tight_layout(rect=[0.0, 0.0, 1.0, 0.96])
    path.parent.mkdir(parents=True, exist_ok=True)
    fig.savefig(path, dpi=150)
    print(f"plot written to {path}")


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__.splitlines()[0] if __doc__ else None
    )
    parser.add_argument(
        "--sizes",
        type=int,
        nargs="+",
        default=[1000, 3000, 10000, 30000, 100000],
        help="entity counts to sweep (T-Box is fixed, ABox scales with this)",
    )
    parser.add_argument(
        "--profiles",
        nargs="+",
        default=PROFILES_ALL,
        help=f"reasoning profiles to sweep; valid values: {PROFILES_ALL}",
    )
    parser.add_argument(
        "--repeats",
        type=int,
        default=3,
        help="repeats per (size, profile) cell (default 3)",
    )
    parser.add_argument(
        "--output-dir",
        type=Path,
        default=HERE / "out_reason",
        help="directory for CSV, JSON summary, and PNG",
    )
    parser.add_argument(
        "--native-bin",
        type=Path,
        default=NATIVE_BIN_DEFAULT,
        help="path to the Rust bench binary (spargeo_bench)",
    )
    args = parser.parse_args()

    for profile in args.profiles:
        if profile not in PROFILES_ALL:
            parser.error(
                f"unknown profile '{profile}'; expected one of {PROFILES_ALL}"
            )

    if not args.native_bin.exists():
        parser.error(
            f"native bench binary not found at {args.native_bin}. "
            "Build it with: cargo build --release --manifest-path "
            "bench/geosparql/native/Cargo.toml --features reasoning"
        )

    args.output_dir.mkdir(parents=True, exist_ok=True)

    all_results: list[ReasonResult] = []
    for size in args.sizes:
        print(f"\n== entities={size} ==")
        for profile in args.profiles:
            all_results.extend(run_cell(args.native_bin, size, profile, args.repeats))

    csv_path = args.output_dir / "results.csv"
    json_path = args.output_dir / "summary.json"
    plot_path = args.output_dir / "reason_benchmark.png"

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
