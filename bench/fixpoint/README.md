# Fixpoint comparison benchmark

Times three engines on the same OWL 2 RL `cax-sco` workload:

* `standard`: oxigraph's per-tuple SPARQL evaluator (`lib/spareval`).
* `datafusion`: the DataFusion-backed evaluator (`lib/spareval-fusion`,
  via `SparqlEvaluator::datafusion`).
* `reasonable`: the [reasonable](https://github.com/gtfierro/reasonable)
  OWL 2 RL reasoner.

The two SPARQL engines answer the query

```sparql
SELECT (COUNT(*) AS ?c) WHERE {
    ?i a/rdfs:subClassOf* ?c
}
```

`reasonable` instead materialises every OWL 2 RL inference and then counts
the resulting `rdf:type` triples. All three produce the same answer count,
so a divergence on that field is a correctness bug.

## Build

Two binaries, one per engine family. They must be separate because
`reasonable 0.2` transitively depends on an old `oxigraph 0.3.5` that
links rocksdb, which collides with the workspace's local
`oxrocksdb-sys` on the cargo `links = "rocksdb"` invariant.

```
cargo build --release --manifest-path bench/fixpoint/native/Cargo.toml
cargo build --release --manifest-path bench/fixpoint/native_reasonable/Cargo.toml
```

This produces:

* `bench/fixpoint/native/target/release/fixpoint_bench_oxigraph` (engines:
  `standard`, `datafusion`)
* `bench/fixpoint/native_reasonable/target/release/fixpoint_bench_reasonable`
  (engine: `reasonable`)

Both are standalone Cargo workspaces so they do not bleed into the outer
workspace's lockfile.

## Run

```
pip install matplotlib
python bench/fixpoint/run.py \
    --sizes small medium large huge \
    --repeats 3
```

Outputs land in `bench/fixpoint/out/`:

* `data/<size>.ttl`: cached turtle fixtures.
* `results.csv`: one row per `(engine, size, repeat)` with `load_ms`,
  `compute_ms`, `triples_in`, `answer_count`.
* `summary.json`: median load and compute per `(engine, size)`.
* `fixpoint_comparison.png`: two subplots, compute time and total time
  (load + compute) on log-log axes.

## Size presets

| Name   | Depth | Branching | Instances/leaf | Approx triples in | Approx answer count |
|--------|-------|-----------|----------------|-------------------|---------------------|
| small  | 4     | 3         | 20             | ~1.7k             | ~8.1k               |
| medium | 5     | 4         | 30             | ~31k              | ~184k               |
| large  | 6     | 4         | 50             | ~205k             | ~1.4M               |
| huge   | 6     | 5         | 80             | ~1.25M            | ~8.7M               |

Use `--size-spec name=depth,branching,instances` to add custom shapes.

## Note on answer counts

`standard` and `datafusion` answer the SPARQL query directly, so they
return exactly the count of `(?i a/rdfs:subClassOf* ?c)` solutions. They
agree on every size.

`reasonable` materialises every OWL 2 RL inference, then we count
`(?s rdf:type ?o)` triples in the closed graph. That superset includes
inferences beyond subclass closure (RDFS class membership, axiom-derived
type assertions, etc.), so reasonable's `answer_count` is slightly higher
than the SPARQL engines' on the same input. This is a known semantic
difference, not a correctness bug. The headline signal is `compute_ms`,
not the absolute count.

## Timeouts

Pass `--timeout SECONDS` (default 120) to cap each individual run. On
timeout the engine is recorded with `timed_out=true` and skipped at
subsequent larger sizes. Useful because DataFusion's `RecursiveQueryExec`
scales super-linearly on this workload and can run for tens of minutes at
`large` and beyond.

## What the bench actually exercises

The DataFusion `RecursiveQueryExec` runs the `rdfs:subClassOf*` closure and
joins with the `rdf:type` relation. The standard evaluator runs a
per-tuple worklist over the type assertions. `reasonable` runs a full
forward chainer over its native rule set.

This is the canonical reasoning workload that the DataFusion fixpoint
operator should be able to subsume. Where DataFusion sits relative to
`reasonable` here is direct signal on whether keeping oxreason out of tree
is viable. If the DataFusion line stays competitive with `reasonable` at
the `huge` size, the unified evaluator story holds; if it does not, the
DataFusion recursive operator needs work before reasoning can ride on it.
