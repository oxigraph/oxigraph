use crate::model::*;
use crate::sparql::PreparedQuery;
use crate::Result;
use std::io::Read;

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
/// use crate::rudf::sparql::PreparedQuery;
/// use std::str::FromStr;
/// use rudf::sparql::algebra::QueryResult;
///
/// let repository = MemoryRepository::default();
/// let connection = repository.connection().unwrap();
///
/// // insertion
/// let ex = NamedNode::new("http://example.com");
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// connection.insert(&quad);
///
/// // quad filter
/// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
/// assert_eq!(vec![quad], results.unwrap());
///
/// // SPARQL query
/// let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }".as_bytes()).unwrap();
/// let results = prepared_query.exec().unwrap();
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

    /// Prepares a [SPARQL 1.1](https://www.w3.org/TR/sparql11-query/) query and returns an object that could be used to execute it.
    ///
    /// The implementation is a work in progress, SPARQL 1.1 specific features are not implemented yet.
    ///
    /// Usage example:
    /// ```
    /// use rudf::model::*;
    /// use rudf::{Repository, RepositoryConnection, MemoryRepository};
    /// use rudf::sparql::PreparedQuery;
    /// use rudf::sparql::algebra::QueryResult;
    /// use std::str::FromStr;
    ///
    /// let repository = MemoryRepository::default();
    /// let connection = repository.connection().unwrap();
    ///
    /// // insertions
    /// let ex = NamedNode::new("http://example.com");
    /// connection.insert(&Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
    ///
    /// // SPARQL query
    /// let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }".as_bytes()).unwrap();
    /// let results = prepared_query.exec().unwrap();
    /// if let QueryResult::Bindings(results) = results {
    ///     assert_eq!(results.into_values_iter().next().unwrap().unwrap()[0], Some(ex.into()));
    /// }
    /// ```
    fn prepare_query(&self, query: impl Read) -> Result<Self::PreparedQuery>;

    /// Retrieves quads with a filter on each quad component
    ///
    /// Usage example:
    /// ```
    /// use rudf::model::*;
    /// use rudf::{Repository, RepositoryConnection, MemoryRepository, Result};
    /// use std::str::FromStr;
    ///
    /// let repository = MemoryRepository::default();
    /// let connection = repository.connection().unwrap();
    ///
    /// // insertion
    /// let ex = NamedNode::new("http://example.com");
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
        graph_name: Option<&NamedOrBlankNode>,
    ) -> Box<dyn Iterator<Item = Result<Quad>> + 'a>
    where
        Self: 'a;

    /// Checks if this dataset contains a given quad
    fn contains(&self, quad: &Quad) -> Result<bool>;

    /// Adds a quad to this dataset
    fn insert(&self, quad: &Quad) -> Result<()>;

    /// Removes a quad from this dataset
    fn remove(&self, quad: &Quad) -> Result<()>;
}
