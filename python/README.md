Oxigraph for Python
===================

[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)

This package provides a Python API on top of Oxigraph.

Oxigraph is a work in progress graph database written in Rust implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

It offers two stores with [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/) capabilities.
One of the store is in-memory, and the other one is disk based.

The store is also able to load RDF serialized in [Turtle](https://www.w3.org/TR/turtle/), [TriG](https://www.w3.org/TR/trig/), [N-Triples](https://www.w3.org/TR/n-triples/), [N-Quads](https://www.w3.org/TR/n-quads/) and [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/).

## Install

To install the development version of Oxigraph you need first to install the build tool [Maturin](https://github.com/PyO3/maturin).
This could be done using the usual `pip install maturin`.

Then you just need to run `maturin develop` to install Oxigraph in the current Python environment.


## Example

Insert the triple `<http://example/> <http://schema.org/name> "example"` and print the name of `<http://example/>` in SPARQL:
```python
from oxigraph import *

store = MemoryStore()
ex = NamedNode('http://example/')
schemaName = NamedNode('http://schema.org/name')
store.add((ex, schemaName, Literal('example')))
for binding in store.query('SELECT ?name WHERE { <http://example/> <http://schema.org/name> ?name }'):
    print(binding['name'].value)
```

## API

### Model

Oxigraph provides python classes for the basic RDF model elements.

#### `NamedNode`

An RDF [IRI](https://www.w3.org/TR/rdf11-concepts/#dfn-iri).
```python
from oxigraph import NamedNode

assert NamedNode('http://example.com/foo').value == 'http://example.com/foo'
assert str(NamedNode('http://example.com/foo')) == '<http://example.com/foo>'
```

#### `BlankNode`

An RDF [blank node](https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node).
```python
from oxigraph import BlankNode

assert BlankNode('foo').value == 'foo'
assert str(BlankNode('foo')) == 'foo'
```

#### `Literal`

An RDF [literal](https://www.w3.org/TR/rdf11-concepts/#dfn-literal).
```python
from oxigraph import NamedNode, Literal

assert Literal('foo').value == 'foo'
assert str(NamedNode('foo')) == '"foo"'

assert Literal('foo', language='en').language == 'en'
assert str(NamedNode('foo', language='en')) == '"foo"@en'

assert Literal('11', datatype=NamedNode('http://www.w3.org/2001/XMLSchema#integer')).datatype == 'http://www.w3.org/2001/XMLSchema#integer'
assert str(Literal('11', datatype=NamedNode('http://www.w3.org/2001/XMLSchema#integer'))) == '"foo"^^<http://www.w3.org/2001/XMLSchema#integer>'
```

#### `DefaultGraph`

The RDF [default graph name](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph).
```python
from oxigraph import DefaultGraph

DefaultGraph()
```

### Stores

Oxigraph provides two stores:

* `MemoryStore` that stores the RDF quads in memory
* `SledStore` that stores the graph on disk using [Sled](https://github.com/spacejam/sled).

Both stores provide a similar API. They encode an [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).

#### Constructor

##### `MemoryStore`
 
It could be constructed using:
```python
from oxigraph import MemoryStore

store = MemoryStore()
```

##### `SledStore`

The following code creates a store using the directory `foo/bar` for storage.
```python
from oxigraph import SledStore

store = SledStore('foo/bar')
```

It is also possible to use a temporary directory that will be removed when the `SledStore` Python object is dropped:
```python
from oxigraph import SledStore

store = SledStore()
```

#### `add`

To add a quad in the store:
```python
s = NamedNode('http://example.com/subject')
p = NamedNode('http://example.com/predicate')
o = NamedNode('http://example.com/object')
g = NamedNode('http://example.com/graph')
store.add((s, p, o, g))
```

If a triple is provided, it is added to the default graph i.e. `store.add((s, p, o, g))` is the same as `store.add((s, p, o, DefaultGraph()))`

#### `remove`

To remove a quad from the store:
```python
store.remove((s, p, o, g))
```

#### `__contains__`

Checks if a quad is in the store:
```python
assert (s, p, o, g) in store
```

#### `__len__`

Returns the number of quads in the store:
```python
assert len(store) == 1
```

#### `__iter__`

Iterates on all quads in the store:
```python
assert list(iter(store)) == [(s, p, o, g)]
```

#### `match`

Returns all the quads matching a given pattern using an iterator.

Return all the quads with the subject `s`:
```python
assert list(store.match(s, None, None, None)) == [(s, p, o, g)]
```

Return all the quads in the default graph:
```python
assert list(store.match(s, None, None, DefaultGraph())) == []
```

#### `query`

Executes a [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/).

The `ASK` queries return a boolean:
```python
assert store.query('ASK { ?s ?s ?s }')
```

The `SELECT` queries return an iterator of query solutions that could be indexed by variable name or position in the `SELECT` clause:
```python
solutions = list(store.query('SELECT ?s WHERE { ?s ?p ?o }'))
assert solutions[0][0] == s
assert solutions[0]['s'] == s
```

The `CONSTRUCT` and `DESCRIBE` queries return an iterator of query solutions that could be indexed by variable name or position in the `SELECT` clause:
```python
solutions = list(store.query('SELECT ?s WHERE { ?s ?p ?o }'))
assert solutions[0][0] == s
assert solutions[0]['s'] == s
```

### `load`

Loads serialized RDF triples or quad into the store.
The method arguments are:
1. `data`: the serialized RDF triples or quads.
2. `mime_type`: the MIME type of the serialization. See below for the supported mime types.
3. `base_iri`: the base IRI used to resolve the relative IRIs in the serialization.
4. `to_named_graph`: for triple serialization formats, the name of the named graph the triple should be loaded to.

The available formats are:
* [Turtle](https://www.w3.org/TR/turtle/): `text/turtle`
* [TriG](https://www.w3.org/TR/trig/): `application/trig`
* [N-Triples](https://www.w3.org/TR/n-triples/): `application/n-triples`
* [N-Quads](https://www.w3.org/TR/n-quads/): `application/n-quads`
* [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/): `application/rdf+xml`

Example of loading a Turtle file into the named graph `<http://example.com/graph>` with the base IRI `http://example.com`:
```python
store.load('<http://example.com> <http://example.com> <> .', mime_type='text/turtle', base_iri="http://example.com", to_graph=NamedNode('http://example.com/graph'))
```


## How to contribute

The Oxigraph bindings are written in Rust using [PyO3](https://github.com/PyO3/pyo3).

They are build using [Maturin](https://github.com/PyO3/maturin).
Maturin could be installed using the usual `pip install maturin`.
To install development version of Oxigraph just run `maturin develop`.

The Python bindings tests are written in Python.
To run them use the usual `python -m unittest` in the `tests` directory.
