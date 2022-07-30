# This file was generated from python/base.pyi using generate_docs.py. 
# Do not modify this file directly.
import io
from typing import Any, Iterator, List, Optional, Union

Subject = Union[NamedNode, BlankNode, Triple]
Term = Union[NamedNode, BlankNode, Literal, Triple]
GraphName = Union[NamedNode, BlankNode, DefaultGraph]

class NamedNode:
    @property
    def value(self) -> str: ...
    def __init__(self, value: str) -> None:
        """
        Initialize self.  See help(type(self)) for accurate signature.
        """
    def __str__(self) -> str: ...
    def __repr__(self) -> str: ...
    def __hash__(self) -> int: ...
    def __eq__(self, other: NamedNode) -> bool: ...
    def __ge__(self, other: NamedNode) -> bool: ...
    def __gt__(self, other: NamedNode) -> bool: ...
    def __le__(self, other: NamedNode) -> bool: ...
    def __lt__(self, other: NamedNode) -> bool: ...
    def __ne__(self, other: NamedNode) -> bool: ...

class BlankNode:
    @property
    def value(self) -> str: ...
    def __init__(self, value: Optional[str] = None) -> None:
        """
        Initialize self.  See help(type(self)) for accurate signature.
        """
    def __eq__(self, other: BlankNode) -> bool: ...
    def __ge__(self, other: BlankNode) -> bool: ...
    def __gt__(self, other: BlankNode) -> bool: ...
    def __hash__(self: BlankNode) -> int: ...
    def __le__(self, other: BlankNode) -> bool: ...
    def __lt__(self, other: BlankNode) -> bool: ...
    def __ne__(self, other: BlankNode) -> bool: ...

class DefaultGraph:
    @property
    def value(self) -> str: ...
    def __init__(self) -> None:
        """
        Initialize self.  See help(type(self)) for accurate signature.
        """
    def __eq__(self, other: Any) -> bool: ...
    def __ge__(self, other: Any) -> bool: ...
    def __gt__(self, other: Any) -> bool: ...
    def __hash__(self) -> int: ...
    def __le__(self, other: Any) -> bool: ...
    def __lt__(self, other: Any) -> bool: ...
    def __ne__(self, other: Any) -> bool: ...

class Literal:
    @property
    def datatype(self) -> NamedNode: ...
    @property
    def language(self) -> Union[str, None]: ...
    @property
    def value(self) -> str: ...
    def __init__(
        self,
        value: str,
        datatype: Optional[NamedNode] = None,
        language: Optional[str] = None,
    ) -> None: ...
    def __eq__(self, other: Literal) -> bool: ...
    def __ge__(self, other: Literal) -> bool: ...
    def __gt__(self, other: Literal) -> bool: ...
    def __hash__(self) -> int: ...
    def __le__(self, other: Literal) -> bool: ...
    def __lt__(self, other: Literal) -> bool: ...
    def __ne__(self, other: Literal) -> bool: ...

class Quad:
    @property
    def subject(self) -> Subject: ...
    @property
    def predicate(self) -> NamedNode: ...
    @property
    def object(self) -> Term: ...
    @property
    def graph_name(self) -> GraphName: ...
    @property
    def triple(self) -> Triple: ...
    def __init__(
        self,
        subject: Subject,
        predicate: NamedNode,
        object: Term,
        graph_name: Optional[GraphName] = None,
    ) -> None: ...
    def __eq__(self, other: Quad) -> bool: ...
    def __ge__(self, other: Quad) -> bool: ...
    def __getitem__(self, index: int) -> Union[Subject, NamedNode, Term, GraphName]: ...
    def __gt__(self, other: Quad) -> bool: ...
    def __hash__(self) -> int: ...
    def __iter__(self) -> Iterator: ...
    def __le__(self, other: Quad) -> bool: ...
    def __len__(self) -> int: ...
    def __lt__(self, other: Quad) -> bool: ...
    def __ne__(self, other: Quad) -> bool: ...

class QuerySolution:
    def __init__(self, *args, **kwargs) -> None:
        """
        Initialize self.  See help(type(self)) for accurate signature.
        """
    def __getitem__(self, index: int) -> Term: ...
    def __iter__(self) -> QuerySolution: ...
    def __next__(self) -> Term: ...
    def __len__(self) -> int: ...

