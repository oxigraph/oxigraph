#!/usr/bin/env python3
"""
LUBM-style synthetic data generator for the fixpoint comparison bench.

Produces a graph with:

1. A bushy rdfs:subClassOf hierarchy of `--depth` levels and `--branching`
   children per node.
2. `--instances-per-leaf` typed instances at every leaf class.

The result exercises the same fixpoint as OWL 2 RL's cax-sco rule plus
property paths' `rdfs:subClassOf*`. The size of the answer to the bench
query is `instances-per-leaf * (depth + 1) * leaves` (every instance gets
an inferred type at every ancestor class up to the root).

Usage:

    python bench/fixpoint/generate.py --depth 6 --branching 3 \\
        --instances-per-leaf 50 --output bench/fixpoint/data/medium.ttl
"""

import argparse
import sys
from pathlib import Path


def parse_args() -> argparse.Namespace:
    p = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    p.add_argument("--depth", type=int, default=6, help="hierarchy depth (default 6)")
    p.add_argument("--branching", type=int, default=3, help="children per class (default 3)")
    p.add_argument(
        "--instances-per-leaf",
        type=int,
        default=50,
        help="instances typed at each leaf class (default 50)",
    )
    p.add_argument("--output", type=Path, required=True, help="output turtle file")
    return p.parse_args()


def build_classes(depth: int, branching: int) -> tuple[list[list[int]], int]:
    """
    Returns (classes_by_level, total_classes). classes_by_level[i] is the
    list of class IDs at level i. Root is level 0. IDs are stable across
    runs so two runs with the same args produce isomorphic graphs.
    """
    levels: list[list[int]] = [[0]]
    next_id = 1
    for _ in range(depth):
        prev = levels[-1]
        nxt: list[int] = []
        for _ in prev:
            for _ in range(branching):
                nxt.append(next_id)
                next_id += 1
        levels.append(nxt)
    return levels, next_id


def main() -> None:
    args = parse_args()
    if args.depth < 0 or args.branching < 1 or args.instances_per_leaf < 0:
        sys.exit("invalid arguments")

    args.output.parent.mkdir(parents=True, exist_ok=True)

    levels, total_classes = build_classes(args.depth, args.branching)
    leaves = levels[-1]
    triple_count = 0

    with args.output.open("w") as f:
        f.write("@prefix rdf: <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .\n")
        f.write("@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .\n")
        f.write("@prefix ex: <http://ex/> .\n\n")

        # rdfs:subClassOf hierarchy: child rdfs:subClassOf parent.
        for level_idx in range(1, len(levels)):
            parents = levels[level_idx - 1]
            children = levels[level_idx]
            # Each parent has `branching` consecutive children.
            for i, c in enumerate(children):
                parent = parents[i // args.branching]
                f.write(f"ex:c{c} rdfs:subClassOf ex:c{parent} .\n")
                triple_count += 1

        # Instances typed at every leaf class.
        instance_id = 0
        for leaf in leaves:
            for _ in range(args.instances_per_leaf):
                f.write(f"ex:i{instance_id} rdf:type ex:c{leaf} .\n")
                triple_count += 1
                instance_id += 1

    expected_answer = args.instances_per_leaf * len(leaves) * (args.depth + 1)
    print(
        f"Wrote {args.output} -- {triple_count} input triples, "
        f"{total_classes} classes, {len(leaves)} leaves, "
        f"{args.instances_per_leaf * len(leaves)} instances. "
        f"Expected answer count: {expected_answer}."
    )


if __name__ == "__main__":
    main()
