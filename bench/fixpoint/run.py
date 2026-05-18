#!/usr/bin/env python3
"""
Driver for the fixpoint comparison benchmark.

For each (engine, size, repeat) cell, the driver:
1. Ensures a turtle fixture of that size exists (calling generate.py if not).
2. Subprocesses the native bench binary `fixpoint_bench <engine> <ttl>`.
3. Parses the single JSON line returned and stores the row.

After the matrix completes, the driver writes results.csv, summary.json, and
a comparison PNG.

Build the native binary once before running:

    cargo build --release --manifest-path bench/fixpoint/native/Cargo.toml

Then:

    python bench/fixpoint/run.py \\
        --sizes small medium large \\
        --engines standard datafusion reasonable \\
        --repeats 3 \\
        --output-dir bench/fixpoint/out

Predefined sizes are tuned to span four orders of magnitude in input triple
count, from ~5k for `small` up to ~1M for `huge`. Override with `--size-spec
name=depth,branching,instances` if you want custom shapes.
"""

import argparse
import csv
import json
import statistics
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

import matplotlib.pyplot as plt


PRESET_SIZES = {
    # name: (depth, branching, instances-per-leaf)
    "small":  (4, 3, 20),    # ~1.7k input triples
    "medium": (5, 4, 30),    # ~31k input triples
    "large":  (6, 4, 50),    # ~205k input triples
    "huge":   (6, 5, 80),    # ~1.25M input triples
}

DEFAULT_ENGINES = ("standard", "datafusion", "reasonable")


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--sizes", nargs="+", default=list(PRESET_SIZES.keys()), help="size presets to run")
    p.add_argument("--engines", nargs="+", default=list(DEFAULT_ENGINES), help="engines to run")
    p.add_argument("--repeats", type=int, default=3)
    p.add_argument(
        "--timeout",
        type=float,
        default=120.0,
        help="seconds per single run before the engine is recorded as timed out (default 120)",
    )
    p.add_argument(
        "--oxigraph-bin",
        type=Path,
        default=Path("bench/fixpoint/native/target/release/fixpoint_bench_oxigraph"),
        help="path to the oxigraph-side bench binary",
    )
    p.add_argument(
        "--reasonable-bin",
        type=Path,
        default=Path("bench/fixpoint/native_reasonable/target/release/fixpoint_bench_reasonable"),
        help="path to the reasonable bench binary",
    )
    p.add_argument("--data-dir", type=Path, default=Path("bench/fixpoint/data"))
    p.add_argument("--output-dir", type=Path, default=Path("bench/fixpoint/out"))
    p.add_argument(
        "--size-spec",
        action="append",
        default=[],
        help="custom size spec: name=depth,branching,instances (repeatable)",
    )
    return p.parse_args()


def resolve_sizes(args: argparse.Namespace) -> dict[str, tuple[int, int, int]]:
    sizes = dict(PRESET_SIZES)
    for spec in args.size_spec:
        try:
            name, rhs = spec.split("=", 1)
            depth_s, branch_s, inst_s = rhs.split(",")
            sizes[name] = (int(depth_s), int(branch_s), int(inst_s))
        except Exception:
            sys.exit(f"invalid --size-spec {spec!r}; expected name=depth,branching,instances")
    chosen = {}
    for name in args.sizes:
        if name not in sizes:
            sys.exit(f"unknown size {name!r}. Known: {sorted(sizes)}")
        chosen[name] = sizes[name]
    return chosen


def ensure_fixture(name: str, spec: tuple[int, int, int], data_dir: Path) -> Path:
    data_dir.mkdir(parents=True, exist_ok=True)
    ttl = data_dir / f"{name}.ttl"
    if ttl.exists():
        return ttl
    depth, branching, instances = spec
    cmd = [
        sys.executable,
        str(Path(__file__).parent / "generate.py"),
        "--depth",
        str(depth),
        "--branching",
        str(branching),
        "--instances-per-leaf",
        str(instances),
        "--output",
        str(ttl),
    ]
    subprocess.run(cmd, check=True)
    return ttl


def binary_for_engine(args: argparse.Namespace, engine: str) -> Path:
    if engine in ("standard", "datafusion"):
        return args.oxigraph_bin
    if engine == "reasonable":
        return args.reasonable_bin
    sys.exit(f"unknown engine {engine!r}")


def run_one(binary: Path, engine: str, ttl: Path, timeout_s: float) -> dict:
    try:
        result = subprocess.run(
            [str(binary), engine, str(ttl)],
            capture_output=True,
            text=True,
            timeout=timeout_s,
        )
    except subprocess.TimeoutExpired:
        return {
            "engine": engine,
            "load_ms": None,
            "compute_ms": None,
            "triples_in": None,
            "answer_count": None,
            "timed_out": True,
        }
    if result.returncode != 0:
        sys.exit(
            f"{binary.name} {engine} {ttl} exited {result.returncode}:\n"
            f"{result.stderr}"
        )
    line = result.stdout.strip().splitlines()[-1]
    row = json.loads(line)
    row["timed_out"] = False
    return row


