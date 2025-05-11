# Pyoxigraph (Oxigraph for Python)

[![PyPI](https://img.shields.io/pypi/v/pyoxigraph)](https://pypi.org/project/pyoxigraph/)
[![Conda](https://img.shields.io/conda/vn/conda-forge/pyoxigraph)](https://anaconda.org/conda-forge/pyoxigraph)
![PyPI - Implementation](https://img.shields.io/pypi/implementation/pyoxigraph)
![PyPI - Python Version](https://img.shields.io/pypi/pyversions/pyoxigraph)
[![actions status](https://github.com/oxigraph/oxigraph/workflows/build/badge.svg)](https://github.com/oxigraph/oxigraph/actions)
[![Gitter](https://badges.gitter.im/oxigraph/community.svg)](https://gitter.im/oxigraph/community)

Pyoxigraph is a graph database library implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.
It is a Python library written on top of [Oxigraph](https://crates.io/crates/oxigraph).

Pyoxigraph offers two stores with [SPARQL 1.1](https://www.w3.org/TR/sparql11-overview/) capabilities.
One of the store is in-memory, and the other one is disk based.

It also provides a set of utility functions for reading, writing and processing RDF files in
[JSON-LD 1.0](https://www.w3.org/TR/json-ld/),
[Turtle](https://www.w3.org/TR/turtle/),
[TriG](https://www.w3.org/TR/trig/),
[N-Triples](https://www.w3.org/TR/n-triples/),
[N-Quads](https://www.w3.org/TR/n-quads/) and
[RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/).

Pyoxigraph is distributed [on Pypi](https://pypi.org/project/pyoxigraph/) and [on conda-forge](https://anaconda.org/conda-forge/pyoxigraph).
Run `pip install pyoxigraph` to install it.

There exists also a small library providing [rdflib](https://rdflib.readthedocs.io) stores using pyoxigraph: [oxrdflib](https://github.com/oxigraph/oxrdflib).

Pyoxigraph documentation is [available on the Oxigraph website](https://pyoxigraph.readthedocs.io/).

## Build the development version

To build and install the development version of pyoxigraph you need to clone this git repository including submodules (`git clone --recursive https://github.com/oxigraph/oxigraph.git`)
and to run `pip install .` in the `python` directory (the one this README is in).

Note that by default the installation will not use [cpython stable ABI](https://docs.python.org/3/c-api/stable.html).
Use `--features abi3` feature to use cpython stable ABI.

## Help

Feel free to use [GitHub discussions](https://github.com/oxigraph/oxigraph/discussions) or [the Gitter chat](https://gitter.im/oxigraph/community) to ask questions or talk about Oxigraph.
[Bug reports](https://github.com/oxigraph/oxigraph/issues) are also very welcome.

If you need advanced support or are willing to pay to get some extra features, feel free to reach out to [Tpt](https://github.com/Tpt).

## How to contribute

Pyoxigraph is written in Rust using [PyO3](https://github.com/PyO3/pyo3).

Pyoxigraph is built using [Maturin](https://github.com/PyO3/maturin).
Maturin could be installed using the `pip install 'maturin>=0.9,<0.10'`.
To install a development version of Oxigraph just run `maturin develop` in this README directory.

### Tests

The Python bindings tests are written in Python.
To run them use `python -m unittest` in the `tests` directory.

### Docs

The Sphinx documentation can be generated and viewed in the browser using the following command:

```
sphinx-autobuild docs docs/_build/html
```

Note that you will need to have [sphinx-autobuild](https://pypi.org/project/sphinx-autobuild/) installed.

Alternatively, you can use `sphinx-build` with Python's `http.server` to achieve the same thing.

## License

This project is licensed under either of

- Apache License, Version 2.0, ([LICENSE-APACHE](../LICENSE-APACHE) or
  http://www.apache.org/licenses/LICENSE-2.0)
- MIT license ([LICENSE-MIT](../LICENSE-MIT) or
  http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Oxigraph by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
