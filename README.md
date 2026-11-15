# Incroxigraph

Incroxigraph is an RDF and SPARQL database based on
[Oxigraph](https://github.com/oxigraph/oxigraph). It adds incremental query evaluation:
a live query maintains its answer as committed changes reach the store.

You can read either the complete current result or additions and deletions since
the previous read. Incremental queries support SELECT, ASK and CONSTRUCT. Ordinary
SPARQL queries and updates, RDF parsing and serialization, and persistent storage
are available through the underlying engine.

## Interfaces

| Interface | Directory | Description |
| --- | --- | --- |
| Rust | [lib/oxigraph](lib/oxigraph/README.md) | Database engine, published as `incroxigraph` |
| Python | [python](python/README.md) | `pyincroxigraph`, with native RDF types, persistent stores and live queries |
| JavaScript | [js](js/README.md) | `incroxigraph-js`, with an in-memory WebAssembly store and RDF/JS types |
| Command line | [cli](cli/README.md) | RDF tools and a SPARQL HTTP server; the executable is named `oxigraph` |

The Python and JavaScript packages each include their own engine. Their READMEs
contain source installation instructions and examples.

## Incremental queries

- **Snapshots** return the complete answer after processing pending changes.
- **Deltas** return additions and deletions. SELECT preserves duplicate rows;
  CONSTRUCT uses set semantics; ASK reports its initial boolean and later changes.
- **Transactions** group writes into an atomic commit observed by live queries.

Incremental evaluation is under development. The current evaluator supports quad
patterns, joins, projection and VALUES. Optimized plans containing unsupported
operators return errors; incremental DESCRIBE is unsupported. Ordinary queries
use the broader SPARQL implementation.

See the [Rust examples](lib/oxigraph/README.md#incremental-queries),
[Python guide](python/docs/incremental.rst), or
[JavaScript API overview](js/README.md#api-overview) for the interfaces and their limits.

## Build from source

Clone this fork with its submodules:

```sh
git clone --recurse-submodules https://github.com/maartyman/incroxigraph.git
cd incroxigraph
```

For an existing checkout, run `git submodule update --init --recursive`.
Native builds with RocksDB require Rust, a C++ toolchain and Clang.
The binding READMEs list their additional Python or JavaScript requirements.

The [Rust workspace overview](lib/README.md) describes the supporting crates.
Store benchmarks and their inputs are documented in
[lib/oxigraph/benches](lib/oxigraph/benches/README.md).

## Development

Run the file-based test suite from the repository root:

```sh
cargo test -p oxigraph-testsuite --features spareval/sep-0006
```

The feature enables LATERAL support required by the suite's corresponding fixtures.
Binding-specific test commands are in the Python and JavaScript READMEs.

Report problems with this fork in
[GitHub issues](https://github.com/maartyman/incroxigraph/issues).

## License

Incroxigraph is available under [Apache-2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT),
at your option. It derives from Oxigraph and retains upstream copyright and
license notices. Contributions are dual licensed under the same terms unless
explicitly stated otherwise.
