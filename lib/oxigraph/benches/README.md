# Oxigraph store benchmarks

Run commands from the workspace root with the `rocksdb` feature. The two targets cover BSBM as well as **different** WatDiv workloads:

| Bench target | Workload | Inputs |
| --- | --- | --- |
| `store` | Conventional, static WatDiv | One N-Triples dataset and the static stress SPARQL query set |
| `store` | BSBM | Published datasets and query/update logs |
| `incremental-store` | Stream WatDiv | Static N-Triples data, ordered stream events, and generated SPARQL queries |
| `incremental-store` | BSBM | Published dataset and generated product-update cases |

## Static WatDiv (`store.rs`)

The default is scale `100` (10M triples). If missing, the benchmark downloads the [official WatDiv dataset and stress workload](https://dsg.uwaterloo.ca/watdiv/) into `target/watdiv/`. 
Scale `1000` (100M triples) is also downloadable with `WATDIV_STORE_SCALE=1000`.
Smaller scales, including `1` and `10`, must be supplied locally because the official site does not publish pre-generated datasets or stress queries for them.
The stress suite uses `test.1`–`test.5`; each file has 100 SELECT queries. The benchmark reports batches of 10 queries separately, so results appear progressively and a slow portion is easier to identify.
Both ordinary SPARQL evaluation and initial evaluation with the incremental engine run on the same queries. The dataset is streamed into an in-memory store once per benchmark run, outside the timed query evaluations. The incremental benchmark reads directly from that store without making a second dataset copy. Loading the default 10M-triple dataset may require substantial RAM and startup time.
No stream events or named-graph union are involved, so the incremental-engine case measures initial result computation, not update maintenance.

```sh
cargo bench -p incroxigraph --features rocksdb --bench store -- WatDiv
```

For files elsewhere, set `WATDIV_STORE_DATA` to the static `.nt` file and `WATDIV_STORE_WORKLOAD_DIR` to the directory containing `watdiv-stress-<scale>/`. 
Set `WATDIV_STORE_SCALE` to the actual dataset scale. 
These are the **standard static WatDiv files**, not files from the Stream WatDiv ZIPs. 
The `WatDiv` name filter skips the BSBM benchmark groups before they download or initialize their data. With no `WatDiv` or `BSBM` filter, all groups run as usual.

## Stream WatDiv (`incremental-store.rs`)

The benchmark downloads and caches the [published Stream WatDiv workloads](https://zenodo.org/records/23016810) automatically. 
The default is scale `10` for both static and stream data (`static-10-stream-10-seed-42-q50-m6-c1`); set `WATDIV_INCREMENTAL_SCALE=1` for the smaller `static-1-stream-1-seed-42-q10-m6-c1` workload. 
No dataset paths or generator settings are needed.

```sh
cargo bench -p incroxigraph --features rocksdb --bench incremental-store -- WatDiv

WATDIV_INCREMENTAL_SCALE=1 \
cargo bench -p incroxigraph --features rocksdb --bench incremental-store -- WatDiv
```

The default initial stream fraction is `0.90`; override it with `WATDIV_INCREMENTAL_INITIAL_STREAM_FRACTIONS`. 
Initial and final stores are bulk-loaded; later insertions or deletions are applied in bounded transaction batches. The timed incremental case consumes result deltas after those updates, while the reevaluation case runs the query on the final store. Neither timing includes loading the store or applying the updates. 
The ZIP is cached under `target/watdiv-incremental/downloads/`, extracted to `target/watdiv-incremental/<workload>/`, and `results.tsv` is written there. 
Validation still constructs stores for each case and can be slow at scale 10. In `incremental-store`, a `WatDiv` filter now skips BSBM setup.

## BSBM

The `store` target benchmarks BSBM parsing, loading, queries, and query/update workloads on in-memory and on-disk stores. The `incremental-store` target benchmarks BSBM Explore queries 1–4 across product-update scenarios, comparing incremental evaluation with recomputation, with and without update time. Both targets cache the [published BSBM inputs](https://zenodo.org/records/12663333) after the first download.

```sh
cargo bench -p incroxigraph --features rocksdb --bench store -- BSBM
cargo bench -p incroxigraph --features rocksdb --bench incremental-store -- BSBM
```

In `incremental-store`, a `BSBM` filter skips WatDiv setup (and vice versa). The BSBM incremental benchmark currently cannot complete: one Explore query uses a `FILTER` that the incremental SELECT core does not support. The `store` target still sets up other workload groups before Criterion applies its name filter.
