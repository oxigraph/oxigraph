# OWL 2 RL reasoner benchmark

This directory contains a benchmark pipeline that compares OWL 2 RL reasoners
on LUBM-style synthetic graphs:

* `oxreason` from this repository, timed inside a Rust bench binary.
* [reasonable](https://github.com/gtfierro/reasonable) (Rust), also timed
  inside the same Rust bench binary via its crates.io release.
* [OWL-RL](https://github.com/RDFLib/OWL-RL) (`owlrl` + rdflib, pure Python),
  timed in the Python driver because it has no Rust counterpart.

Timing the two Rust reasoners inside one native binary keeps Python's parse
and FFI costs off the hot path for both. Each `(reasoner, size, repeat)`
cell is a fresh subprocess so there is no warm cache advantage either way.

The pipeline has three pieces:

* `generate_lubm.py` synthesises Turtle fixtures of a tunable size.
* `native/` contains the Rust bench binary that owns parsing and reasoning.
* `bench.py` drives the matrix, shells out to the binary, runs OWL-RL in
  process, and emits CSV, JSON, and a Matplotlib PNG.

## Prerequisites

Build the Rust bench binary:

```
cargo build --release --manifest-path bench/reasoner/native/Cargo.toml
```

Install the Python dependencies used by the driver and by OWL-RL:

```
pip install rdflib owlrl matplotlib
```

The `reasonable` Python package is no longer required: the Rust bench
binary links the `reasonable` crate directly.

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

Useful flags:

* `--only` restricts the run to a subset of
  `{oxreason, reasonable, owlrl}`.
* `--native-bin` points at a different build of the Rust bench binary.

```
python bench/reasoner/bench.py --only oxreason reasonable --sizes 100 1000 10000
```

## Workload notes

The LUBM-style ontology is encoded directly in `generate_lubm.py`. It
includes a sub-class hierarchy (`University`, `Department`, `Faculty`,
`Student` and their subclasses), `rdfs:subPropertyOf` chains on
`worksFor`/`memberOf`/`headOf`, `owl:inverseOf`, a transitive
`subOrganizationOf`, and a symmetric `colleagueOf`. Each size setting
scales the university count and picks the one whose estimated triple count
is closest to the target.

## JSON contract from the native bench binary

The Rust binary at `native/src/main.rs` takes two positional arguments,
`<reasoner>` and `<path-to-turtle>`, and prints a single JSON line to
stdout:

```json
{"reasoner":"oxreason","parse_ms":12.3,"reason_ms":45.6,"triples_in":1234,"triples_out":3456,"rounds":3,"firings":12345}
```

`reasoner` is one of `oxreason` or `reasonable`.
`rounds` and `firings` are `0` for `reasonable` because that crate does
not expose those counters.
