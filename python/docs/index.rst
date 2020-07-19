Oxigraph Python (``pyoxigraph`` |release|)
==========================================

Oxigraph is a graph database implementing the `SPARQL <https://www.w3.org/TR/sparql11-overview/>`_ standard.

It offers two stores with `SPARQL 1.1 Query <https://www.w3.org/TR/sparql11-query/>`_ capabilities.
One of the store is in-memory, and the other one is disk based.

It also provides a set of utility functions for reading, writing and processing RDF files.

The stores are also able to load and dump RDF data serialized in
`Turtle <https://www.w3.org/TR/turtle/>`_,
`TriG <https://www.w3.org/TR/trig/>`_,
`N-Triples <https://www.w3.org/TR/n-triples/>`_,
`N-Quads <https://www.w3.org/TR/n-quads/>`_ and
`RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.

Oxigraph source code is on `GitHub <https://github.com/oxigraph/oxigraph/tree/master/python>`_.


Installation
""""""""""""

Just run the usual ``pip install pyoxigraph``.


Example
"""""""

Insert the triple ``<http://example/> <http://schema.org/name> "example"`` and print the name of ``<http://example/>`` in SPARQL:

::

    from pyoxigraph import *

    store = MemoryStore()
    ex = NamedNode('http://example/')
    schemaName = NamedNode('http://schema.org/name')
    store.add((ex, schemaName, Literal('example')))
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
