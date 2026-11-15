# Incroxigraph for JavaScript

`incroxigraph-js` provides JavaScript and TypeScript bindings for Incroxigraph,
an RDF and SPARQL database based on [Oxigraph](https://github.com/oxigraph/oxigraph).
The package includes an in-memory WebAssembly engine, RDF/JS types, ordinary
SPARQL queries and updates, and incremental queries.

## Build and install locally

Use Node.js 24 and Rust with the WebAssembly target. From `js/`, run:

```sh
rustup target add wasm32-unknown-unknown
npm ci
npm run build
```

The generated package is in `js/pkg/`. From your application directory, install
it and the RDF/JS factory used in the example:

```sh
npm install /path/to/incroxigraph/js/pkg @rdfjs/data-model
```

Use a WebAssembly-capable bundler for browser applications. With Node.js 24,
the example can be run as `node --experimental-wasm-modules example.mjs`.

## Example

```js
import { DeltaKind, Store } from "incroxigraph-js";
import factory from "@rdfjs/data-model";

const store = new Store();
const query = store.queryIncrementalDeltas(
    "SELECT ?name WHERE { ?s <urn:name> ?name }",
);
try {
    store.add(factory.quad(
        factory.namedNode("urn:alice"),
        factory.namedNode("urn:name"),
        factory.literal("Alice"),
    ));
    for (const change of query.deltas()) {
        if (change.kind === DeltaKind.Addition) {
            console.log(change.value.get("name").value); // Alice
        }
    }
} finally {
    query.close();
    query.free();
    store.free();
}
```

## API overview

| Method | Purpose |
| --- | --- |
| `store.query(text)` | Evaluate an ordinary SPARQL query once |
| `store.queryIncrementalResults(text)` | Create a live handle whose `results()` returns complete snapshots |
| `store.queryIncrementalDeltas(text)` | Create a live handle whose `deltas()` consumes additions and deletions |
| `store.extend(quads)` | Insert an iterable of quads atomically |
| `store.transaction()` | Buffer additions and deletions until an explicit `commit()` |

The ordinary store API also includes `add`, `delete`, `has`, `size`, `match`,
`update`, `load` and `dump`. Transactions expose `add`, `delete`, `extend`,
`commit` and `rollback`; `free()` discards uncommitted writes.

By default, SELECT snapshots are arrays of Maps from variable names to RDF/JS
terms. CONSTRUCT snapshots are arrays of default-graph quads, and ASK snapshots
are booleans. SELECT and CONSTRUCT deltas have `{ kind, value }` records with `DeltaKind.Addition` or
`DeltaKind.Deletion`. ASK batches contain the initial boolean and later changes.
SELECT preserves duplicate rows; CONSTRUCT uses set semantics.

Reads process pending committed changes synchronously. Returned arrays, Maps and
RDF/JS terms own their data. Close live queries with `close()` and release
WebAssembly wrappers with `free()` when finished.

The current incremental evaluator supports quad patterns, joins, projection and
VALUES. Optimized plans containing unsupported operators, and incremental DESCRIBE,
throw an error named `NotImplementedError`.

## RDF interoperability

Store inputs follow RDF/JS and accept valid quads from compatible factories.
Use a consistent blank-node mapping when merging independent datasets.
The parsers and serializers support N-Triples, N-Quads, Turtle, TriG, N3, RDF/XML
and JSON-LD.

Query and I/O options retain the original snake_case names, such as `base_iri`,
`to_graph_name`, `from_graph_name` and `lenient`. The `unchecked` option is a
deprecated alias for `lenient`. `parse` accepts buffers and synchronous or
asynchronous chunk iterables; `Store.load` accepts buffers and synchronous iterables.

## Development

```sh
npm test       # Build a debug package and run the tests.
npm run check  # Check TypeScript and formatting after building.
```

Format the Rust bindings with `cargo fmt -p incroxigraph-js`.
The build uses a wasm-bindgen executable matching `Cargo.lock`, downloading it if
needed. Set `WASM_BINDGEN` to use a local executable. For offline builds with
cached dependencies, run `python3 build_package.py --offline`.

## License

Apache-2.0 or MIT, at your option. See
[LICENSE-APACHE](https://github.com/maartyman/incroxigraph/blob/main/LICENSE-APACHE)
and [LICENSE-MIT](https://github.com/maartyman/incroxigraph/blob/main/LICENSE-MIT).
The bindings derive from Oxigraph; upstream copyright and license notices are retained.
