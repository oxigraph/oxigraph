#!/usr/bin/env python3
"""LUBM-style synthetic RDF data generator for reasoner benchmarking.

The generator emits Turtle files with a compact LUBM-inspired ontology and
a tunable volume of ABox assertions. The goal is to exercise the rule
families that OWL 2 RL reasoners must handle at scale:

* ``rdfs:subClassOf`` hierarchies exercise ``cax-sco`` and ``scm-sco``
* ``rdfs:subPropertyOf`` hierarchies exercise ``prp-spo1``
* ``rdfs:domain`` and ``rdfs:range`` exercise ``prp-dom`` and ``prp-rng``
* symmetric and transitive properties exercise ``prp-symp`` and ``prp-trp``
* ``owl:inverseOf`` exercises ``prp-inv1`` and ``prp-inv2``

The generator does NOT emit any inconsistency or disjointness declarations,
so the benchmark measures raw materialisation throughput rather than clash
detection.

Triple count is approximate: the ontology (TBox) is fixed size, and the
ABox scales with the number of universities. The caller passes a target
triple count and the generator picks a university count that lands close
to it.

Usage::

    python generate_lubm.py --target-triples 10000 --output data/10k.ttl
"""

from __future__ import annotations

import argparse
import random
from pathlib import Path

EX = "https://example.org/lubm#"

# TBox: a fixed ontology that every size reuses. Keep the rules that
# touch every ABox triple (subClassOf chains, subPropertyOf chains,
# inverseOf on worksFor/hasWorker) so the reasoner does non trivial work.
TBOX = f"""@prefix : <{EX}> .
@prefix rdf:  <http://www.w3.org/1999/02/22-rdf-syntax-ns#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix owl:  <http://www.w3.org/2002/07/owl#> .

:Organization a owl:Class .
:University a owl:Class ; rdfs:subClassOf :Organization .
:Department a owl:Class ; rdfs:subClassOf :Organization .
:ResearchGroup a owl:Class ; rdfs:subClassOf :Organization .

:Person a owl:Class .
:Employee a owl:Class ; rdfs:subClassOf :Person .
:Faculty a owl:Class ; rdfs:subClassOf :Employee .
:Professor a owl:Class ; rdfs:subClassOf :Faculty .
:FullProfessor a owl:Class ; rdfs:subClassOf :Professor .
:AssociateProfessor a owl:Class ; rdfs:subClassOf :Professor .
:AssistantProfessor a owl:Class ; rdfs:subClassOf :Professor .
:Lecturer a owl:Class ; rdfs:subClassOf :Faculty .
:Student a owl:Class ; rdfs:subClassOf :Person .
:UndergraduateStudent a owl:Class ; rdfs:subClassOf :Student .
:GraduateStudent a owl:Class ; rdfs:subClassOf :Student .

:affiliatedWith a owl:ObjectProperty ;
                rdfs:domain :Person ;
                rdfs:range :Organization .
:worksFor a owl:ObjectProperty ;
          rdfs:subPropertyOf :affiliatedWith ;
          rdfs:domain :Employee ;
          rdfs:range :Organization .
:memberOf a owl:ObjectProperty ;
          rdfs:subPropertyOf :affiliatedWith ;
          rdfs:domain :Person ;
          rdfs:range :Organization .
:headOf a owl:ObjectProperty ;
        rdfs:subPropertyOf :worksFor .

:hasWorker a owl:ObjectProperty ;
           owl:inverseOf :worksFor .
:hasMember a owl:ObjectProperty ;
           owl:inverseOf :memberOf .

:subOrganizationOf a owl:ObjectProperty , owl:TransitiveProperty ;
                   rdfs:domain :Organization ;
                   rdfs:range :Organization .
:colleagueOf a owl:ObjectProperty , owl:SymmetricProperty ;
             rdfs:domain :Person ;
             rdfs:range :Person .
"""


# Per university counts that produce roughly 50 ABox triples per university
# of coherent data. Scale linearly via `num_universities`.
DEPTS_PER_UNI = 4
GROUPS_PER_DEPT = 2
FACULTY_PER_DEPT = 5
STUDENTS_PER_DEPT = 10
COLLEAGUE_EDGES_PER_DEPT = 4


