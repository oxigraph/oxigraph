from typing import Union

from .pyoxigraph import NamedNode as PyNamedNode
from .pyoxigraph import BlankNode as PyBlankNode
from .pyoxigraph import Literal as PyLiteral
from .pyoxigraph import Triple as PyTriple
from .pyoxigraph import Quad as PyQuad
from .pyoxigraph import DefaultGraph as PyDefaultGraph


class NamedNode(PyNamedNode):
    """An RDF `node identified by an IRI <https://www.w3.org/TR/rdf11-concepts/#dfn-iri>`_.

    :param value: the IRI as a string.
    :raises ValueError: if the IRI is not valid according to `RFC 3987 <https://tools.ietf.org/rfc/rfc3987>`_.

    The :py:func:`str` function provides a serialization compatible with N-Triples, Turtle, and SPARQL:

    >>> str(NamedNode('http://example.com'))
    '<http://example.com>'
    """

    def __init__(self, value: str) -> None:
        ...

    @property
    def value(self) -> str:
        """the named node IRI.

        >>> NamedNode("http://example.com").value
        'http://example.com'
        """
        return super().value


class BlankNode(PyBlankNode):
    """An RDF `blank node <https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node>`_.

    :param value: the `blank node ID <https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node-identifier>`_ (if not present, a random blank node ID is automatically generated).
    :raises ValueError: if the blank node ID is invalid according to NTriples, Turtle, and SPARQL grammars.

    The :py:func:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:

    >>> str(BlankNode('ex'))
    '_:ex'
    """

    def __init__(self, value: Union[str, None] = None) -> None:
        ...

    @property
    def value(self) -> str:
        """the `blank node ID <https://www.w3.org/TR/rdf11-concepts/#dfn-blank-node-identifier>`_.

        >>> BlankNode("ex").value
        'ex'"""
        return super().value


class Literal(PyLiteral):
    """An RDF `literal <https://www.w3.org/TR/rdf11-concepts/#dfn-literal>`_.

    :param value: the literal value or `lexical form <https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form>`_.
    :param datatype: the literal `datatype IRI <https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri>`_.
    :param language: the literal `language tag <https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag>`_.
    :raises ValueError: if the language tag is not valid according to `RFC 5646 <https://tools.ietf.org/rfc/rfc5646>`_ (`BCP 47 <https://tools.ietf.org/rfc/bcp/bcp47>`_).

    The :py:func:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:

    >>> str(Literal('example'))
    '"example"'
    >>> str(Literal('example', language='en'))
    '"example"@en'
    >>> str(Literal('11', datatype=NamedNode('http://www.w3.org/2001/XMLSchema#integer')))
    '"11"^^<http://www.w3.org/2001/XMLSchema#integer>'
    """

    def __init__(
        self,
        value: str,
        datatype: Union[NamedNode, None] = None,
        language: Union[str, None] = None,
    ) -> None:
        ...

    @property
    def datatype(self) -> NamedNode:
        """the literal `datatype IRI <https://www.w3.org/TR/rdf11-concepts/#dfn-datatype-iri>`_.

        >>> Literal('11', datatype=NamedNode('http://www.w3.org/2001/XMLSchema#integer')).datatype
        <NamedNode value=http://www.w3.org/2001/XMLSchema#integer>
        >>> Literal('example').datatype
        <NamedNode value=http://www.w3.org/2001/XMLSchema#string>
        >>> Literal('example', language='en').datatype
        <NamedNode value=http://www.w3.org/1999/02/22-rdf-syntax-ns#langString>
        """
        return super().datatype

    @property
    def language(self) -> Union[str, None]:
        """the literal `language tag <https://www.w3.org/TR/rdf11-concepts/#dfn-language-tag>`_.

        >>> Literal('example', language='en').language
        'en'
        >>> Literal('example').language
        """
        return super().language

    @property
    def value(self) -> str:
        """the literal value or `lexical form <https://www.w3.org/TR/rdf11-concepts/#dfn-lexical-form>`_.

        >>> Literal("example").value
        'example'
        """
        return super().value


class Triple(PyTriple):
    """An RDF `triple <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple>`_.

    :param subject: the triple subject.
    :param predicate: the triple predicate.
    :param object: the triple object.

    The :py:func:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:

    >>> str(Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
    '<http://example.com> <http://example.com/p> "1"'

    A triple could also be easily destructed into its components:

    >>> (s, p, o) = Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'))
    """

    def __init__(
        self,
        subject: Union[NamedNode, BlankNode, "Triple"],
        predicate: NamedNode,
        object: Union[NamedNode, BlankNode, Literal, "Triple"],
    ) -> None:
        ...

    @property
    def subject(self) -> Union[NamedNode, BlankNode, "Triple"]:
        """the triple subject.

        >>> Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')).subject
        <NamedNode value=http://example.com>
        """
        return super().subject

    @property
    def predicate(self) -> NamedNode:
        """the triple predicate.

        >>> Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')).predicate
        <NamedNode value=http://example.com/p>
        """
        return super().predicate

    @property
    def object(self) -> Union[NamedNode, BlankNode, Literal, "Triple"]:
        """the triple object.

        >>> Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')).object
        <Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>
        """
        return super().object


class Quad(PyQuad):
    """
    An RDF `triple <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-triple>`_.
    in a `RDF dataset <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset>`_.

    :param subject: the quad subject.
    :param predicate: the quad predicate.
    :param object: the quad object.
    :param graph: the quad graph name. If not present, the default graph is assumed.

    The :py:func:`str` function provides a serialization compatible with NTriples, Turtle, and SPARQL:

    >>> str(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    '<http://example.com> <http://example.com/p> "1" <http://example.com/g>'

    >>> str(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), DefaultGraph()))
    '<http://example.com> <http://example.com/p> "1"'

    A quad could also be easily destructed into its components:

    >>> (s, p, o, g) = Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))
    """

    def __init__(
        self,
        subject: Union[NamedNode, BlankNode, Triple],
        predicate: NamedNode,
        object: Union[NamedNode, BlankNode, Literal, Triple],
        graph: Union[NamedNode, BlankNode, "DefaultGraph", None] = None,
    ) -> None:
        ...

    @property
    def subject(self) -> Union[NamedNode, BlankNode, Triple]:
        """the quad subject.

        >>> Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')).subject
        <NamedNode value=http://example.com>
        """
        return super().subject

    @property
    def predicate(self) -> NamedNode:
        """the quad predicate.

        >>> Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')).predicate
        <NamedNode value=http://example.com/p>
        """
        return super().predicate

    @property
    def object(self) -> Union[NamedNode, BlankNode, Literal, Triple]:
        """the quad object.

        >>> Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')).object
        <Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>
        """
        return super().object

    @property
    def graph_name(self) -> Union[NamedNode, BlankNode, "DefaultGraph"]:
        """the quad graph name.

        >>> Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')).graph_name
        <NamedNode value=http://example.com/g>
        """
        return super().graph_name


class DefaultGraph(PyDefaultGraph):
    """The RDF `default graph name <https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph>`_."""
