Oxigraph for JavaScript
=======================

[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![npm](https://img.shields.io/npm/v/oxigraph)](https://www.npmjs.com/package/oxigraph)

This package provides a JavaScript API on top of Oxigraph compiled with WebAssembly.

Oxigraph is a work in progress graph database written in Rust implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

It is a work in progress and currently offers a simple in-memory store with [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/) capabilities.

It is distributed using a [a NPM package](https://www.npmjs.com/package/oxigraph) that should work with nodeJS.

```bash
npm install oxigraph
```

```js
const oxigraph = require('oxigraph');
```

## API

Oxigraph currently provides a simple JS API.
It is centered around the `MemoryStore` class.

The `NamedNode`, `BlankNode`, `Literal`, `DefaultGraph`, `Quad` and `DataFactory` types
are following the [RDF/JS datamodel specification](https://rdf.js.org/data-model-spec/).

To import `MemoryStore` using Node:
```js
const { MemoryStore } = require('oxigraph');
```

### `MemoryStore`

#### `MemoryStore(optional sequence<Quad>? quads)` (constructor)
```js
const store = new MemoryStore();
```

If provided, the `MemoryStore` will be initialized with a sequence of quads.

#### `MemoryStore.dataFactory`
Returns a `DataFactory` following [RDF/JS datamodel specification](https://rdf.js.org/data-model-spec/).

Example:
```js
const store = new MemoryStore();
const ex = store.dataFactory.namedNode("http://example.com");
const blank = store.dataFactory.blankNode();
const foo = store.dataFactory.literal("foo");
const quad = store.dataFactory.quad(blank, ex, foo);
```

#### `MemoryStore.prototype.add(Quad quad)`
Inserts a quad in the store.

Example:
```js
store.add(quad);
```

#### `MemoryStore.prototype.delete(Quad quad)`
Removes a quad from the store.

Example:
```js
store.delete(quad);
```

#### `MemoryStore.prototype.has(Quad quad)`
Returns a boolean stating if the store contains the quad.

Example:
```js
store.has(quad);
```

#### `MemoryStore.prototype.match(optional Term? subject, optional Term? predicate, optional Term? object, optional Term? graph)`
Returns an array with all the quads matching a given quad pattern.

Example to get all quads in the default graph with `ex` for subject:
```js
store.match(ex, null, null, store.dataFactory.defaultGraph());
```

Example to get all quads:
```js
store.match();
```

#### `MemoryStore.prototype.query(String query)`
Executes a [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/).
For `SELECT` queries the return type is an array of `Map` which keys are the bound variables and values are the values the result is bound to.
For `CONSTRUCT` and `ÐESCRIBE` queries the return type is an array of `Quad`.
For `ASK` queries the return type is a boolean.

Example of SELECT query:
```js
for (binding of store.query("SELECT DISTINCT ?s WHERE { ?s ?p ?o }")) {
    console.log(binding.get("s").value);
}
```

Example of CONSTRUCT query:
```js
const filteredStore = new MemoryStore(store.query("CONSTRUCT { <http:/example.com/> ?p ?o } WHERE { <http:/example.com/> ?p ?o }"));
```

Example of ASK query:
```js
if (store.query("ASK { ?s ?s ?s }")) {
    console.log("there is a triple with same subject, predicate and object");
}
```


## Example

Insert the triple `<http://example/> <http://schema.org/name> "example"` and log the name of `<http://example/>` in SPARQL:
```js
const { MemoryStore } = require('oxigraph');
const store = new MemoryStore();
const dataFactory = store.dataFactory;
const ex = dataFactory.namedNode("http://example/");
const schemaName = dataFactory.namedNode("http://schema.org/name");
store.add(dataFactory.triple(ex, schemaName, dataFactory.literal("example")));
for (binding of store.query("SELECT ?name WHERE { <http://example/> <http://schema.org/name> ?name }")) {
    console.log(binding.get("name").value);
}
```


## How to contribute

The Oxigraph bindings are written in Rust using [the Rust WASM toolkit](https://rustwasm.github.io/docs.html).

The [The Rust Wasm Book](https://rustwasm.github.io/docs/book/) is a great tutorial to get started.

To build the JavaScript bindings, just run `wasm-pack build`, to run the tests of the JS bindings written in JS just do a usual `npm test`.
