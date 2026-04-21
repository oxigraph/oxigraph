#!/usr/bin/env python3
"""Synthetic GeoSPARQL fixture generator for spargeo benchmarking.

Emits a Turtle file containing:

* ``K`` query polygons scattered across a CRS84 bounding box, each with
  ``geo:hasGeometry`` + ``geo:asWKT`` assertions using square wktLiterals.
* ``N`` candidate points sampled uniformly inside the same bounding box,
  also with ``geo:hasGeometry`` + ``geo:asWKT``.

The fixture is shaped so that a point-in-polygon workload (``geof:sfWithin``
or equivalent) has a non trivial amount of work to do: polygons are small
enough that the match ratio stays well below 100% but above 0%, so every
engine must actually run the topology test rather than short circuiting
on bounding boxes alone.

Triple count is approximate and dominated by the ABox. The caller passes a
target point count; the polygon count is fixed (defaults to 10).

Usage::

    python generate_geodata.py --target-points 10000 --output data/geo_10000.ttl
"""

from __future__ import annotations

import argparse
import random
from pathlib import Path

EX = "https://example.org/geo#"
GEO = "http://www.opengis.net/ont/geosparql#"

# Bounding box roughly covering continental Europe, expressed as
# (min_lon, min_lat, max_lon, max_lat) in CRS84 order (lon, lat).
BBOX = (-10.0, 36.0, 30.0, 60.0)

# Each polygon is a square of side ``POLYGON_SIZE`` degrees centred on a
# random point inside the bounding box. Scale relative to box extent so
# the expected match ratio stays around a few percent.
POLYGON_SIZE = 1.5

PROLOGUE = f"""@prefix : <{EX}> .
@prefix rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix geo:  <{GEO}> .

"""


def generate(num_points: int, num_polygons: int, seed: int = 42) -> str:
    rng = random.Random(seed)
    lines: list[str] = [PROLOGUE]

    min_lon, min_lat, max_lon, max_lat = BBOX
    inner_min_lon = min_lon + POLYGON_SIZE
    inner_max_lon = max_lon - POLYGON_SIZE
    inner_min_lat = min_lat + POLYGON_SIZE
    inner_max_lat = max_lat - POLYGON_SIZE

    for i in range(num_polygons):
        cx = rng.uniform(inner_min_lon, inner_max_lon)
        cy = rng.uniform(inner_min_lat, inner_max_lat)
        half = POLYGON_SIZE / 2.0
        x0, y0 = cx - half, cy - half
        x1, y1 = cx + half, cy - half
        x2, y2 = cx + half, cy + half
        x3, y3 = cx - half, cy + half
        wkt = (
            f"POLYGON(({x0:.6f} {y0:.6f}, "
            f"{x1:.6f} {y1:.6f}, "
            f"{x2:.6f} {y2:.6f}, "
            f"{x3:.6f} {y3:.6f}, "
            f"{x0:.6f} {y0:.6f}))"
        )
        lines.append(f":Region{i} a :Region ;")
        lines.append(f"    geo:hasGeometry :RegionGeom{i} .")
        lines.append(f":RegionGeom{i} a geo:Geometry ;")
        lines.append(
            f'    geo:asWKT "{wkt}"^^geo:wktLiteral .'
        )

    for j in range(num_points):
        x = rng.uniform(min_lon, max_lon)
        y = rng.uniform(min_lat, max_lat)
        wkt = f"POINT({x:.6f} {y:.6f})"
        lines.append(f":Feature{j} a :Feature ;")
        lines.append(f"    geo:hasGeometry :FeatureGeom{j} .")
        lines.append(f":FeatureGeom{j} a geo:Geometry ;")
        lines.append(
            f'    geo:asWKT "{wkt}"^^geo:wktLiteral .'
        )

    return "\n".join(lines) + "\n"


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--target-points",
        type=int,
        required=True,
        help="approximate number of candidate points to emit",
    )
    parser.add_argument(
        "--polygons",
        type=int,
        default=10,
        help="number of query polygons (default 10)",
    )
    parser.add_argument(
        "--output",
        type=Path,
        required=True,
        help="path of the Turtle file to write",
    )
    parser.add_argument(
        "--seed",
        type=int,
        default=42,
        help="random seed (default 42)",
    )
    args = parser.parse_args()

    ttl = generate(args.target_points, args.polygons, seed=args.seed)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(ttl, encoding="utf-8")

    print(
        f"wrote {args.output} "
        f"(points={args.target_points}, polygons={args.polygons})"
    )


if __name__ == "__main__":
    main()
