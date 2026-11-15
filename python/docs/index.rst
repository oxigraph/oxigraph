Pyincroxigraph |release|
========================

Pyincroxigraph is a self-contained Python RDF and SPARQL database with incremental queries.
It is based on `Oxigraph and Pyoxigraph <https://github.com/oxigraph/oxigraph>`_, using
`PyO3 <https://pyo3.rs/>`_. It includes its own engine and RDF classes and does not depend
on Pyoxigraph.

Installation
------------

Build this checkout with ``pip install ./python`` from the repository root.
See the `Python README <https://github.com/maartyman/incroxigraph/tree/main/python>`_
for build requirements and development instructions.

Example
-------

.. doctest::

    >>> from pyincroxigraph import Store, NamedNode, Quad, Literal
    >>> store = Store()
    >>> store.add(Quad(NamedNode("urn:example"), NamedNode("urn:name"), Literal("example")))
    >>> for row in store.query("SELECT ?name WHERE { ?s <urn:name> ?name }"):
    ...     print(row["name"].value)
    example

The :doc:`incremental` guide describes live snapshots, result deltas, and transaction batching.

Table of contents
-----------------

.. toctree::

   model
   io
   store
   sparql
   incremental
