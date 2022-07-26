import io
from typing import Union, Iterator

from pyoxigraph import Triple, Quad
from .pyoxigraph import parse as pyparse


def parse(
    input: Union[io.RawIOBase, io.BufferedIOBase, str],
    mime_type: str,
    base_iri: Union[str, None] = None,
) -> Iterator[Union[Triple, Quad]]:
    """Parses RDF graph and dataset serialization formats.

    It currently supports the following formats:

    * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (``application/n-triples``)
    * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (``application/n-quads``)
    * `Turtle <https://www.w3.org/TR/turtle/>`_ (``text/turtle``)
    * `TriG <https://www.w3.org/TR/trig/>`_ (``application/trig``)
    * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (``application/rdf+xml``)

    It supports also some MIME type aliases.
    For example, ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
    and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.

    :param input: The binary I/O object or file path to read from. For example, it could be a file path as a string or a file reader opened in binary mode with ``open('my_file.ttl', 'rb')``.
    :param mime_type: the MIME type of the RDF serialization.
    :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
    :return: an iterator of RDF triples or quads depending on the format.
    :raises ValueError: if the MIME type is not supported.
    :raises SyntaxError: if the provided data is invalid.

    >>> input = io.BytesIO(b'<foo> <p> "1" .')
    >>> list(parse(input, "text/turtle", base_iri="http://example.com/"))
    [<Triple subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>]
    """
    return pyparse(input, mime_type, base_iri)
