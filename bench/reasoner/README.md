# OWL 2 RL reasoner benchmark

This directory contains a small benchmark pipeline that compares three OWL 2
RL reasoners on LUBM-style synthetic graphs:

* `pyoxigraph.Reasoner` from this repository (backed by the `oxreason` crate)
* [OWL-RL](https://github.com/RDFLib/OWL-RL) (`owlrl` + rdflib, pure Python)
* [reasonable](https://github.com/gtfierro/reasonable) (Rust, Python binding)

The pipeline has three pieces:

* `generate_lubm.py` synthesises Turtle fixtures of a tunable size.
* `bench.py` parses each fixture into each reasoner's native data structure,
  runs reasoning, and records wall-clock durations.
* `bench.py` also emits a Matplotlib PNG comparing the reasoners across sizes.

## Prerequisites

Build `pyoxigraph` with the new `Reasoner` binding and install the Python
reasoners:

```
# from the repository root
cd python
uv run maturin develop --release
cd ..

pip install rdflib owlrl reasonable matplotlib
```

The `pyoxigraph` build produces a Python wheel that exposes `Reasoner` and
`ReasoningReport` from the `pyoxigraph` package.

## Running the benchmark

```
python bench/reasoner/bench.py \
    --sizes 100 300 1000 3000 10000 30000 100000 \
    --repeats 3 \
    --output-dir bench/reasoner/out
```

Outputs land in the `--output-dir`:

* `data/lubm_<size>.ttl` cached Turtle fixtures
* `results.csv` one row per run, raw numbers
* `summary.json` median, min, and max per (reasoner, size)
* `reasoner_comparison.png` log-log plot of reasoning duration vs input size

`--only` narrows the run to a subset of reasoners, useful for debugging:

```
python bench/reasoner/bench.py --only pyoxigraph --sizes 100 1000 10000
```

## Workload notes

The LUBM-style ontology is encoded directly in `generate_lubm.py`. It
includes a sub-class hierarchy (`University`, `Department`, `Faculty`,
`Student` and their subclasses), `rdfs:subPropertyOf` chains on
`worksFor`/`memberOf`/`headOf`, `owl:inverseOf`, a transitive
`subOrganizationOf`, and a symmetric `colleagueOf`. Each size setting
scales the university count and picks the one whose estimated triple count
is closest to the target.
