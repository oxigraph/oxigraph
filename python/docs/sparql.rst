SPARQL utility objects
======================
.. py:currentmodule:: pyoxigraph

Oxigraph provides also some utilities related to SPARQL queries:

Variable
""""""""
.. autoclass:: Variable
    :members:

``SELECT`` solutions
""""""""""""""""""""
.. autoclass:: QuerySolutions
    :members:
.. autoclass:: QuerySolution
    :members:

``ASK`` results
"""""""""""""""
.. autoclass:: QueryBoolean
    :members:

``CONSTRUCT`` results
"""""""""""""""""""""
.. autoclass:: QueryTriples
    :members:

Query results parsing
"""""""""""""""""""""
.. autofunction:: parse_query_results
.. autoclass:: QueryResultsFormat
    :members:
