use crate::model::*;
use crate::sparql::{GraphPattern, PreparedQuery, QueryOptions};
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
/// use oxigraph::model::*;
/// use oxigraph::{Repository, RepositoryConnection, MemoryRepository, Result};
/// use crate::oxigraph::sparql::{PreparedQuery, QueryOptions};
/// use oxigraph::sparql::QueryResult;
///
/// let repository = MemoryRepository::default();
/// let mut connection = repository.connection()?;
///
/// // insertion
/// let ex = NamedNode::parse("http://example.com")?;
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// connection.insert(&quad);
///
/// // quad filter
/// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
/// assert_eq!(vec![quad], results?);
///
/// // SPARQL query
/// let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
/// let results = prepared_query.exec()?;
/// if let QueryResult::Bindings(results) = results {
///     assert_eq!(results.into_values_iter().next().unwrap()?[0], Some(ex.into()));
/// }
/// # Result::Ok(())
/// ```
///
/// The implementation based on RocksDB if disabled by default and requires the `"rocksdb"` feature to be activated.
/// A `RocksDbRepository` could be built using `RocksDbRepository::open` and works just like its in-memory equivalent:
/// ```ignore
/// use oxigraph::RocksDbRepository;
/// let dataset = RocksDbRepository::open("example.db")?;
/// ```
///
/// If you want transaction with [ACID](https://en.wikipedia.org/wiki/ACID) properties you could use the `RepositoryConnection.transaction` method.
/// This transaction support is only limited to writes and does not support reads as part of transactions yet.
pub trait Repository {
    type Connection: RepositoryConnection;

    fn connection(self) -> Result<Self::Connection>;
}

/// A connection to a `Repository`
pub trait RepositoryConnection: Clone {
    type Transaction: RepositoryTransaction;
    type PreparedQuery: PreparedQuery;

    /// Prepares a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) and returns an object that could be used to execute it.
    ///
    /// The implementation is a work in progress, SPARQL 1.1 specific features are not implemented yet.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{Repository, RepositoryConnection, MemoryRepository, Result};
    /// use oxigraph::sparql::{PreparedQuery, QueryOptions};
    /// use oxigraph::sparql::QueryResult;
    ///
    /// let repository = MemoryRepository::default();
    /// let mut connection = repository.connection()?;
    ///
    /// // insertions
    /// let ex = NamedNode::parse("http://example.com")?;
    /// connection.insert(&Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
    ///
    /// // SPARQL query
    /// let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
    /// let results = prepared_query.exec()?;
    /// if let QueryResult::Bindings(results) = results {
    ///     assert_eq!(results.into_values_iter().next().unwrap()?[0], Some(ex.into()));
    /// }
    /// # Result::Ok(())
    /// ```
    fn prepare_query(&self, query: &str, options: QueryOptions<'_>) -> Result<Self::PreparedQuery>;

    /// This is similar to `prepare_query`, but useful if a SPARQL query has already been parsed, which is the case when building `ServiceHandler`s for federated queries with `SERVICE` clauses. For examples, look in the tests.
    fn prepare_query_from_pattern(
        &self,
        graph_pattern: &GraphPattern,
        options: QueryOptions<'_>,
    ) -> Result<Self::PreparedQuery>;

    /// Retrieves quads with a filter on each quad component
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{Repository, RepositoryConnection, MemoryRepository, Result};
    ///
    /// let repository = MemoryRepository::default();
    /// let mut connection = repository.connection()?;
    ///
    /// // insertion
    /// let ex = NamedNode::parse("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
    /// connection.insert(&quad);
    ///
    /// // quad filter
    /// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
    /// assert_eq!(vec![quad], results?);
    /// # Result::Ok(())
    /// ```
    #[allow(clippy::option_option)]
    fn quads_for_pattern<'a>(
        &'a self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<Option<&NamedOrBlankNode>>,
    ) -> Box<dyn Iterator<Item = Result<Quad>> + 'a>
    where
        Self: 'a;

    /// Checks if this repository contains a given quad
    fn contains(&self, quad: &Quad) -> Result<bool>;

    /// Executes a transaction.
    ///
    /// The transaction is executed if the given closure returns `Ok`.
    /// Nothing is done if the clusre returns `Err`.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{Repository, RepositoryConnection, RepositoryTransaction, MemoryRepository, Result};
    ///
    /// let repository = MemoryRepository::default();
    /// let mut connection = repository.connection()?;
    ///
    /// let ex = NamedNode::parse("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
    ///
    /// // transaction
    /// connection.transaction(|transaction| {
    ///     transaction.insert(&quad)
    /// });
    ///
    /// // quad filter
    /// assert!(connection.contains(&quad).unwrap());
    /// # Result::Ok(())
    /// ```
    fn transaction(&self, f: impl FnOnce(&mut Self::Transaction) -> Result<()>) -> Result<()>;

    /// Loads a graph file (i.e. triples) into the repository
    ///
    /// Warning: This functions saves the triples in batch. If the parsing fails in the middle of the file,
    /// only a part of it may be written. Use a (memory greedy) transaction if you do not want that.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{Repository, RepositoryConnection, MemoryRepository, Result, GraphSyntax};
    ///
    /// let repository = MemoryRepository::default();
    /// let mut connection = repository.connection()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// connection.load_graph(file.as_ref(), GraphSyntax::NTriples, None, None);
    ///
    /// // quad filter
    /// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::parse("http://example.com")?;
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), None)], results?);
    /// # Result::Ok(())
    /// ```
    fn load_graph(
        &mut self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: Option<&NamedOrBlankNode>,
        base_iri: Option<&str>,
    ) -> Result<()>;

    /// Loads a dataset file (i.e. quads) into the repository.
    ///
    /// Warning: This functions saves the quads in batch. If the parsing fails in the middle of the file,
    /// only a part of it may be written. Use a (memory greedy) transaction if you do not want that.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{Repository, RepositoryConnection, MemoryRepository, Result, DatasetSyntax};
    ///
    /// let repository = MemoryRepository::default();
    /// let mut connection = repository.connection()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// connection.load_dataset(file.as_ref(), DatasetSyntax::NQuads, None);
    ///
    /// // quad filter
    /// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::parse("http://example.com")?;
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), Some(ex.into()))], results?);
    /// # Result::Ok(())
    /// ```
    fn load_dataset(
        &mut self,
        reader: impl BufRead,
        syntax: DatasetSyntax,
        base_iri: Option<&str>,
    ) -> Result<()>;

    /// Adds a quad to this repository.
    ///
    /// If you want to insert a lot of quads at the same time,
    /// you should probably use an `auto_transaction`.
    ///
    /// To make a transaction, you could use `transaction`.
    fn insert(&mut self, quad: &Quad) -> Result<()>;

    /// Removes a quad from this repository.
    ///
    /// If you want to remove a lot of quads at the same time,
    /// you should probably use an `auto_transaction`.
    ///
    /// To make a transaction, you could use `transaction`.
    fn remove(&mut self, quad: &Quad) -> Result<()>;
}

/// A transaction done on a `RepositoryConnection`
pub trait RepositoryTransaction {
    /// Adds quads from a graph file into the transaction insertions.
    ///
    /// Warning: It loads all the files triples into main memory.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{Repository, RepositoryConnection, RepositoryTransaction, MemoryRepository, Result, GraphSyntax};
    ///
    /// let repository = MemoryRepository::default();
    /// let connection = repository.connection()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// connection.transaction(|transaction|
    ///     transaction.load_graph(file.as_ref(), GraphSyntax::NTriples, None, None)
    /// );
    ///
    /// // quad filter
    /// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::parse("http://example.com")?;
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), None)], results?);
    /// # Result::Ok(())
    /// ```
    fn load_graph(
        &mut self,
        reader: impl BufRead,
        syntax: GraphSyntax,
        to_graph_name: Option<&NamedOrBlankNode>,
        base_iri: Option<&str>,
    ) -> Result<()>;

    /// Adds quads from a dataset file into the transaction insertions.
    ///
    /// Warning: It loads all the files quads into main memory.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::{Repository, RepositoryConnection, RepositoryTransaction, MemoryRepository, Result, DatasetSyntax};
    ///
    /// let repository = MemoryRepository::default();
    /// let connection = repository.connection()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// connection.transaction(|transaction|
    ///     transaction.load_dataset(file.as_ref(), DatasetSyntax::NQuads, None)
    /// );
    ///
    /// // quad filter
    /// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
    /// let ex = NamedNode::parse("http://example.com")?;
    /// assert_eq!(vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), Some(ex.into()))], results?);
    /// # Result::Ok(())
    /// ```
    fn load_dataset(
        &mut self,
        reader: impl BufRead,
        syntax: DatasetSyntax,
        base_iri: Option<&str>,
    ) -> Result<()>;

    /// Adds a quad insertion to this transaction
    fn insert(&mut self, quad: &Quad) -> Result<()>;

    /// Adds a quad removals for this transaction
    fn remove(&mut self, quad: &Quad) -> Result<()>;
}
