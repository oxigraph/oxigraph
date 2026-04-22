#!/usr/bin/env python3
"""Benchmark spargeo and competitors on a GeoSPARQL point-in-polygon workload.

Engines under test:

* ``spargeo`` (native Rust): invokes ``geof:sfWithin`` through the public
  ``GEOSPARQL_EXTENSION_FUNCTIONS`` table. Each call reparses the WKT
  literal, matching how the SPARQL evaluator actually drives it.
* ``geo`` (native Rust): calls ``geo::Relate::relate(...).is_within()``
  directly on WKT strings parsed once up front. This is the Rust lower
  bound spargeo could approach if literal parsing were amortised (e.g.
  via the WKB storage proposed in oxigraph issue #1560).
* ``index`` (native Rust): drops the candidate points into
  ``spargeo::index::SpatialIndex`` and calls ``query_within`` for each
  polygon. Measures the ancestor walk plus Hilbert range scan that
  gathers candidates before ``geo::Relate`` refines the result, so
  query time should stay near-constant in ``points`` for a fixed
  polygon set.
* ``shapely`` (Python): the de facto Python geometry reference. Parses
  WKT once via ``shapely.wkt.loads``, then iterates
  ``polygon.contains(point)`` in a tight loop.

The Rust bench binary lives at ``bench/geosparql/native`` and is invoked
once per (engine, size, repeat) cell. It prints a single JSON line with
``parse_ms``, ``query_ms``, ``points``, ``polygons`` and ``matches``.

Usage::

    cargo build --release --manifest-path bench/geosparql/native/Cargo.toml

    python bench/geosparql/bench.py --sizes 100 300 1000 3000 10000 30000 \\
                                    --repeats 3 \\
                                    --output-dir bench/geosparql/out
"""

from __future__ import annotations

import argparse
import csv
import json
import re
import statistics
import subprocess
import sys
import time
from dataclasses import dataclass
from pathlib import Path
from typing import Callable

HERE = Path(__file__).resolve().parent
GENERATE = HERE / "generate_geodata.py"
REPO_ROOT = HERE.parent.parent
NATIVE_BIN_DEFAULT = REPO_ROOT / "bench" / "geosparql" / "native" / "target" / "release" / "spargeo_bench"


@dataclass
class RunResult:
    engine: str
    target_points: int
    actual_points: int
    polygons: int
    parse_ms: float
    query_ms: float
    matches: int
    repeat_index: int


def ensure_fixture(target_points: int, polygons: int, data_dir: Path) -> Path:
    path = data_dir / f"geo_{target_points}.ttl"
    if not path.exists():
        subprocess.run(
            [
                sys.executable,
                str(GENERATE),
                "--target-points",
                str(target_points),
                "--polygons",
                str(polygons),
                "--output",
                str(path),
            ],
            check=True,
        )
    return path


# -----------------------------------------------------------------------------
# Engine adapters
# -----------------------------------------------------------------------------


def make_native_runner(
    binary: Path, engine_key: str
) -> Callable[[Path], tuple[float, float, int, int, int]]:
    def runner(ttl_path: Path) -> tuple[float, float, int, int, int]:
        proc = subprocess.run(
            [str(binary), engine_key, str(ttl_path)],
            check=True,
            capture_output=True,
            text=True,
        )
        line = proc.stdout.strip().splitlines()[-1]
        payload = json.loads(line)
        return (
            float(payload["parse_ms"]),
            float(payload["query_ms"]),
            int(payload["points"]),
            int(payload["polygons"]),
            int(payload["matches"]),
        )

    return runner


# WKT extractor for the Python path. rdflib's Turtle parser is slow enough to
# dominate the timings on larger fixtures; since the fixture format is fully
# predictable we extract WKT payloads with a regex and classify by prefix.
# This matches what the native bench does in its `extract_wkts` helper.
_ASWKT_RE = re.compile(
    r'geo:asWKT\s+"([^"]+)"\^\^geo:wktLiteral'
)


def load_wkts(ttl_path: Path) -> tuple[list[str], list[str]]:
    points: list[str] = []
    polygons: list[str] = []
    with ttl_path.open("r", encoding="utf-8") as fh:
        for match in _ASWKT_RE.finditer(fh.read()):
            value = match.group(1).strip()
            if value.startswith("POINT"):
                points.append(value)
            elif value.startswith("POLYGON"):
                polygons.append(value)
    return points, polygons


def run_shapely(ttl_path: Path) -> tuple[float, float, int, int, int]:
    from shapely import wkt as shapely_wkt

    parse_start = time.perf_counter()
    points_wkt, polygons_wkt = load_wkts(ttl_path)
    points = [shapely_wkt.loads(p) for p in points_wkt]
    polygons = [shapely_wkt.loads(p) for p in polygons_wkt]
    parse_ms = (time.perf_counter() - parse_start) * 1000.0

    query_start = time.perf_counter()
    matches = 0
    for polygon in polygons:
        for point in points:
            if polygon.contains(point):
                matches += 1
    query_ms = (time.perf_counter() - query_start) * 1000.0

    return parse_ms, query_ms, len(points), len(polygons), matches


# -----------------------------------------------------------------------------
# Orchestration
# -----------------------------------------------------------------------------


def run_cell(
    engine_name: str,
    fn: Callable[[Path], tuple[float, float, int, int, int]],
    ttl_path: Path,
    target: int,
    repeats: int,
) -> list[RunResult]:
    results: list[RunResult] = []
    for i in range(repeats):
        try:
            parse_ms, query_ms, points, polygons, matches = fn(ttl_path)
        except subprocess.CalledProcessError as exc:
            stderr = (exc.stderr or "").strip()
            print(
                f"  [{engine_name}] repeat {i}: FAILED (exit {exc.returncode}): {stderr}",
                file=sys.stderr,
            )
            continue
        except Exception as exc:
            print(
                f"  [{engine_name}] repeat {i}: FAILED: {exc}",
                file=sys.stderr,
            )
            continue
        results.append(
            RunResult(
                engine=engine_name,
                target_points=target,
                actual_points=points,
                polygons=polygons,
                parse_ms=parse_ms,
                query_ms=query_ms,
                matches=matches,
                repeat_index=i,
            )
        )
        print(
            f"  [{engine_name}] repeat {i}: "
            f"parse={parse_ms:.1f}ms query={query_ms:.1f}ms "
            f"matches={matches}"
        )
    return results


def write_csv(results: list[RunResult], path: Path) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as fh:
        writer = csv.writer(fh)
        writer.writerow(
            [
                "engine",
                "target_points",
                "actual_points",
                "polygons",
                "repeat",
                "parse_ms",
                "query_ms",
                "matches",
            ]
        )
        for r in results:
            writer.writerow(
                [
                    r.engine,
                    r.target_points,
                    r.actual_points,
                    r.polygons,
                    r.repeat_index,
                    f"{r.parse_ms:.3f}",
                    f"{r.query_ms:.3f}",
                    r.matches,
                ]
            )


def summarise(results: list[RunResult]) -> dict:
    by_key: dict[tuple[str, int], list[float]] = {}
    actuals: dict[int, int] = {}
    matches_map: dict[tuple[str, int], int] = {}
    polygons_map: dict[int, int] = {}
    for r in results:
        by_key.setdefault((r.engine, r.target_points), []).append(r.query_ms)
        actuals[r.target_points] = r.actual_points
        matches_map[(r.engine, r.target_points)] = r.matches
        polygons_map[r.target_points] = r.polygons
    summary: dict = {}
    for (engine, target), samples in by_key.items():
        summary.setdefault(engine, []).append(
            {
                "target_points": target,
                "actual_points": actuals[target],
                "polygons": polygons_map[target],
                "query_ms_median": statistics.median(samples),
                "query_ms_min": min(samples),
                "query_ms_max": max(samples),
                "matches": matches_map[(engine, target)],
                "repeats": len(samples),
            }
        )
    for rows in summary.values():
        rows.sort(key=lambda x: x["target_points"])
    return summary


def plot(summary: dict, path: Path) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    fig, ax = plt.subplots(figsize=(8, 5))
    colors = {
        "spargeo": "#1f77b4",
        "geo": "#2ca02c",
        "index": "#9467bd",
        "shapely": "#d62728",
    }
    for engine, rows in summary.items():
        xs = [row["actual_points"] for row in rows]
        ys = [row["query_ms_median"] for row in rows]
        ax.plot(
            xs,
            ys,
            marker="o",
            label=engine,
            color=colors.get(engine),
        )

    ax.set_xscale("log")
    ax.set_yscale("log")
    ax.set_xlabel("candidate points (log scale)")
    ax.set_ylabel("sfWithin query wall clock (ms, log scale)")
    ax.set_title("GeoSPARQL point-in-polygon throughput by engine")
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
        default=[100, 300, 1000, 3000, 10000, 30000],
        help="target candidate point counts to benchmark",
    )
    parser.add_argument(
        "--polygons",
        type=int,
        default=10,
        help="number of query polygons (default 10, fixed across sizes)",
    )
    parser.add_argument(
        "--repeats",
        type=int,
        default=3,
        help="repeats per (engine, size) cell (default 3)",
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
        help="path to the Rust bench binary (spargeo_bench)",
    )
    parser.add_argument(
        "--only",
        nargs="+",
        default=None,
        help=(
            "restrict to a subset of engines from "
            "{spargeo, geo, index, shapely}"
        ),
    )
    args = parser.parse_args()

    all_engines = ["spargeo", "geo", "index", "shapely"]
    selected = args.only if args.only is not None else all_engines
    for name in selected:
        if name not in all_engines:
            parser.error(f"unknown engine '{name}'; expected one of {all_engines}")

    native_engines = {"spargeo", "geo", "index"}
    if any(name in native_engines for name in selected):
        if not args.native_bin.exists():
            parser.error(
                f"native bench binary not found at {args.native_bin}. "
                "Build it with: cargo build --release --manifest-path "
                "bench/geosparql/native/Cargo.toml"
            )

    runners: dict[str, Callable[[Path], tuple[float, float, int, int, int]]] = {}
    for name in selected:
        if name in native_engines:
            runners[name] = make_native_runner(args.native_bin, name)
        elif name == "shapely":
            runners[name] = run_shapely

    data_dir = args.output_dir / "data"
    data_dir.mkdir(parents=True, exist_ok=True)

    all_results: list[RunResult] = []
    for size in args.sizes:
        ttl = ensure_fixture(size, args.polygons, data_dir)
        print(f"\n== target_points={size} ({ttl.name}) ==")
        for name in selected:
            all_results.extend(
                run_cell(name, runners[name], ttl, size, args.repeats)
            )

    csv_path = args.output_dir / "results.csv"
    json_path = args.output_dir / "summary.json"
    plot_path = args.output_dir / "geosparql_comparison.png"

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
