from typing import List
from .pyoxigraph import Variable as PyVariable
from .pyoxigraph import QuerySolutions as PyQuerySolutions
from .pyoxigraph import QuerySolution as PyQuerySolution
from .pyoxigraph import QueryTriples as PyQueryTriples


class Variable(PyVariable):
    """A SPARQL query variable.

    :param value: the variable name as a string.
    :raises ValueError: if the variable name is invalid according to the SPARQL grammar.

    The :py:func:`str` function provides a serialization compatible with SPARQL:

    >>> str(Variable('foo'))
    '?foo'
    """

    def __init__(self, value: str) -> None:
        ...

    @property
    def value(self) -> str:
        """the variable name.

        >>> Variable("foo").value
        'foo'
        """
        return super().value


class QuerySolutions(PyQuerySolutions):
    """An iterator of :py:class:`QuerySolution` returned by a SPARQL ``SELECT`` query

    >>> store = Store()
    >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    >>> list(store.query('SELECT ?s WHERE { ?s ?p ?o }'))
    [<QuerySolution s=<NamedNode value=http://example.com>>]
    """

    @property
    def variables(self) -> List[Variable]:
        """the ordered list of all variables that could appear in the query results

        >>> store = Store()
        >>> store.query('SELECT ?s WHERE { ?s ?p ?o }').variables
        [<Variable value=s>]
        """
        return super().variables


class QuerySolution(PyQuerySolution):
    """Tuple associating variables and terms that are the result of a SPARQL ``SELECT`` query.

    It is the equivalent of a row in SQL.

    It could be indexes by variable name (:py:class:`Variable` or :py:class:`str`) or position in the tuple (:py:class:`int`).
    Unpacking also works.

    >>> store = Store()
    >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    >>> solution = next(store.query('SELECT ?s ?p ?o WHERE { ?s ?p ?o }'))
    >>> solution[Variable('s')]
    <NamedNode value=http://example.com>
    >>> solution['s']
    <NamedNode value=http://example.com>
    >>> solution[0]
    <NamedNode value=http://example.com>
    >>> s, p, o = solution
    >>> s
    <NamedNode value=http://example.com>
    """


class QueryTriples(PyQueryTriples):
    """An iterator of :py:class:`Triple` returned by a SPARQL ``CONSTRUCT`` or ``DESCRIBE`` query

    >>> store = Store()
    >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    >>> list(store.query('CONSTRUCT WHERE { ?s ?p ?o }'))
    [<Triple subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>]
    """
