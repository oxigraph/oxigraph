import io
from typing import Iterator, Union

from pyoxigraph import Triple, Quad
from .pyoxigraph import serialize as pyserialize


def serialize(
    input: Iterator[Union[Triple, Quad]],
    output: Union[io.RawIOBase, io.BufferedIOBase, str],
    mime_type: str,
):
    """Serializes an RDF graph or dataset.

    It currently supports the following formats:

    * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (``application/n-triples``)
    * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (``application/n-quads``)
    * `Turtle <https://www.w3.org/TR/turtle/>`_ (``text/turtle``)
    * `TriG <https://www.w3.org/TR/trig/>`_ (``application/trig``)
    * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (``application/rdf+xml``)

    It supports also some MIME type aliases.
    For example, ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.

    :param input: the RDF triples and quads to serialize.
    :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``.
    :param mime_type: the MIME type of the RDF serialization.
    :raises ValueError: if the MIME type is not supported.
    :raises TypeError: if a triple is given during a quad format serialization or reverse.

    >>> output = io.BytesIO()
    >>> serialize([Triple(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'))], output, "text/turtle")
    >>> output.getvalue()
    b'<http://example.com> <http://example.com/p> "1" .\n'
    """
    return pyserialize(input, output, mime_type)