def generate(num_universities: int, seed: int = 42) -> str:
    """Generate Turtle ABox for `num_universities` universities.

    Deterministic when `seed` is fixed.
    """
    rng = random.Random(seed)
    lines: list[str] = [TBOX]

    def emit(subj: str, pred: str, obj: str) -> None:
        lines.append(f":{subj} {pred} :{obj} .")

    def cls(subj: str, class_name: str) -> None:
        lines.append(f":{subj} rdf:type :{class_name} .")

    for u in range(num_universities):
        uni = f"Uni{u}"
        cls(uni, "University")

        for d in range(DEPTS_PER_UNI):
            dept = f"Dept{u}_{d}"
            cls(dept, "Department")
            emit(dept, ":subOrganizationOf", uni)

            for g in range(GROUPS_PER_DEPT):
                grp = f"Group{u}_{d}_{g}"
                cls(grp, "ResearchGroup")
                emit(grp, ":subOrganizationOf", dept)

            # Faculty
            faculty_ids: list[str] = []
            for f in range(FACULTY_PER_DEPT):
                fac = f"Fac{u}_{d}_{f}"
                # Pick a professor rank for variety
                rank = rng.choice(
                    [
                        "FullProfessor",
                        "AssociateProfessor",
                        "AssistantProfessor",
                        "Lecturer",
                    ]
                )
                cls(fac, rank)
                emit(fac, ":worksFor", dept)
                faculty_ids.append(fac)
            # Head of department
            if faculty_ids:
                emit(faculty_ids[0], ":headOf", dept)

            # Students
            student_ids: list[str] = []
            for s in range(STUDENTS_PER_DEPT):
                stu = f"Stu{u}_{d}_{s}"
                is_grad = s % 3 == 0
                cls(stu, "GraduateStudent" if is_grad else "UndergraduateStudent")
                emit(stu, ":memberOf", dept)
                student_ids.append(stu)

            # Colleague edges inside the faculty; the symmetric rule must
            # materialise the reverse side.
            all_people = faculty_ids + student_ids
            for _ in range(COLLEAGUE_EDGES_PER_DEPT):
                if len(all_people) < 2:
                    break
                a, b = rng.sample(all_people, 2)
                emit(a, ":colleagueOf", b)

    return "\n".join(lines) + "\n"


def estimate_triples(num_universities: int) -> int:
    """Rough pre generation estimate so we can pick `num_universities`."""
    per_uni = (
        1  # University type
        + DEPTS_PER_UNI * (2 + GROUPS_PER_DEPT * 2)  # dept type+subOrg, groups
        + DEPTS_PER_UNI * FACULTY_PER_DEPT * 2  # faculty type+worksFor
        + DEPTS_PER_UNI  # headOf
        + DEPTS_PER_UNI * STUDENTS_PER_DEPT * 2  # student type+memberOf
        + DEPTS_PER_UNI * COLLEAGUE_EDGES_PER_DEPT  # colleague
    )
    tbox_triples = 90  # approximate size of the TBox block
    return tbox_triples + num_universities * per_uni


def universities_for_target(target: int) -> int:
    """Pick the university count that lands the triple count closest to target."""
    # Binary search on estimate_triples.
    lo, hi = 1, 1
    while estimate_triples(hi) < target:
        hi *= 2
    while lo < hi:
        mid = (lo + hi) // 2
        if estimate_triples(mid) < target:
            lo = mid + 1
        else:
            hi = mid
    # `lo` is the smallest count with estimate >= target. Pick it.
    return max(1, lo)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--target-triples",
        type=int,
        required=True,
        help="approximate number of triples to produce (ontology plus ABox)",
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
        help="random seed for colleague edge selection (default 42)",
    )
    args = parser.parse_args()

    num_unis = universities_for_target(args.target_triples)
    ttl = generate(num_unis, seed=args.seed)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(ttl, encoding="utf-8")

    line_count = ttl.count("\n")
    print(
        f"wrote {args.output} "
        f"(universities={num_unis}, approx_triples={line_count})"
    )


if __name__ == "__main__":
    main()
