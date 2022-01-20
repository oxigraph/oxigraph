Migration Guide
===============

From 0.2 to 0.3
"""""""""""""""

* Python 3.6 and ``manylinux2010`` (`PEP 571 <https://www.python.org/dev/peps/pep-0571/>`_) support have been removed. The new minimal versions are Python 3.7 and ``manylinux2014`` (`PEP 599 <https://www.python.org/dev/peps/pep-0599/>`_).
* The on-disk storage system has been rebuilt on top of `RocksDB <http://rocksdb.org/>`_.
  It is now implemented by the :py:class:`.Store` class that keeps the same API as the late :py:class:`.SledStore` class.

  To migrate you have to dump the store content using pyoxigraph **0.2** and the following code:

  .. code-block:: python

    from pyoxigraph import SledStore
    store = SledStore('MY_STORAGE_PATH')
    with open('temp_file.nq', 'wb') as fp:
        store.dump(fp, "application/n-quads")

  And then upgrade to pyoxigraph **0.3** and run:

  .. code-block:: python

    from pyoxigraph import Store
    store = Store('MY_NEW_STORAGE_PATH')
    with open('temp_file.nq', 'rb') as fp:
        store.bulk_load(fp, "application/n-quads")

* The in-memory storage class :py:class:`.MemoryStore` has been merged into the :py:class:`.Store` class that provides the exact same API as the late :py:class:`.MemoryStore`.
  On platforms other than Linux, a temporary directory is created when opening the :py:class:`.Store` and automatically removed when it is garbage collected. No data is written in this directory.
* :py:class:`.Store` operations are now transactional using the "repeatable read" isolation level:
  the store only exposes changes that have been "committed" (i.e. no partial writes)
  and the exposed state does not change for the complete duration of a read operation (e.g. a SPARQL query) or a read/write operation (e.g. a SPARQL update).
* `RDF-star <https://w3c.github.io/rdf-star/cg-spec>`_ is now supported (including serialization formats and SPARQL-star). :py:class:`.Triple` can now be used in :py:attr:`.Triple.object`, :py:attr:`.Triple.object`, :py:attr:`.Quad.subject` and :py:attr:`.Quad.object`.
