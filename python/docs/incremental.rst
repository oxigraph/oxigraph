Incremental queries
===================
.. py:currentmodule:: pyincroxigraph

An incremental query keeps evaluation state as its store changes. Ordinary
:py:meth:`Store.query` evaluates a query once; the incremental methods return a live handle
that you read repeatedly. They accept the same query options as the ordinary method.

Complete snapshots
------------------

Use :py:meth:`Store.query_incremental_results` when you need the complete current answer:

.. doctest::

    >>> store = Store()
    >>> quad = Quad(NamedNode("urn:alice"), NamedNode("urn:name"), Literal("Alice"))
    >>> with store.query_incremental_results("SELECT ?name WHERE { ?s <urn:name> ?name }") as query:
    ...     assert list(query.results()) == []
    ...     store.add(quad)
    ...     print([row["name"].value for row in query.results()])
    ['Alice']

SELECT returns :py:class:`QuerySolutions`, ASK returns :py:class:`QueryBoolean`, and
CONSTRUCT returns :py:class:`QueryTriples`. Each read processes currently pending changes
and captures an owned snapshot. You can retain a snapshot while requesting another, or
consume it after closing the handle. Capturing it copies its rows or triples, with time and
memory proportional to the complete answer. Returned row and triple iterators are single-pass.

Result deltas
-------------

Use :py:meth:`Store.query_incremental_deltas` to maintain your own result collection:

.. doctest::

    >>> store = Store()
    >>> quad = Quad(NamedNode("urn:alice"), NamedNode("urn:name"), Literal("Alice"))
    >>> with store.query_incremental_deltas("SELECT ?name WHERE { ?s <urn:name> ?name }") as query:
    ...     assert list(query.deltas()) == []
    ...     store.add(quad)
    ...     addition = next(query.deltas())
    ...     assert addition.kind == DeltaKind.ADDITION
    ...     print(addition.value["name"].value)
    ...     store.remove(quad)
    ...     assert next(query.deltas()).kind == DeltaKind.DELETION
    Alice

SELECT yields :py:class:`QuerySolutionDeltas`, an iterator of :py:class:`Delta` values whose
``value`` is a :py:class:`QuerySolution`. Duplicate rows are preserved: one addition or
deletion changes a row's multiplicity by one. Both the handle and each SELECT batch expose
``variables`` in projection order.

CONSTRUCT yields :py:class:`QueryTripleDeltas`, with :py:class:`Triple` values in its deltas.
The result is a set: a triple is added when its first supporting solution appears and
removed when its last supporting solution disappears. Template blank nodes retain their
identity when their triples are later deleted.

ASK yields :py:class:`QueryBooleanDeltas`: its initial boolean, including false, and then
a new :py:class:`QueryBoolean` only when the answer changes. Convert each with ``bool(value)``.
A batch contains zero or one boolean values; false is a result, not an empty batch.

Each call consumes ready changes and returns an owned, single-pass batch, including an empty
one if nothing is ready. Retaining an earlier batch does not prevent another read or close.
Discarding an unread batch discards those changes for that consumer. Separate handles have
independent consumption positions.

Initial SELECT rows and CONSTRUCT triples are additions. The first read may also include
writes committed before that read. Several commits can be combined into a single read,
without reporting every intermediate result. Batches have no transaction identifiers or
per-commit boundaries. Row, triple, and delta ordering is unspecified.

Atomic write batches
--------------------

Use :py:meth:`Store.transaction` to group additions and removals:

.. doctest::

    >>> store = Store()
    >>> quad = Quad(NamedNode("urn:alice"), NamedNode("urn:name"), Literal("Alice"))
    >>> with store.transaction() as tx:
    ...     tx.add(quad)
    ...     assert len(store) == 0
    >>> len(store)
    1

The transaction buffers writes in memory and applies them in order in one atomic commit.
Successful context exit commits; an exception rolls back. Explicit ``commit()`` and
``rollback()`` are also available. A transaction is write-only, provides no read snapshot,
and holds no database transaction lock while Python is collecting operations. Dropping it
before commit discards the writes. Large imports should use the existing loading methods.

Only committed changes reach live query handles. A transaction's intermediate writes are
invisible, and adding then removing an initially absent quad in one batch produces no change.

Lifecycle and current limits
----------------------------

Use handles on the thread that created them. Reads process pending work synchronously and
release Python's interpreter lock while evaluating it. Use a context manager or idempotent
``close()`` to release subscriptions promptly. Reads after close raise ``RuntimeError``.
Evaluation failures close a handle: create a new one to restart from the current store.
Unconsumed writes can accumulate in subscription queues while a handle remains open.

Subscriptions are in-process state associated with the store instance and its shared Rust
handles. They are not a persistent change log, do not resume after reopening the database,
and do not watch an independently opened database in another process.

The current incremental evaluator implements quad patterns, joins, projection, and VALUES.
Optimized plans containing unsupported operators, including FILTER, OPTIONAL, UNION,
MINUS, BIND, DISTINCT, aggregates, ORDER BY, LIMIT/OFFSET, property paths, or SERVICE,
raise ``NotImplementedError``. Incremental DESCRIBE is unsupported. Optimization can remove
some operators before evaluation. Ordinary queries retain the broader SPARQL implementation.

This release exposes synchronous polling. Asynchronous snapshot and delta streams are
reserved for a later release.

API reference
-------------

.. autoclass:: IncrementalQueryResults
    :members:

.. autoclass:: IncrementalQueryDeltas
    :members:

.. autoclass:: Delta
    :members:

.. autoclass:: DeltaKind
    :members:

.. autoclass:: QuerySolutionDeltas
    :members:

.. autoclass:: QueryBooleanDeltas
    :members:

.. autoclass:: QueryTripleDeltas
    :members:

.. autoclass:: Transaction
    :members:
