# Pyincroxigraph

`pyincroxigraph` provides Python bindings for Incroxigraph, an RDF and SPARQL
database based on [Oxigraph](https://github.com/oxigraph/oxigraph). The package
includes the native engine, in-memory and persistent stores, RDF types, parsers,
serializers, and incremental queries.

## Example

```python
from pyincroxigraph import DeltaKind, Literal, NamedNode, Quad, Store

store = Store()  # Use Store("database") for persistent storage.
quad = Quad(NamedNode("urn:alice"), NamedNode("urn:name"), Literal("Alice"))

with store.query_incremental_deltas("SELECT ?name WHERE { ?s <urn:name> ?name }") as query:
    with store.transaction() as tx:
        tx.add(quad)
    for change in query.deltas():
        if change.kind == DeltaKind.ADDITION:
            print(change.value["name"].value)  # Alice
```

Use `Store.query()` for an ordinary query, `query_incremental_results().results()`
for complete maintained snapshots, and `query_incremental_deltas().deltas()` for
consumed batches of additions and deletions. Live queries observe committed changes.
A transaction commits on successful context exit and rolls back on an exception.

SELECT preserves duplicate rows, CONSTRUCT uses set semantics, and ASK emits its
initial boolean and subsequent changes. Returned snapshots and batches own their
data. Use live handles on the thread that created them and close them with a
context manager or `close()`.

The current incremental evaluator supports quad patterns, joins, projection and
VALUES. Optimized plans containing unsupported operators, and incremental DESCRIBE,
raise `NotImplementedError`. Reads process pending work synchronously.

The [incremental API guide](https://github.com/maartyman/incroxigraph/blob/main/python/docs/incremental.rst)
contains result types, batching behavior and lifecycle details.

Pyincroxigraph and Pyoxigraph have separate RDF classes. When using both packages,
exchange RDF through serialization or explicitly convert their terms.

## Build from source

Python 3.9+, Rust, a C++ toolchain and Clang are required. Activate a Python virtual
environment, initialize the submodules, and install from the repository root:

```sh
git submodule update --init --recursive
python -m pip install ./python
```

## Development

To rebuild the native module while developing, run these commands from `python/`
in an activated virtual environment:

```sh
python -m pip install 'maturin~=1.0'
maturin develop --release
python -m unittest discover -s tests
```

Build a CPython stable-ABI wheel with `maturin build --release --features abi3`.
Wheels are written to `../target/wheels/`.

For the documentation and type-checking tools, use `uv sync --locked` from this
directory. Generate type stubs and build the documentation with:

```sh
uv run python generate_stubs.py pyincroxigraph pyincroxigraph.pyi --ruff
uv run sphinx-build -M html docs docs/build
```

## License

Apache-2.0 or MIT, at your option. See
[LICENSE-APACHE](https://github.com/maartyman/incroxigraph/blob/main/LICENSE-APACHE)
and [LICENSE-MIT](https://github.com/maartyman/incroxigraph/blob/main/LICENSE-MIT).
The bindings derive from Pyoxigraph; upstream copyright and license notices are retained.
