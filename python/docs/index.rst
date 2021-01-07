pyoxigraph |release|
====================

.. image:: https://img.shields.io/pypi/v/pyoxigraph
    :alt: PyPI
    :target: https://pypi.org/project/pyoxigraph/
.. image:: https://img.shields.io/pypi/implementation/pyoxigraph
    :alt: PyPI - Implementation
.. image:: https://img.shields.io/pypi/pyversions/pyoxigraph
    :alt: PyPI - Python Version
.. image:: https://img.shields.io/pypi/l/pyoxigraph
    :alt: PyPI - License


Pyoxigraph is a Python graph database library implementing the `SPARQL <https://www.w3.org/TR/sparql11-overview/>`_ standard.

It is built on top of `Oxigraph <https://crates.io/crates/oxigraph>`_ using `PyO3 <https://pyo3.rs/>`_.

It offers two stores with `SPARQL 1.1 <https://www.w3.org/TR/sparql11-overview/>`_ capabilities.
One of the store is in-memory, and the other one is disk based.

It also provides a set of utility functions for reading, writing and processing RDF files in
`Turtle <https://www.w3.org/TR/turtle/>`_,
`TriG <https://www.w3.org/TR/trig/>`_,
`N-Triples <https://www.w3.org/TR/n-triples/>`_,
`N-Quads <https://www.w3.org/TR/n-quads/>`_ and
`RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.

Pyoxigraph is `distributed on Pypi <https://pypi.org/project/pyoxigraph/>`_.

There exists also a small library providing `rdflib <https://rdflib.readthedocs.io>`_ stores using pyoxigraph: `oxrdflib <https://github.com/oxigraph/oxrdflib>`_.

Oxigraph and pyoxigraph source code are on `GitHub <https://github.com/oxigraph/oxigraph/tree/master/python>`_.


Installation
""""""""""""

Pyoxigraph is distributed on `Pypi <https://pypi.org/project/pyoxigraph/>`_.

To install it, run the usual ``pip install pyoxigraph``


Example
"""""""

Insert the triple ``<http://example/> <http://schema.org/name> "example"`` and print the name of ``<http://example/>`` in SPARQL:

::

    from pyoxigraph import *

    store = MemoryStore()
    ex = NamedNode('http://example/')
    schema_name = NamedNode('http://schema.org/name')
    store.add(Quad(ex, schema_name, Literal('example')))
    for binding in store.query('SELECT ?name WHERE { <http://example/> <http://schema.org/name> ?name }'):
        print(binding['name'].value)


Table of contents
"""""""""""""""""

.. toctree::
   :maxdepth: 2

   model
   io
   store/memory
   store/sled
   sparql
