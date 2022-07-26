import io
import mimetypes
from typing import Iterator, List, Union

from pyoxigraph import (
    Quad,
    BlankNode,
    DefaultGraph,
    Literal,
)

from pyoxigraph.sparql import QuerySolutions, QueryTriples

from .pyoxigraph import NamedNode, Store as PyStore


class Store(PyStore):
    """RDF store.

    It encodes a `RDF dataset <https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset>`_ and allows to query it using SPARQL.
    It is based on the `RocksDB <https://rocksdb.org/>`_ key-value database.

    This store ensures the "repeatable read" isolation level: the store only exposes changes that have
    been "committed" (i.e. no partial writes) and the exposed state does not change for the complete duration
    of a read operation (e.g. a SPARQL query) or a read/write operation (e.g. a SPARQL update).

    :param path: the path of the directory in which the store should read and write its data. If the directory does not exist, it is created.
                 If no directory is provided a temporary one is created and removed when the Python garbage collector removes the store.
                 In this case, the store data are kept in memory and never written on disk.
    :raises IOError: if the target directory contains invalid data or could not be accessed.

    The :py:func:`str` function provides a serialization of the store in NQuads:

    >>> store = Store()
    >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
    >>> str(store)
    '<http://example.com> <http://example.com/p> "1" <http://example.com/g> .\\n'
    """

    def __init__(self, path: Union[str, None] = None) -> None:
        ...

    def add(self, quad: Quad) -> None:
        """Adds a quad to the store.

        :param quad: the quad to add.
        :type quad: Quad
        :raises IOError: if an I/O error happens during the quad insertion.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> list(store)
        [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
        """
        super().add(quad)

    def add_graph(self, graph_name: Union[NamedNode, BlankNode]) -> None:
        """Adds a named graph to the store.

        :param graph_name: the name of the name graph to add.
        :raises IOError: if an I/O error happens during the named graph insertion.

        >>> store = Store()
        >>> store.add_graph(NamedNode('http://example.com/g'))
        >>> list(store.named_graphs())
        [<NamedNode value=http://example.com/g>]
        """
        super().add_graph(graph_name)

    def backup(self, target_directory: str) -> None:
        """Creates database backup into the `target_directory`.

        After its creation, the backup is usable using :py:class:`Store` constructor.
        like a regular pyxigraph database and operates independently from the original database.

        Warning: Backups are only possible for on-disk databases created by providing a path to :py:class:`Store` constructor.
        Temporary in-memory databases created without path are not compatible with the backup system.

        Warning: An error is raised if the ``target_directory`` already exists.

        If the target directory is in the same file system as the current database,
        the database content will not be fully copied
        but hard links will be used to point to the original database immutable snapshots.
        This allows cheap regular backups.

        If you want to move your data to another RDF storage system, you should have a look at the :py:func:`dump_dataset` function instead.

        :param target_directory: the directory name to save the database to.
        :raises IOError: if an I/O error happens during the backup.
        """
        super().backup(target_directory)

    def bulk_load(
        self,
        input: Union[io.RawIOBase, io.BufferedIOBase, str],
        mime_type: str,
        base_iri: Union[str, None] = None,
        to_graph: Union[NamedNode, BlankNode, DefaultGraph, None] = None,
    ) -> None:
        """Loads an RDF serialization into the store.

        This function is designed to be as fast as possible on big files **without** transactional guarantees.
        If the file is invalid only a piece of it might be written to the store.

        The :py:func:`load` method is also available for loads with transactional guarantees.

        It currently supports the following formats:

        * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (``application/n-triples``)
        * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (``application/n-quads``)
        * `Turtle <https://www.w3.org/TR/turtle/>`_ (``text/turtle``)
        * `TriG <https://www.w3.org/TR/trig/>`_ (``application/trig``)
        * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (``application/rdf+xml``)

        It supports also some MIME type aliases.
        For example, ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
        and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.

        :param input: the binary I/O object or file path to read from. For example, it could be a file path as a string or a file reader opened in binary mode with ``open('my_file.ttl', 'rb')``.
        :param mime_type: the MIME type of the RDF serialization.
        :param base_iri: the base IRI used to resolve the relative IRIs in the file or :py:const:`None` if relative IRI resolution should not be done.
        :param to_graph: if it is a file composed of triples, the graph in which the triples should be stored. By default, the default graph is used.
        :raises ValueError: if the MIME type is not supported or the `to_graph` parameter is given with a quad file.
        :raises SyntaxError: if the provided data is invalid.
        :raises IOError: if an I/O error happens during a quad insertion.

        >>> store = Store()
        >>> store.bulk_load(io.BytesIO(b'<foo> <p> "1" .'), "text/turtle", base_iri="http://example.com/", to_graph=NamedNode("http://example.com/g"))
        >>> list(store)
        [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
        """
        super().bulk_load(input, mimetypes, base_iri, to_graph)

    def clear(self) -> None:
        """Clears the store by removing all its contents.

        :raises IOError: if an I/O error happens during the operation.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> store.clear()
        >>> list(store)
        []
        >>> list(store.named_graphs())
        []
        """
        super().clear()

    def clear_graph(self, graph_name: NamedNode) -> None:
        """Clears a graph from the store without removing it.

        :param graph_name: the name of the name graph to clear.
        :raises IOError: if an I/O error happens during the operation.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> store.clear_graph(NamedNode('http://example.com/g'))
        >>> list(store)
        []
        >>> list(store.named_graphs())
        [<NamedNode value=http://example.com/g>]
        """
        super().clear_graph(graph_name)

    def dump(
        self,
        output: Union[io.RawIOBase, io.BufferedIOBase, str],
        mime_type: str,
        from_graph: Union[NamedNode, BlankNode, DefaultGraph, None] = None,
    ) -> None:
        """Dumps the store quads or triples into a file.

        It currently supports the following formats:

        * `N-Triples <https://www.w3.org/TR/n-triples/>`_ (``application/n-triples``)
        * `N-Quads <https://www.w3.org/TR/n-quads/>`_ (``application/n-quads``)
        * `Turtle <https://www.w3.org/TR/turtle/>`_ (``text/turtle``)
        * `TriG <https://www.w3.org/TR/trig/>`_ (``application/trig``)
        * `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_ (``application/rdf+xml``)

        It supports also some MIME type aliases.
        For example, ``application/turtle`` could also be used for `Turtle <https://www.w3.org/TR/turtle/>`_
        and ``application/xml`` for `RDF/XML <https://www.w3.org/TR/rdf-syntax-grammar/>`_.

        :param output: The binary I/O object or file path to write to. For example, it could be a file path as a string or a file writer opened in binary mode with ``open('my_file.ttl', 'wb')``.
        :param mime_type: the MIME type of the RDF serialization.
        :param from_graph: if a triple based format is requested, the store graph from which dump the triples. By default, the default graph is used.
        :raises ValueError: if the MIME type is not supported or the `from_graph` parameter is given with a quad syntax.
        :raises IOError: if an I/O error happens during a quad lookup

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> output = io.BytesIO()
        >>> store.dump(output, "text/turtle", from_graph=NamedNode("http://example.com/g"))
        >>> output.getvalue()
        b'<http://example.com> <http://example.com/p> "1" .\\n'
        """
        super().dump(output, mime_type, from_graph=from_graph)

    def flush(self) -> None:
        """Flushes all buffers and ensures that all writes are saved on disk.

        Flushes are automatically done using background threads but might lag a little bit.

        :raises IOError: if an I/O error happens during the flush.
        """
        super().flush()

    def load(
        self,
        input: Union[io.RawIOBase, io.BufferedIOBase, str],
        mime_type: str,
        base_iri: Union[str, None] = None,
        to_graph: Union[NamedNode, BlankNode, DefaultGraph, None] = None,
    ) -> None:
        """Loads an RDF serialization into the store.

        Loads are applied in a transactional manner: either the full operation succeeds or nothing is written to the database.
        The :py:func:`bulk_load` method is also available for much faster loading of big files but without transactional guarantees.

        Beware, the full file is loaded into memory.

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
        :param to_graph: if it is a file composed of triples, the graph in which the triples should be stored. By default, the default graph is used.
        :raises ValueError: if the MIME type is not supported or the `to_graph` parameter is given with a quad file.
        :raises SyntaxError: if the provided data is invalid.
        :raises IOError: if an I/O error happens during a quad insertion.

        >>> store = Store()
        >>> store.load(io.BytesIO(b'<foo> <p> "1" .'), "text/turtle", base_iri="http://example.com/", to_graph=NamedNode("http://example.com/g"))
        >>> list(store)
        [<Quad subject=<NamedNode value=http://example.com/foo> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
        """
        super().load(input, mime_type, base_iri=base_iri, to_graph=to_graph)

    def named_graphs(self) -> Iterator[Union[NamedNode, BlankNode]]:
        """Returns an iterator over all the store named graphs.

        :return: an iterator of the store graph names.
        :raises IOError: if an I/O error happens during the named graphs lookup.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> list(store.named_graphs())
        [<NamedNode value=http://example.com/g>]
        """
        return super().named_graphs()

    def optimize(self) -> None:
        """Optimizes the database for future workload.

        Useful to call after a batch upload or another similar operation.

        :raises IOError: if an I/O error happens during the optimization.
        """
        super().optimize()

    def quads_for_pattern(
        self,
        subject: Union[NamedNode, BlankNode, None] = None,
        predicate: Union[NamedNode, None] = None,
        object: Union[NamedNode, BlankNode, Literal, None] = None,
        graph: Union[NamedNode, BlankNode, DefaultGraph, None] = None,
    ) -> Iterator[Quad]:
        """Looks for the quads matching a given pattern.

        :param subject: the quad subject or :py:const:`None` to match everything.
        :param predicate: the quad predicate or :py:const:`None` to match everything.
        :param object: the quad object or :py:const:`None` to match everything.
        :param graph: the quad graph name. To match only the default graph, use :py:class:`DefaultGraph`. To match everything use :py:const:`None`.
        :return: an iterator of the quads matching the pattern.
        :rtype: iter(Quad)
        :raises IOError: if an I/O error happens during the quads lookup.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> list(store.quads_for_pattern(NamedNode('http://example.com'), None, None, None))
        [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
        """
        return super().quads_for_pattern(subject, predicate, object, graph)

    def query(
        self,
        query: str,
        base_iri: Union[str, None] = None,
        use_default_graph_as_union: bool = False,
        default_graph: Union[
            NamedNode,
            BlankNode,
            DefaultGraph,
            List[Union[NamedNode, BlankNode, DefaultGraph]],
            None,
        ] = None,
        named_graphs: Union[List[Union[NamedNode, BlankNode]], None] = None,
    ) -> Union[QuerySolutions, QueryTriples, bool]:
        """Executes a `SPARQL 1.1 query <https://www.w3.org/TR/sparql11-query/>`_.

        :param query: the query to execute.
        :param base_iri: the base IRI used to resolve the relative IRIs in the SPARQL query or :py:const:`None` if relative IRI resolution should not be done.
        :param use_default_graph_as_union: if the SPARQL query should look for triples in all the dataset graphs by default (i.e. without `GRAPH` operations). Disabled by default.
        :param default_graph: list of the graphs that should be used as the query default graph. By default, the store default graph is used.
        :param named_graphs: list of the named graphs that could be used in SPARQL `GRAPH` clause. By default, all the store named graphs are available.
        :return: a :py:class:`bool` for ``ASK`` queries, an iterator of :py:class:`Triple` for ``CONSTRUCT`` and ``DESCRIBE`` queries and an iterator of :py:class:`QuerySolution` for ``SELECT`` queries.
        :raises SyntaxError: if the provided query is invalid.
        :raises IOError: if an I/O error happens while reading the store.

        ``SELECT`` query:

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
        >>> list(solution['s'] for solution in store.query('SELECT ?s WHERE { ?s ?p ?o }'))
        [<NamedNode value=http://example.com>]

        ``CONSTRUCT`` query:

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
        >>> list(store.query('CONSTRUCT WHERE { ?s ?p ?o }'))
        [<Triple subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>>>]

        ``ASK`` query:

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
        >>> store.query('ASK { ?s ?p ?o }')
        True
        """
        return super().query(
            query,
            base_iri=base_iri,
            use_default_graph_as_union=use_default_graph_as_union,
            default_graph=default_graph,
            named_graphs=named_graphs,
        )

    def remove(self, quad: Quad) -> None:
        """Removes a quad from the store.

        :param quad: the quad to remove.
        :raises IOError: if an I/O error happens during the quad removal.

        >>> store = Store()
        >>> quad = Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))
        >>> store.add(quad)
        >>> store.remove(quad)
        >>> list(store)
        []
        """
        super().remove(quad)

    def remove_graph(
        self, graph_name: Union[NamedNode, BlankNode, DefaultGraph]
    ) -> None:
        """Removes a graph from the store.

        The default graph will not be removed but just cleared.

        :param graph_name: the name of the name graph to remove.
        :raises IOError: if an I/O error happens during the named graph removal.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> store.remove_graph(NamedNode('http://example.com/g'))
        >>> list(store.named_graphs())
        []
        """
        super().remove_graph(graph_name)

    def update(
        self,
        update: str,
        base_iri: Union[str, None] = None,
    ) -> None:
        """Executes a `SPARQL 1.1 update <https://www.w3.org/TR/sparql11-update/>`_.

        Updates are applied in a transactional manner: either the full operation succeeds or nothing is written to the database.

        :param update: the update to execute.
        :param base_iri: the base IRI used to resolve the relative IRIs in the SPARQL update or :py:const:`None` if relative IRI resolution should not be done.
        :raises SyntaxError: if the provided update is invalid.
        :raises IOError: if an I/O error happens while reading the store.

        ``INSERT DATA`` update:

        >>> store = Store()
        >>> store.update('INSERT DATA { <http://example.com> <http://example.com/p> "1" }')
        >>> list(store)
        [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<DefaultGraph>>]

        ``DELETE DATA`` update:

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
        >>> store.update('DELETE DATA { <http://example.com> <http://example.com/p> "1" }')
        >>> list(store)
        []

        ``DELETE`` update:

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1')))
        >>> store.update('DELETE WHERE { <http://example.com> ?p ?o }')
        >>> list(store)
        []
        """
        super().update(update, base_iri=base_iri)
