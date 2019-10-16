use crate::model::*;
use crate::sparql::{GraphPattern, PreparedQuery};
use crate::{DatasetSyntax, GraphSyntax, Result};
use std::io::BufRead;

/// A `Repository` stores a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
/// and allows to query and update it using SPARQL.
///
/// This crate currently provides two implementation of the `Repository` traits:
/// * One in memory: `MemoryRepository`
/// * One disk-based using [RocksDB](https://rocksdb.org/): `RocksDbRepository`
///
/// Usage example with `MemoryRepository`:
/// ```
/// use rudf::model::*;
/// use rudf::{Repository, RepositoryConnection, MemoryRepository, Result};
/// use crate::rudf::sparql::{PreparedQuery, QueryOptions};
/// use rudf::sparql::QueryResult;
///
/// let repository = MemoryRepository::default();
/// let mut connection = repository.connection().unwrap();
///
/// // insertion
/// let ex = NamedNode::parse("http://example.com").unwrap();
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// connection.insert(&quad);
///
/// // quad filter
/// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
/// assert_eq!(vec![quad], results.unwrap());
///
/// // SPARQL query
/// let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", None).unwrap();
/// let options = QueryOptions::default();
/// let results = prepared_query.exec(&options).unwrap();
/// if let QueryResult::Bindings(results) = results {
///     assert_eq!(results.into_values_iter().next().unwrap().unwrap()[0], Some(ex.into()));
/// }
/// ```
///
/// The implementation based on RocksDB if disabled by default and requires the `"rocksdb"` feature to be activated.
/// A `RocksDbRepository` could be built using `RocksDbRepository::open` and works just like its in-memory equivalent:
/// ```ignore
/// use rudf::RocksDbRepository;
/// let dataset = RocksDbRepository::open("example.db").unwrap();
/// ```
///
/// Quads insertion and deletion should respect [ACID](https://en.wikipedia.org/wiki/ACID) properties for all implementation.
/// No complex transaction support is provided yet.
pub trait Repository {
    type Connection: RepositoryConnection;

    fn connection(self) -> Result<Self::Connection>;
}

/// A connection to a `Repository`
pub trait RepositoryConnection: Clone {
    type PreparedQuery: PreparedQuery;

    /// Prepares a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) and returns an object that could be used to execute it.
    ///
    /// The implementation is a work in progress, SPARQL 1.1 specific features are not implemented yet.
    ///
    /// Usage example:
    /// ```
    /// use rudf::model::*;
    /// use rudf::{Repository, RepositoryConnection, MemoryRepository};
    /// use rudf::sparql::{PreparedQuery, QueryOptions};
    /// use rudf::sparql::QueryResult;
    ///
    /// let repository = MemoryRepository::default();
    /// let mut connection = repository.connection().unwrap();
    ///
    /// // insertions
    /// let ex = NamedNode::parse("http://example.com").unwrap();
    /// connection.insert(&Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
    ///
    /// // SPARQL query
    /// let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", None).unwrap();
    /// let options = QueryOptions::default();
    /// let results = prepared_query.exec(&options).unwrap();
    /// if let QueryResult::Bindings(results) = results {
    ///     assert_eq!(results.into_values_iter().next().unwrap().unwrap()[0], Some(ex.into()));
    /// }
    /// ```
    fn prepare_query<'a>(&'a self, query: &str, base_iri: Option<&'a str>) -> Result<Self::PreparedQuery>;


    fn prepare_query_from_pattern<'a>(
        &'a self,
        graph_pattern: &'a GraphPattern,
        base_iri: Option<&str>,
    ) -> Result<Self::PreparedQuery>;

    /// Retrieves quads with a filter on each quad component
    ///
    /// Usage example:
    /// ```
    /// use rudf::model::*;
    /// use rudf::{Repository, RepositoryConnection, MemoryRepository, Result};
    ///
    /// let repository = MemoryRepository::default();
    /// let mut connection = repository.connection().unwrap();
    ///
    /// // insertion
    /// let ex = NamedNode::parse("http://example.com").unwrap();
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
    /// connection.insert(&quad);
    ///
    /// // quad filter
    /// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
    /// assert_eq!(vec![quad], results.unwrap());
    /// ```
    fn quads_for_pattern<'a>(
        &'a self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<Option<&NamedOrBlankNode>>,
    ) -> Box<dyn Iterator<Item = Result<Quad>> + 'a>
    where
        Self: 'a;

    /// Loads a graph file (i.e. triples) into the repository
    ///
    /// Usage example:
    /// ```
    /// use rudf::model::*;
    /// use rudf::{Repository, RepositoryConnection, MemoryRepository, Result, GraphSyntax};
    ///
    /// let repository = MemoryRepository::default();
    /// let mut connection = repository.connection().unwrap();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// connection.load_graph(file.as_ref(), GraphSyntax::NTriples, None, None);
    ///
    /// // quad filter
    /// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::parse("http://example.com").unwrap();
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), None)], results.unwrap());
    /// ```
    fn load_graph(
        &mut self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: Option<&NamedOrBlankNode>,
        base_iri: Option<&str>,
    ) -> Result<()>;

    /// Loads a dataset file (i.e. quads) into the repository
    ///
    /// Usage example:
    /// ```
    /// use rudf::model::*;
    /// use rudf::{Repository, RepositoryConnection, MemoryRepository, Result, DatasetSyntax};
    ///
    /// let repository = MemoryRepository::default();
    /// let mut connection = repository.connection().unwrap();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// connection.load_dataset(file.as_ref(), DatasetSyntax::NQuads, None);
    ///
    /// // quad filter
    /// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::parse("http://example.com").unwrap();
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), Some(ex.into()))], results.unwrap());
    /// ```
    fn load_dataset(
        &mut self,
        reader: impl BufRead,
        syntax: DatasetSyntax,
        base_iri: Option<&str>,
    ) -> Result<()>;

    /// Checks if this repository contains a given quad
    fn contains(&self, quad: &Quad) -> Result<bool>;

    /// Adds a quad to this repository
    fn insert(&mut self, quad: &Quad) -> Result<()>;

    /// Removes a quad from this repository
    fn remove(&mut self, quad: &Quad) -> Result<()>;
}