class QuerySolutions:
    @property
    def variables(self) -> List[Variable]: ...
    def __init__(self, *args, **kwargs) -> None:
        """
        Initialize self.  See help(type(self)) for accurate signature.
        """
    def __iter__(self) -> QuerySolutions: ...
    def __next__(self) -> QuerySolution: ...

class QueryTriples:
    def __init__(self, *args, **kwargs) -> None:
        """
        Initialize self.  See help(type(self)) for accurate signature.
        """
    def __iter__(self) -> Triple: ...
    def __next__(self) -> Term: ...

class Store:
    def __init__(self, path: Optional[str] = None) -> None:
        """
        Initialize self.  See help(type(self)) for accurate signature.
        """
    def add(self, quad: Quad) -> None:
        """
        Adds a quad to the store.

        :param quad: the quad to add.
        :type quad: Quad
        :raises IOError: if an I/O error happens during the quad insertion.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> list(store)
        [<Quad subject=<NamedNode value=http://example.com> predicate=<NamedNode value=http://example.com/p> object=<Literal value=1 datatype=<NamedNode value=http://www.w3.org/2001/XMLSchema#string>> graph_name=<NamedNode value=http://example.com/g>>]
        """
    def add_graph(self, graph_name: Union[NamedNode, BlankNode]) -> None:
        """
        Adds a named graph to the store.

        :param graph_name: the name of the name graph to add.
        :type graph_name: NamedNode or BlankNode
        :raises IOError: if an I/O error happens during the named graph insertion.

        >>> store = Store()
        >>> store.add_graph(NamedNode('http://example.com/g'))
        >>> list(store.named_graphs())
        [<NamedNode value=http://example.com/g>]
        """
    def backup(self, target_directory: str) -> None:
        """
        Creates database backup into the `target_directory`.

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
        :type target_directory: str
        :raises IOError: if an I/O error happens during the backup.
        """
    def bulk_load(
        self,
        input: Union[io.RawIOBase, io.BufferedIOBase, str],
        mime_type: str,
        base_iri: Optional[str] = None,
        to_graph: Optional[GraphName] = None,
    ) -> Any: ...
    def clear(self) -> None:
        """
        Clears the store by removing all its contents.

        :raises IOError: if an I/O error happens during the operation.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> store.clear()
        >>> list(store)
        []
        >>> list(store.named_graphs())
        []
        """
    def clear_graph(self, graph_name: GraphName) -> None:
        """
        Clears a graph from the store without removing it.

        :raises IOError: if an I/O error happens during the operation.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> store.clear_graph(NamedNode('http://example.com/g'))
        >>> list(store)
        []
        >>> list(store.named_graphs())
        [<NamedNode value=http://example.com/g>]
        """
    def dump(
        self,
        output: Union[io.RawIOBase, io.BufferedIOBase, str],
        mime_type: str,
        from_graph: Optional[GraphName] = None,
    ) -> None: ...
    def flush(self) -> None:
        """
        Flushes all buffers and ensures that all writes are saved on disk.

        Flushes are automatically done using background threads but might lag a little bit.

        :raises IOError: if an I/O error happens during the flush.
        """
    def load(
        self,
        input: Union[io.RawIOBase, io.BufferedIOBase, str],
        mime_type: str,
        base_iri: Optional[str] = None,
        to_graph: Optional[GraphName] = None,
    ) -> Any: ...
    def named_graphs(self) -> Iterator[Union[NamedNode, BlankNode]]:
        """
        Returns an iterator over all the store named graphs.

        :return: an iterator of the store graph names.
        :rtype: iter(NamedNode or BlankNode)
        :raises IOError: if an I/O error happens during the named graphs lookup.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> list(store.named_graphs())
        [<NamedNode value=http://example.com/g>]
        """
    def optimize(self) -> None:
        """
        Optimizes the database for future workload.

        Useful to call after a batch upload or another similar operation.

        :raises IOError: if an I/O error happens during the optimization.
        """
    def quads_for_pattern(
        self,
        subject: Optional[Subject] = None,
        predicate: Optional[NamedNode] = None,
        object: Optional[Term] = None,
        graph_name: Optional[GraphName] = None,
    ) -> Iterator[Quad]: ...
    def query(
        self,
        query: str,
        base_iri: Optional[str] = None,
        use_default_graph_as_union: bool = False,
        default_graph: Optional[
            Union[
                GraphName,
                List[GraphName],
            ]
        ] = None,
        named_graphs: Optional[List[NamedNode, BlankNode]] = None,
    ) -> Union[QuerySolutions, QueryTriples, bool]: ...
    def remove(self, quad: Quad) -> None:
        """
        Removes a quad from the store.

        :param quad: the quad to remove.
        :type quad: Quad
        :raises IOError: if an I/O error happens during the quad removal.

        >>> store = Store()
        >>> quad = Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g'))
        >>> store.add(quad)
        >>> store.remove(quad)
        >>> list(store)
        []
        """
    def remove_graph(self, graph_name: GraphName) -> None:
        """
        Removes a graph from the store.

        The default graph will not be removed but just cleared.

        :param graph_name: the name of the name graph to remove.
        :type graph_name: NamedNode or BlankNode or DefaultGraph
        :raises IOError: if an I/O error happens during the named graph removal.

        >>> store = Store()
        >>> store.add(Quad(NamedNode('http://example.com'), NamedNode('http://example.com/p'), Literal('1'), NamedNode('http://example.com/g')))
        >>> store.remove_graph(NamedNode('http://example.com/g'))
        >>> list(store.named_graphs())
        []
        """
    def update(self, update: str, base_iri: Optional[str] = None) -> None:
        """
        Executes a `SPARQL 1.1 update <https://www.w3.org/TR/sparql11-update/>`_.

        Updates are applied in a transactional manner: either the full operation succeeds or nothing is written to the database.

        :param update: the update to execute.
        :type update: str
        :param base_iri: the base IRI used to resolve the relative IRIs in the SPARQL update or :py:const:`None` if relative IRI resolution should not be done.
        :type base_iri: str or None, optional
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
    def __bool__(self) -> bool: ...
    def __contains__(self, quad: Quad) -> bool: ...
    def __iter__(self) -> Iterator[Quad]: ...
    def __len__(self) -> int: ...

class Triple:
    @property
    def subject(self) -> Subject: ...
    @property
    def predicate(self) -> NamedNode: ...
    @property
    def object(self) -> Term: ...
    def __init__(
        self,
        subject: Subject,
        predicate: NamedNode,
        object: Term,
    ) -> None: ...
    def __eq__(self, other: Triple) -> bool: ...
    def __ge__(self, other: Triple) -> bool: ...
    def __getitem__(self, index: int) -> Term: ...
    def __gt__(self, other: Triple) -> bool: ...
    def __hash__(self) -> int: ...
    def __iter__(self) -> Iterator[Term]: ...
    def __le__(self, other: Triple) -> bool: ...
    def __len__(self) -> int: ...
    def __lt__(self, other: Triple) -> bool: ...
    def __ne__(self, other: Triple) -> bool: ...

class Variable:
    @property
    def value(self) -> str: ...
    def __init__(self, *args, **kwargs) -> None:
        """
        Initialize self.  See help(type(self)) for accurate signature.
        """
    def __eq__(self, other: Variable) -> bool: ...
    def __ge__(self, other: Variable) -> bool: ...
    def __gt__(self, other: Variable) -> bool: ...
    def __hash__(self) -> int: ...
    def __le__(self, other: Variable) -> bool: ...
    def __lt__(self, other: Variable) -> bool: ...
    def __ne__(self, other: Variable) -> bool: ...

def parse(
    input: Union[io.RawIOBase, io.BufferedIOBase, str],
    mime_type: str,
    base_iri: Optional[str] = None,
) -> Iterator[Union[Triple, Quad]]: ...
def serialize(
    input: Iterator[Union[Triple, Quad]],
    output: Union[io.RawIOBase, io.BufferedIOBase, str],
    mime_type: str,
) -> None: ...
