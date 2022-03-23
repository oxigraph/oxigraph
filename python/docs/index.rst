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

It also provides a set of utility functions for reading, writing, and processing RDF files in
`Turtle <https://www.w3.org/TR/turtle/>`_,
`TriG <https://www.w3.org/TR/trig/>`_,
`N-Triples <https://www.w3.org/TR/n-triples/>`_,
`N-Quads <https://www.w3.org/TR/n-quads/>`_ and
`RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.

Pyoxigraph is `distributed on Pypi <https://pypi.org/project/pyoxigraph/>`_.

There is also a small library providing a `rdflib <https://rdflib.readthedocs.io>`_ store using pyoxigraph: `oxrdflib <https://github.com/oxigraph/oxrdflib>`_.

Oxigraph and pyoxigraph source code are on `GitHub <https://github.com/oxigraph/oxigraph/tree/main/python>`_.


Installation
""""""""""""

Pyoxigraph is distributed on `Pypi <https://pypi.org/project/pyoxigraph/>`_.

To install it, run the usual ``pip install pyoxigraph``


Example
"""""""

Insert the triple ``<http://example/> <http://schema.org/name> "example"`` and print the name of ``<http://example/>`` in SPARQL:

::

    from pyoxigraph import *

    store = Store()
    ex = NamedNode('http://example/')
    schema_name = NamedNode('http://schema.org/name')
    store.add(Quad(ex, schema_name, Literal('example')))
    for binding in store.query('SELECT ?name WHERE { <http://example/> <http://schema.org/name> ?name }'):
        print(binding['name'].value)


Table of contents
"""""""""""""""""

.. toctree::

   model
   io
   store
   sparql
   migration


Help
""""

Feel free to use `GitHub discussions <https://github.com/oxigraph/oxigraph/discussions>`_ or `the Gitter chat <https://gitter.im/oxigraph/community>`_ to ask questions or talk about Oxigraph.
`Bug reports <https://github.com/oxigraph/oxigraph/issues>`_ are also very welcome.

If you need advanced support or are willing to pay to get some extra features, feel free to reach out to `Tpt <https://github.com/Tpt>`_.
