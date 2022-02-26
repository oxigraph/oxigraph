Oxigraph for JavaScript
=======================

[![npm](https://img.shields.io/npm/v/oxigraph)](https://www.npmjs.com/package/oxigraph)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

This package provides a JavaScript API on top of [Oxigraph](https://crates.io/crates/oxigraph), compiled with WebAssembly.

Oxigraph is a graph database written in Rust implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

Oxigraph for JavaScript is a work in progress and currently offers a simple in-memory store with [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/) and [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/) capabilities.

The store is also able to load RDF serialized in [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/) and [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/).

It is distributed using a [a NPM package](https://www.npmjs.com/package/oxigraph) that should work with Node.JS 12+ and modern web browsers compatible with WebAssembly.

To install:
```bash
npm install oxigraph
```

To load with Node.JS:
```js
const oxigraph = require('oxigraph');
```

or with ES modules:
```js
import oxigraph from './node_modules/oxigraph/node.js';
```

To load on an HTML web page (for [WebPack 5](https://webpack.js.org/) remove the `<script>` tag and put the code in a JS file):
```html
<script type="module">
    import init, * as oxigraph from './node_modules/oxigraph/web.js'

    (async function () {
        await init(); // Required to compile the WebAssembly code.

        // We can use here Oxigraph methods
    })()
</script>
```

## Node.JS Example

Insert the triple `<http://example/> <http://schema.org/name> "example"` and log the name of `<http://example/>` in  SPARQL:

```js
const oxigraph = require('oxigraph');
const store = new oxigraph.Store();
const ex = oxigraph.namedNode("http://example/");
const schemaName = oxigraph.namedNode("http://schema.org/name");
store.add(oxigraph.triple(ex, schemaName, oxigraph.literal("example")));
for (const binding of store.query("SELECT ?name WHERE { <http://example/> <http://schema.org/name> ?name }")) {
    console.log(binding.get("name").value);
}
```

## Web Example

Insert the triple `<http://example/> <http://schema.org/name> "example"` and log the name of `<http://example/>` in
SPARQL:

```html

<script type="module">
    import init, * as oxigraph from './node_modules/oxigraph/web.js'

    (async function () {
        await init(); // Required to compile the WebAssembly.

        const store = new oxigraph.Store();
        const ex = oxigraph.namedNode("http://example/");
        const schemaName = oxigraph.namedNode("http://schema.org/name");
        store.add(oxigraph.triple(ex, schemaName, oxigraph.literal("example")));
        for (const binding of store.query("SELECT ?name WHERE { <http://example/> <http://schema.org/name> ?name }")) {
            console.log(binding.get("name").value);
        }
    })()
</script>
```

This example works with WebPack too if you remove the `<script>` tag and put the code in a JS file.

## API

Oxigraph currently provides a simple JS API.

### RDF data model

Oxigraph implements the [RDF/JS datamodel specification](https://rdf.js.org/data-model-spec/).

For that, the `oxigraph` module implements the [RDF/JS `DataFactory` interface](http://rdf.js.org/data-model-spec/#datafactory-interface).

Example:
```js
const oxigraph = require('oxigraph');
const ex = oxigraph.namedNode("http://example.com");
const blank = oxigraph.blankNode();
const foo = oxigraph.literal("foo");
const quad = oxigraph.quad(blank, ex, foo);
```

All terms overrides the the `toString()` method to return a N-Quads/SPARQL-like representation of the terms.

### `Store`

Oxigraph API is centered around the `Store` class.

A store contains an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) and allows to query and update them using SPARQL.

#### `Store(optional sequence<Quad>? quads)` (constructor)
Creates a new store.

```js
const oxigraph = require('oxigraph');
const store = new oxigraph.Store();
```

If provided, the `Store` will be initialized with a sequence of quads.

```js
const oxigraph = require('oxigraph');
const store = new oxigraph.Store([oxigraph.quad(blank, ex, foo)]);
```

#### `Store.prototype.add(Quad quad)`
Inserts a quad in the store.

Example:
```js
store.add(quad);
```

#### `Store.prototype.delete(Quad quad)`
Removes a quad from the store.

Example:
```js
store.delete(quad);
```

#### `Store.prototype.has(Quad quad)`
Returns a boolean stating if the store contains the quad.

Example:
```js
store.has(quad);
```

#### `Store.prototype.match(optional Term? subject, optional Term? predicate, optional Term? object, optional Term? graph)`
Returns an array with all the quads matching a given quad pattern.

Example to get all quads in the default graph with `ex` for subject:
```js
store.match(ex, null, null, oxigraph.defaultGraph());
```

Example to get all quads:
```js
store.match();
```

#### `Store.prototype.query(String query)`
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
const filteredStore = new oxigraph.Store(store.query("CONSTRUCT { <http:/example.com/> ?p ?o } WHERE { <http:/example.com/> ?p ?o }"));
```

Example of ASK query:
```js
if (store.query("ASK { ?s ?s ?s }")) {
    console.log("there is a triple with same subject, predicate and object");
}
```

#### `Store.prototype.update(String query)`
Executes a [SPARQL 1.1 Update](https://www.w3.org/TR/sparql11-update/).
The [`LOAD` operation](https://www.w3.org/TR/sparql11-update/#load) is not supported yet.

Example of update:
```js
store.update("DELETE WHERE { <http://example.com/s> ?p ?o }")
```

#### `Store.prototype.load(String data, String mimeType, NamedNode|String? baseIRI, NamedNode|BlankNode|DefaultGraph? toNamedGraph)`

Loads serialized RDF triples or quad into the store.
The method arguments are:
1. `data`: the serialized RDF triples or quads.
2. `mimeType`: the MIME type of the serialization. See below for the supported mime types.
3. `baseIRI`: the base IRI to use to resolve the relative IRIs in the serialization.
4. `toNamedGraph`: for triple serialization formats, the name of the named graph the triple should be loaded to.

The available formats are:
* [Turtle](https://www.w3.org/TR/turtle/): `text/turtle`
* [TriG](https://www.w3.org/TR/trig/): `application/trig`
* [N-Triples](https://www.w3.org/TR/n-triples/): `application/n-triples`
* [N-Quads](https://www.w3.org/TR/n-quads/): `application/n-quads`
* [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/): `application/rdf+xml`

Example of loading a Turtle file into the named graph `<http://example.com/graph>` with the base IRI `http://example.com`:
```js
store.load("<http://example.com> <http://example.com> <> .", "text/turtle", "http://example.com", oxigraph.namedNode("http://example.com/graph"));
```

#### `Store.prototype.dump(String mimeType, NamedNode|BlankNode|DefaultGraph? fromNamedGraph)`

Returns serialized RDF triples or quad from the store.
The method arguments are:
1. `mimeType`: the MIME type of the serialization. See below for the supported mime types.
2. `fromNamedGraph`: for triple serialization formats, the name of the named graph the triple should be loaded from.

The available formats are:
* [Turtle](https://www.w3.org/TR/turtle/): `text/turtle`
* [TriG](https://www.w3.org/TR/trig/): `application/trig`
* [N-Triples](https://www.w3.org/TR/n-triples/): `application/n-triples`
* [N-Quads](https://www.w3.org/TR/n-quads/): `application/n-quads`
* [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/): `application/rdf+xml`

Example of building a Turtle file from the named graph `<http://example.com/graph>`:
```js
store.dump("text/turtle", oxigraph.namedNode("http://example.com/graph"));
```

## Migration guide

### From 0.2 to 0.3
* The `MemoryStore` class is now called `Store` (there is no other kind of stores...).
* RDF/JS datamodel functions (`namedNode`...) are now available at the root of the `oxigraph` package. You now need to call `oxigraph.namedNode` instead of `store.dataFactory.namedNode`.
* [RDF-star](https://w3c.github.io/rdf-star/cg-spec) is now implemented. `Quad` is now a valid value for the `Ωuad` `subject` and `object` properties.

## How to contribute

The Oxigraph bindings are written in Rust using [the Rust WASM toolkit](https://rustwasm.github.io/docs.html).

The [The Rust Wasm Book](https://rustwasm.github.io/docs/book/) is a great tutorial to get started.

To run the tests of the JS bindings written in JS run `npm test`.


## License

This project is licensed under either of

* Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
* MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.


### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
