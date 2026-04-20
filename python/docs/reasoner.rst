OWL 2 RL Reasoner
=================
.. py:currentmodule:: pyoxigraph

Pyoxigraph exposes the experimental ``oxreason`` engine for forward chaining
OWL 2 RL (and RDFS subset) materialisation. The reasoner operates on the
default graph of a :py:class:`Dataset` and writes inferred triples back into
that same default graph.

The current implementation supports the core property and class rules, the
schema rules, and the ``cax-dw`` disjointness check. Equality and functional
property rules are available behind an opt in flag. SHACL validation is
scaffolded in Rust but not yet bound to Python.

.. autoclass:: Reasoner
    :members:

.. autoclass:: ReasoningReport
    :members:
