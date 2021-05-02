//! API to access an on-on disk [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
//!
//! Usage example:
//! ```
//! use oxigraph::store::Store;
//! use oxigraph::sparql::QueryResults;
//! use oxigraph::model::*;
//!
//! let store = Store::new()?;
//!
//! // insertion
//! let ex = NamedNode::new("http://example.com")?;
//! let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
//! store.insert(&quad)?;
//!
//! // quad filter
//! let results: Result<Vec<Quad>,_> = store.quads_for_pattern(None, None, None, None).collect();
//! assert_eq!(vec![quad], results?);
//!
//! // SPARQL query
//! if let QueryResults::Solutions(mut solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
//!     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
//! };
//! # Result::<_,Box<dyn std::error::Error>>::Ok(())
//! ```

use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use crate::sparql::{
    evaluate_query, evaluate_update, EvaluationError, Query, QueryOptions, QueryResults, Update,
    UpdateOptions,
};
use crate::storage::io::{dump_dataset, dump_graph, load_dataset, load_graph};
use crate::storage::numeric_encoder::{Decoder, EncodedQuad, EncodedTerm, WriteEncoder};
pub use crate::storage::ConflictableTransactionError;
pub use crate::storage::TransactionError;
pub use crate::storage::UnabortableTransactionError;
use crate::storage::{
    ChainedDecodingQuadIterator, DecodingGraphIterator, Storage, StorageTransaction,
};
use std::convert::TryInto;
use std::io::{BufRead, Write};
use std::path::Path;
use std::{fmt, io, str};

/// An on-on disk [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
/// Allows to query and update it using SPARQL.
/// It is based on the [Sled](https://sled.rs/) key-value database.
///
/// Warning: Sled is not stable yet and might break its storage format.
///
/// Usage example:
/// ```
/// use oxigraph::store::Store;
/// use oxigraph::sparql::QueryResults;
/// use oxigraph::model::*;
/// # use std::fs::remove_dir_all;
///
/// # {
/// let store = Store::open("example.db")?;
///
/// // insertion
/// let ex = NamedNode::new("http://example.com")?;
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
/// store.insert(&quad)?;
///
/// // quad filter
/// let results: Result<Vec<Quad>,_> = store.quads_for_pattern(None, None, None, None).collect();
/// assert_eq!(vec![quad], results?);
///
/// // SPARQL query
/// if let QueryResults::Solutions(mut solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// };
/// #
/// # };
/// # remove_dir_all("example.db")?;
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone)]
pub struct Store {
    storage: Storage,
}

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

impl Store {
    /// Creates a temporary [`Store`]() that will be deleted after drop.
    pub fn new() -> Result<Self, io::Error> {
        Ok(Self {
            storage: Storage::new()?,
        })
    }

    /// Opens a [`Store`]() and creates it if it does not exist yet.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        Ok(Self {
            storage: Storage::open(path.as_ref())?,
        })
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::QueryResults;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertions
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// store.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?;
    ///
    /// // SPARQL query
    /// if let QueryResults::Solutions(mut solutions) =  store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
    ///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into_owned().into()));
    /// }
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn query(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
    ) -> Result<QueryResults, EvaluationError> {
        self.query_opt(query, QueryOptions::default())
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) with some options.
    pub fn query_opt(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
    ) -> Result<QueryResults, EvaluationError> {
        evaluate_query(self.storage.clone(), query, options)
    }

    /// Retrieves quads with a filter on each quad component
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
    /// store.insert(&quad)?;
    ///
    /// // quad filter by object
    /// let results = store.quads_for_pattern(None, None, Some((&ex).into()), None).collect::<Result<Vec<_>,_>>()?;
    /// assert_eq!(vec![quad], results);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn quads_for_pattern(
        &self,
        subject: Option<SubjectRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
        graph_name: Option<GraphNameRef<'_>>,
    ) -> QuadIter {
        QuadIter {
            iter: self.storage.quads_for_pattern(
                subject.map(EncodedTerm::from).as_ref(),
                predicate.map(EncodedTerm::from).as_ref(),
                object.map(EncodedTerm::from).as_ref(),
                graph_name.map(EncodedTerm::from).as_ref(),
            ),
            storage: self.storage.clone(),
        }
    }

    /// Returns all the quads contained in the store
    pub fn iter(&self) -> QuadIter {
        self.quads_for_pattern(None, None, None, None)
    }

    /// Checks if this store contains a given quad
    pub fn contains<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, io::Error> {
        let quad = EncodedQuad::from(quad.into());
        self.storage.contains(&quad)
    }

    /// Returns the number of quads in the store
    ///
    /// Warning: this function executes a full scan
    pub fn len(&self) -> usize {
        self.storage.len()
    }

    /// Returns if the store is empty
    pub fn is_empty(&self) -> bool {
        self.storage.is_empty()
    }

    /// Executes a [SPARQL 1.1 update](https://www.w3.org/TR/sparql11-update/).
    ///
    /// The store does not track the existence of empty named graphs.
    /// This method has no ACID guarantees.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// store.update("INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }")?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com").unwrap();
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn update(
        &self,
        update: impl TryInto<Update, Error = impl Into<EvaluationError>>,
    ) -> Result<(), EvaluationError> {
        self.update_opt(update, UpdateOptions::default())
    }

    /// Executes a [SPARQL 1.1 update](https://www.w3.org/TR/sparql11-update/) with some options.
    pub fn update_opt(
        &self,
        update: impl TryInto<Update, Error = impl Into<EvaluationError>>,
        options: UpdateOptions,
    ) -> Result<(), EvaluationError> {
        evaluate_update(
            &self.storage,
            update.try_into().map_err(|e| e.into())?,
            options,
        )
    }

    /// Executes an ACID transaction.
    ///
    /// The transaction is executed if the given closure returns `Ok`.
    /// The transaction is rollbacked if the closure returns `Err`.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::{ConflictableTransactionError, Store};
    /// use oxigraph::model::*;
    /// use std::convert::Infallible;
    ///
    /// let store = Store::new()?;
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let quad = QuadRef::new(ex, ex, ex, ex);
    ///
    /// // transaction
    /// store.transaction(|transaction| {
    ///     transaction.insert(quad)?;
    ///     Ok(()) as Result<(),ConflictableTransactionError<Infallible>>
    /// })?;
    ///
    /// assert!(store.contains(quad)?);
    /// assert!(store.contains_named_graph(ex)?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn transaction<T, E>(
        &self,
        f: impl Fn(Transaction<'_>) -> Result<T, ConflictableTransactionError<E>>,
    ) -> Result<T, TransactionError<E>> {
        self.storage
            .transaction(|storage| f(Transaction { storage }))
    }

    /// Loads a graph file (i.e. triples) into the store
    ///
    /// Warning: This functions saves the triples in a not atomic way.
    /// If the parsing fails in the middle of the file only a part of it may be written to the store.
    /// It might leave the store in a bad state if a crash happens during a triple insertion.
    /// Use a (memory greedy) [transaction](Store::transaction()) if you do not want that.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::*;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.load_graph(file.as_ref(), GraphFormat::NTriples, &GraphName::DefaultGraph, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    /// Errors related to data loading into the store use the other error kinds.
    pub fn load_graph<'a>(
        &self,
        reader: impl BufRead,
        format: GraphFormat,
        to_graph_name: impl Into<GraphNameRef<'a>>,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        load_graph(
            &self.storage,
            reader,
            format,
            to_graph_name.into(),
            base_iri,
        )?;
        Ok(())
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// Warning: This functions saves the triples in a not atomic way.
    /// If the parsing fails in the middle of the file, only a part of it may be written to the store.
    /// It might leave the store in a bad state if a crash happens during a quad insertion.
    /// Use a (memory greedy) [transaction](Store::transaction()) if you do not want that.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::DatasetFormat;
    /// use oxigraph::model::*;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store.load_dataset(file.as_ref(), DatasetFormat::NQuads, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    /// Errors related to data loading into the store use the other error kinds.
    pub fn load_dataset(
        &self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        load_dataset(&self.storage, reader, format, base_iri)?;
        Ok(())
    }

    /// Adds a quad to this store.
    ///
    /// Returns `true` if the quad was not already in the store.
    ///
    /// This method is optimized for performances and is not atomic.
    /// It might leave the store in a bad state if a crash happens during the insertion.
    /// Use a (memory greedy) [transaction](Store::transaction()) if you do not want that.
    pub fn insert<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, io::Error> {
        let quad = self.storage.encode_quad(quad.into())?;
        self.storage.insert(&quad)
    }

    /// Removes a quad from this store.
    ///
    /// Returns `true` if the quad was in the store and has been removed.
    ///
    /// This method is optimized for performances and is not atomic.
    /// It might leave the store in a bad state if a crash happens during the removal.
    /// Use a (memory greedy) [transaction](Store::transaction()) if you do not want that.
    pub fn remove<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, io::Error> {
        let quad = EncodedQuad::from(quad.into());
        self.storage.remove(&quad)
    }

    /// Dumps a store graph into a file.
    ///    
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::GraphName;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = Store::new()?;
    /// store.load_graph(file, GraphFormat::NTriples, &GraphName::DefaultGraph, None)?;
    ///
    /// let mut buffer = Vec::new();
    /// store.dump_graph(&mut buffer, GraphFormat::NTriples, &GraphName::DefaultGraph)?;
    /// assert_eq!(file, buffer.as_slice());
    /// # std::io::Result::Ok(())
    /// ```
    pub fn dump_graph<'a>(
        &self,
        writer: impl Write,
        format: GraphFormat,
        from_graph_name: impl Into<GraphNameRef<'a>>,
    ) -> Result<(), io::Error> {
        dump_graph(
            self.quads_for_pattern(None, None, None, Some(from_graph_name.into()))
                .map(|q| Ok(q?.into())),
            writer,
            format,
        )
    }

    /// Dumps the store into a file.
    ///    
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::DatasetFormat;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = Store::new()?;
    /// store.load_dataset(file, DatasetFormat::NQuads, None)?;
    ///
    /// let mut buffer = Vec::new();
    /// store.dump_dataset(&mut buffer, DatasetFormat::NQuads)?;
    /// assert_eq!(file, buffer.as_slice());
    /// # std::io::Result::Ok(())
    /// ```
    pub fn dump_dataset(&self, writer: impl Write, format: DatasetFormat) -> Result<(), io::Error> {
        dump_dataset(self.iter(), writer, format)
    }

    /// Returns all the store named graphs
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let store = Store::new()?;
    /// store.insert(QuadRef::new(&ex, &ex, &ex, &ex))?;
    /// store.insert(QuadRef::new(&ex, &ex, &ex, GraphNameRef::DefaultGraph))?;
    /// assert_eq!(vec![Subject::from(ex)], store.named_graphs().collect::<Result<Vec<_>,_>>()?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn named_graphs(&self) -> GraphNameIter {
        GraphNameIter {
            iter: self.storage.named_graphs(),
            store: self.clone(),
        }
    }

    /// Checks if the store contains a given graph
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::{NamedNode, QuadRef};
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let store = Store::new()?;
    /// store.insert(QuadRef::new(&ex, &ex, &ex, &ex))?;
    /// assert!(store.contains_named_graph(&ex)?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn contains_named_graph<'a>(
        &self,
        graph_name: impl Into<SubjectRef<'a>>,
    ) -> Result<bool, io::Error> {
        let graph_name = EncodedTerm::from(graph_name.into());
        self.storage.contains_named_graph(&graph_name)
    }

    /// Inserts a graph into this store
    ///
    /// Returns `true` if the graph was not already in the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::NamedNodeRef;
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let store = Store::new()?;
    /// store.insert_named_graph(ex)?;
    /// assert_eq!(store.named_graphs().count(), 1);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn insert_named_graph<'a>(
        &self,
        graph_name: impl Into<SubjectRef<'a>>,
    ) -> Result<bool, io::Error> {
        let graph_name = self.storage.encode_subject(graph_name.into())?;
        self.storage.insert_named_graph(&graph_name)
    }

    /// Clears a graph from this store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::{NamedNodeRef, QuadRef};
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let quad = QuadRef::new(ex, ex, ex, ex);
    /// let store = Store::new()?;
    /// store.insert(quad)?;
    /// assert_eq!(1, store.len());
    ///
    /// store.clear_graph(ex)?;
    /// assert_eq!(0, store.len());
    /// assert_eq!(1, store.named_graphs().count());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn clear_graph<'a>(
        &self,
        graph_name: impl Into<GraphNameRef<'a>>,
    ) -> Result<(), io::Error> {
        let graph_name = EncodedTerm::from(graph_name.into());
        self.storage.clear_graph(&graph_name)
    }

    /// Removes a graph from this store.
    ///
    /// Returns `true` if the graph was in the store and has been removed.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::{NamedNodeRef, QuadRef};
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let quad = QuadRef::new(ex, ex, ex, ex);
    /// let store = Store::new()?;
    /// store.insert(quad)?;
    /// assert_eq!(1, store.len());
    ///
    /// store.remove_named_graph(ex)?;
    /// assert!(store.is_empty());
    /// assert_eq!(0, store.named_graphs().count());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn remove_named_graph<'a>(
        &self,
        graph_name: impl Into<SubjectRef<'a>>,
    ) -> Result<bool, io::Error> {
        let graph_name = EncodedTerm::from(graph_name.into());
        self.storage.remove_named_graph(&graph_name)
    }

    /// Clears the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let store = Store::new()?;
    /// store.insert(QuadRef::new(ex, ex, ex, ex))?;
    /// store.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?;    
    /// assert_eq!(2, store.len());
    ///
    /// store.clear()?;
    /// assert!(store.is_empty());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn clear(&self) -> Result<(), io::Error> {
        self.storage.clear()
    }
}

impl fmt::Display for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self.iter() {
            writeln!(f, "{}", t.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

/// Allows inserting and deleting quads during an ACID transaction with the [`Store`].
pub struct Transaction<'a> {
    storage: StorageTransaction<'a>,
}

impl Transaction<'_> {
    /// Loads a graph file (i.e. triples) into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content might be temporarily stored in main memory.
    /// Do not use for big files.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::ConflictableTransactionError;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.transaction(|transaction| {
    ///     transaction.load_graph(file.as_ref(), GraphFormat::NTriples, &GraphName::DefaultGraph, None)?;
    ///     Ok(()) as Result<(),ConflictableTransactionError<std::io::Error>>
    /// })?;
    ///
    /// // we inspect the store content
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// If the file parsing fails in the middle of the file, the triples read before are still
    /// considered by the transaction. Rollback the transaction by making the transaction closure
    /// return an error if you don't want that.
    /// Moving up the parsing error through the transaction is enough to do that.
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    pub fn load_graph<'a>(
        &self,
        reader: impl BufRead,
        format: GraphFormat,
        to_graph_name: impl Into<GraphNameRef<'a>>,
        base_iri: Option<&str>,
    ) -> Result<(), UnabortableTransactionError> {
        load_graph(
            &self.storage,
            reader,
            format,
            to_graph_name.into(),
            base_iri,
        )?;
        Ok(())
    }

    /// Loads a dataset file (i.e. quads) into the store. into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content might be temporarily stored in main memory.
    /// Do not use for big files.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::{Store, ConflictableTransactionError};
    /// use oxigraph::io::DatasetFormat;
    /// use oxigraph::model::*;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store.transaction(|transaction| {
    ///     transaction.load_dataset(file.as_ref(), DatasetFormat::NQuads, None)?;
    ///     Ok(()) as Result<(),ConflictableTransactionError<std::io::Error>>
    /// })?;
    ///
    /// // we inspect the store content
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```    
    ///
    /// If the file parsing fails in the middle of the file, the quads read before are still
    /// considered by the transaction. Rollback the transaction by making the transaction closure
    /// return an error if you don't want that.
    /// Moving up the parsing error through the transaction is enough to do that.
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    pub fn load_dataset(
        &self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), UnabortableTransactionError> {
        Ok(load_dataset(&self.storage, reader, format, base_iri)?)
    }

    /// Adds a quad to this store during the transaction.
    ///
    /// Returns `true` if the quad was not already in the store.
    pub fn insert<'a>(
        &self,
        quad: impl Into<QuadRef<'a>>,
    ) -> Result<bool, UnabortableTransactionError> {
        let quad = self.storage.encode_quad(quad.into())?;
        self.storage.insert(&quad)
    }

    /// Removes a quad from this store during the transaction.
    ///
    /// Returns `true` if the quad was in the store and has been removed.
    pub fn remove<'a>(
        &self,
        quad: impl Into<QuadRef<'a>>,
    ) -> Result<bool, UnabortableTransactionError> {
        let quad = EncodedQuad::from(quad.into());
        self.storage.remove(&quad)
    }

    /// Inserts a graph into this store during the transaction
    ///
    /// Returns `true` if the graph was not already in the store.
    pub fn insert_named_graph<'a>(
        &self,
        graph_name: impl Into<SubjectRef<'a>>,
    ) -> Result<bool, UnabortableTransactionError> {
        let graph_name = self.storage.encode_subject(graph_name.into())?;
        self.storage.insert_named_graph(&graph_name)
    }
}

/// An iterator returning the quads contained in a [`Store`].
pub struct QuadIter {
    iter: ChainedDecodingQuadIterator,
    storage: Storage,
}

impl Iterator for QuadIter {
    type Item = Result<Quad, io::Error>;

    fn next(&mut self) -> Option<Result<Quad, io::Error>> {
        Some(match self.iter.next()? {
            Ok(quad) => self.storage.decode_quad(&quad).map_err(|e| e.into()),
            Err(error) => Err(error),
        })
    }
}

/// An iterator returning the graph names contained in a [`Store`].
pub struct GraphNameIter {
    iter: DecodingGraphIterator,
    store: Store,
}

impl Iterator for GraphNameIter {
    type Item = Result<Subject, io::Error>;

    fn next(&mut self) -> Option<Result<Subject, io::Error>> {
        Some(
            self.iter
                .next()?
                .and_then(|graph_name| Ok(self.store.storage.decode_subject(&graph_name)?)),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

#[test]
fn store() -> Result<(), io::Error> {
    use crate::model::*;

    let main_s = Subject::from(BlankNode::default());
    let main_p = NamedNode::new("http://example.com").unwrap();
    let main_o = Term::from(Literal::from(1));
    let main_g = GraphName::from(BlankNode::default());

    let default_quad = Quad::new(
        main_s.clone(),
        main_p.clone(),
        main_o.clone(),
        GraphName::DefaultGraph,
    );
    let named_quad = Quad::new(
        main_s.clone(),
        main_p.clone(),
        main_o.clone(),
        main_g.clone(),
    );
    let default_quads = vec![
        Quad::new(
            main_s.clone(),
            main_p.clone(),
            Literal::from(0),
            GraphName::DefaultGraph,
        ),
        default_quad.clone(),
        Quad::new(
            main_s.clone(),
            main_p.clone(),
            Literal::from(200000000),
            GraphName::DefaultGraph,
        ),
    ];
    let all_quads = vec![
        Quad::new(
            main_s.clone(),
            main_p.clone(),
            Literal::from(0),
            GraphName::DefaultGraph,
        ),
        default_quad.clone(),
        Quad::new(
            main_s.clone(),
            main_p.clone(),
            Literal::from(200000000),
            GraphName::DefaultGraph,
        ),
        named_quad.clone(),
    ];

    let store = Store::new()?;
    for t in &default_quads {
        assert!(store.insert(t)?);
    }

    let result: Result<_, TransactionError<io::Error>> = store.transaction(|t| {
        assert!(t.remove(&default_quad)?);
        assert_eq!(t.remove(&default_quad)?, false);
        assert!(t.insert(&named_quad)?);
        assert_eq!(t.insert(&named_quad)?, false);
        assert!(t.insert(&default_quad)?);
        assert_eq!(t.insert(&default_quad)?, false);
        Ok(())
    });
    result?;
    assert_eq!(store.insert(&default_quad)?, false);

    assert_eq!(store.len(), 4);
    assert_eq!(store.iter().collect::<Result<Vec<_>, _>>()?, all_quads);
    assert_eq!(
        store
            .quads_for_pattern(Some(main_s.as_ref()), None, None, None)
            .collect::<Result<Vec<_>, _>>()?,
        all_quads
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(main_s.as_ref()), Some(main_p.as_ref()), None, None)
            .collect::<Result<Vec<_>, _>>()?,
        all_quads
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                Some(main_p.as_ref()),
                Some(main_o.as_ref()),
                None
            )
            .collect::<Result<Vec<_>, _>>()?,
        vec![default_quad.clone(), named_quad.clone()]
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                Some(main_p.as_ref()),
                Some(main_o.as_ref()),
                Some(GraphNameRef::DefaultGraph)
            )
            .collect::<Result<Vec<_>, _>>()?,
        vec![default_quad.clone()]
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                Some(main_p.as_ref()),
                Some(main_o.as_ref()),
                Some(main_g.as_ref())
            )
            .collect::<Result<Vec<_>, _>>()?,
        vec![named_quad.clone()]
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                Some(main_p.as_ref()),
                None,
                Some(GraphNameRef::DefaultGraph)
            )
            .collect::<Result<Vec<_>, _>>()?,
        default_quads
    );
    assert_eq!(
        store
            .quads_for_pattern(Some(main_s.as_ref()), None, Some(main_o.as_ref()), None)
            .collect::<Result<Vec<_>, _>>()?,
        vec![default_quad.clone(), named_quad.clone()]
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                None,
                Some(main_o.as_ref()),
                Some(GraphNameRef::DefaultGraph)
            )
            .collect::<Result<Vec<_>, _>>()?,
        vec![default_quad.clone()]
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                None,
                Some(main_o.as_ref()),
                Some(main_g.as_ref())
            )
            .collect::<Result<Vec<_>, _>>()?,
        vec![named_quad.clone()]
    );
    assert_eq!(
        store
            .quads_for_pattern(
                Some(main_s.as_ref()),
                None,
                None,
                Some(GraphNameRef::DefaultGraph)
            )
            .collect::<Result<Vec<_>, _>>()?,
        default_quads
    );
    assert_eq!(
        store
            .quads_for_pattern(None, Some(main_p.as_ref()), None, None)
            .collect::<Result<Vec<_>, _>>()?,
        all_quads
    );
    assert_eq!(
        store
            .quads_for_pattern(None, Some(main_p.as_ref()), Some(main_o.as_ref()), None)
            .collect::<Result<Vec<_>, _>>()?,
        vec![default_quad.clone(), named_quad.clone()]
    );
    assert_eq!(
        store
            .quads_for_pattern(None, None, Some(main_o.as_ref()), None)
            .collect::<Result<Vec<_>, _>>()?,
        vec![default_quad.clone(), named_quad.clone()]
    );
    assert_eq!(
        store
            .quads_for_pattern(None, None, None, Some(GraphNameRef::DefaultGraph))
            .collect::<Result<Vec<_>, _>>()?,
        default_quads
    );
    assert_eq!(
        store
            .quads_for_pattern(
                None,
                Some(main_p.as_ref()),
                Some(main_o.as_ref()),
                Some(GraphNameRef::DefaultGraph)
            )
            .collect::<Result<Vec<_>, _>>()?,
        vec![default_quad]
    );
    assert_eq!(
        store
            .quads_for_pattern(
                None,
                Some(main_p.as_ref()),
                Some(main_o.as_ref()),
                Some(main_g.as_ref())
            )
            .collect::<Result<Vec<_>, _>>()?,
        vec![named_quad]
    );

    Ok(())
}
