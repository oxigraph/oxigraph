Oxigraph for Python (`pyoxigraph`)
==================================

[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community?utm_source=badge&utm_medium=badge&utm_campaign=pr-badge)

This Python package provides a Python API on top of Oxigraph named `pyoxigraph`.

Oxigraph is a graph database implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.

It offers two stores with [SPARQL 1.1 Query](https://www.w3.org/TR/sparql11-query/) capabilities.
One of the store is in-memory, and the other one is disk based.

It also provides a set of utility functions for reading, writing and processing RDF files.

The stores are also able to load and dump RDF data serialized in
[Turtle](https://www.w3.org/TR/turtle/), 
[TriG](https://www.w3.org/TR/trig/), 
[N-Triples](https://www.w3.org/TR/n-triples/),
[N-Quads](https://www.w3.org/TR/n-quads/) and
[RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/).

## Install

To install the development version of Oxigraph you need first to install the build tool [Maturin](https://github.com/PyO3/maturin).
This could be done using the usual `pip install maturin`.

`maturin build release` allows build a release Oxigraph Python wheel.
This wheel could be installed using `pip install PATH` in the current Python environment where `PATH` is the path to the built Oxigraph wheel.


## How to contribute

The Oxigraph bindings are written in Rust using [PyO3](https://github.com/PyO3/pyo3).

They are build using [Maturin](https://github.com/PyO3/maturin).
Maturin could be installed using the usual `pip install maturin`.
To install development version of Oxigraph just run `maturin develop`.

The Python bindings tests are written in Python.
To run them use the usual `python -m unittest` in the `tests` directory.