def run_matrix(args: argparse.Namespace) -> tuple[list[dict], dict[str, tuple[int, int, int]]]:
    sizes = resolve_sizes(args)
    needed_binaries = {binary_for_engine(args, e) for e in args.engines}
    for binary in needed_binaries:
        if not binary.is_file():
            sys.exit(
                f"bench binary not found at {binary}. Build both binaries first:\n"
                "  cargo build --release --manifest-path bench/fixpoint/native/Cargo.toml\n"
                "  cargo build --release --manifest-path bench/fixpoint/native_reasonable/Cargo.toml"
            )
    rows: list[dict] = []
    # Engines that have already timed out at a smaller size are skipped at
    # later (bigger) sizes, since they can only get slower from there.
    timed_out: set[str] = set()
    for size_name, spec in sizes.items():
        ttl = ensure_fixture(size_name, spec, args.data_dir)
        for engine in args.engines:
            if engine in timed_out:
                print(f"{size_name:>8} {engine:>10}: skipped (timed out at smaller size)")
                continue
            binary = binary_for_engine(args, engine)
            for repeat in range(args.repeats):
                row = run_one(binary, engine, ttl, args.timeout)
                row["size"] = size_name
                row["repeat"] = repeat
                rows.append(row)
                if row.get("timed_out"):
                    print(
                        f"{size_name:>8} {engine:>10} repeat {repeat}: "
                        f"timed out after {args.timeout:.1f} s"
                    )
                    timed_out.add(engine)
                    break
                print(
                    f"{size_name:>8} {engine:>10} repeat {repeat}: "
                    f"load {row['load_ms']:>8.1f} ms, compute {row['compute_ms']:>8.1f} ms, "
                    f"answer {row['answer_count']}"
                )
    return rows, sizes


def write_outputs(rows: list[dict], sizes: dict[str, tuple[int, int, int]], out_dir: Path) -> None:
    out_dir.mkdir(parents=True, exist_ok=True)

    # results.csv: one row per run.
    csv_path = out_dir / "results.csv"
    fieldnames = ["size", "engine", "repeat", "triples_in", "answer_count", "load_ms", "compute_ms", "timed_out"]
    with csv_path.open("w", newline="") as f:
        writer = csv.DictWriter(f, fieldnames=fieldnames)
        writer.writeheader()
        for row in rows:
            writer.writerow({k: row.get(k) for k in fieldnames})
    print(f"Wrote {csv_path}")

    # summary.json: median load_ms, compute_ms, total_ms per (engine, size).
    summary: dict[str, list[dict]] = defaultdict(list)
    by_key: dict[tuple[str, str], list[dict]] = defaultdict(list)
    for row in rows:
        by_key[(row["engine"], row["size"])].append(row)
    for (engine, size), runs in sorted(by_key.items()):
        good = [r for r in runs if not r.get("timed_out")]
        if not good:
            summary[engine].append(
                {
                    "size": size,
                    "timed_out": True,
                    "repeats": len(runs),
                }
            )
            continue
        load_med = statistics.median(r["load_ms"] for r in good)
        compute_med = statistics.median(r["compute_ms"] for r in good)
        triples_in = good[0]["triples_in"]
        answer = good[0]["answer_count"]
        summary[engine].append(
            {
                "size": size,
                "triples_in": triples_in,
                "answer_count": answer,
                "load_ms_median": load_med,
                "compute_ms_median": compute_med,
                "total_ms_median": load_med + compute_med,
                "repeats": len(good),
                "timed_out": False,
            }
        )
    json_path = out_dir / "summary.json"
    with json_path.open("w") as f:
        json.dump(summary, f, indent=2)
    print(f"Wrote {json_path}")

    render_plot(summary, out_dir / "fixpoint_comparison.png")


def render_plot(summary: dict[str, list[dict]], output: Path) -> None:
    fig, (ax_compute, ax_total) = plt.subplots(1, 2, figsize=(13, 5))
    colors = {"standard": "tab:blue", "datafusion": "tab:red", "reasonable": "tab:green"}

    for ax, key, title in (
        (ax_compute, "compute_ms_median", "compute time (load excluded)"),
        (ax_total, "total_ms_median", "total time (load + compute)"),
    ):
        for engine, samples in summary.items():
            usable = [s for s in samples if not s.get("timed_out")]
            samples_sorted = sorted(usable, key=lambda r: r["triples_in"])
            if not samples_sorted:
                continue
            xs = [s["triples_in"] for s in samples_sorted]
            ys = [s[key] for s in samples_sorted]
            ax.plot(xs, ys, marker="o", label=engine, color=colors.get(engine))
        ax.set_xscale("log")
        ax.set_yscale("log")
        ax.set_title(title)
        ax.set_xlabel("input triples (log)")
        ax.set_ylabel("median ms (log)")
        ax.grid(True, which="both", linestyle=":", linewidth=0.5)
        ax.legend(loc="best", fontsize=9)

    fig.suptitle("Subclass closure: standard SPARQL vs DataFusion vs reasonable", fontsize=12)
    fig.tight_layout(rect=(0, 0, 1, 0.95))
    fig.savefig(output, dpi=130)
    print(f"Wrote {output}")


def main() -> None:
    args = parse_args()
    rows, sizes = run_matrix(args)
    write_outputs(rows, sizes, args.output_dir)


if __name__ == "__main__":
    main()
