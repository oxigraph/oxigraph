//! API to access an on-disk [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
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
use crate::io::read::ParseError;
use crate::io::{
    DatasetFormat, DatasetParser, DatasetSerializer, GraphFormat, GraphParser, GraphSerializer,
};
use crate::model::*;
use crate::sparql::{
    evaluate_query, evaluate_update, EvaluationError, Query, QueryOptions, QueryResults, Update,
    UpdateOptions,
};
#[cfg(not(target_arch = "wasm32"))]
use crate::storage::bulk_load;
use crate::storage::numeric_encoder::{Decoder, EncodedQuad, EncodedTerm};
use crate::storage::{ChainedDecodingQuadIterator, DecodingGraphIterator, Storage, StorageReader};
pub use crate::storage::{CorruptionError, LoaderError, SerializerError, StorageError};
use std::io::{BufRead, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::{fmt, str};

/// An on-disk [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
/// Allows to query and update it using SPARQL.
/// It is based on the [RocksDB](https://rocksdb.org/) key-value store.
///
/// This store ensure the "repeatable read" isolation level: the store only exposes changes that have
/// been "committed" (i.e. no partial writes) and the exposed state does not change for the complete duration
/// of a read operation (e.g. a SPARQL query) or a read/write operation (e.g. a SPARQL update).
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
    /// Creates a temporary [`Store`] that will be deleted after drop.
    pub fn new() -> Result<Self, StorageError> {
        Ok(Self {
            storage: Storage::new()?,
        })
    }

    /// Opens a [`Store`] and creates it if it does not exist yet.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
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
    ///
    ///
    /// Usage example with a custom function serializing terms to N-Triples:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryOptions, QueryResults};
    ///
    /// let store = Store::new()?;
    /// if let QueryResults::Solutions(mut solutions) = store.query_opt(
    ///     "SELECT (<http://www.w3.org/ns/formats/N-Triples>(1) AS ?nt) WHERE {}",
    ///     QueryOptions::default().with_custom_function(
    ///         NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?,
    ///         |args| args.get(0).map(|t| Literal::from(t.to_string()).into())
    ///     )
    /// )? {
    ///     assert_eq!(solutions.next().unwrap()?.get("nt"), Some(&Literal::from("\"1\"^^<http://www.w3.org/2001/XMLSchema#integer>").into()));
    /// }
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
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
        let reader = self.storage.snapshot();
        QuadIter {
            iter: reader.quads_for_pattern(
                subject.map(EncodedTerm::from).as_ref(),
                predicate.map(EncodedTerm::from).as_ref(),
                object.map(EncodedTerm::from).as_ref(),
                graph_name.map(EncodedTerm::from).as_ref(),
            ),
            reader,
        }
    }

    /// Returns all the quads contained in the store.
    pub fn iter(&self) -> QuadIter {
        self.quads_for_pattern(None, None, None, None)
    }

    /// Checks if this store contains a given quad.
    pub fn contains<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, StorageError> {
        let quad = EncodedQuad::from(quad.into());
        self.storage.snapshot().contains(&quad)
    }

    /// Returns the number of quads in the store.
    ///
    /// Warning: this function executes a full scan.
    pub fn len(&self) -> Result<usize, StorageError> {
        self.storage.snapshot().len()
    }

    /// Returns if the store is empty.
    pub fn is_empty(&self) -> Result<bool, StorageError> {
        self.storage.snapshot().is_empty()
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
    ///
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::QueryOptions;
    ///
    /// let store = Store::new()?;
    /// store.update_opt(
    ///     "INSERT { ?s <http://example.com/n-triples-representation> ?n } WHERE { ?s ?p ?o BIND(<http://www.w3.org/ns/formats/N-Triples>(?s) AS ?nt) }",
    ///     QueryOptions::default().with_custom_function(
    ///         NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?,
    ///         |args| args.get(0).map(|t| Literal::from(t.to_string()).into())
    ///     )
    /// )?;
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn update_opt(
        &self,
        update: impl TryInto<Update, Error = impl Into<EvaluationError>>,
        options: impl Into<UpdateOptions>,
    ) -> Result<(), EvaluationError> {
        evaluate_update(
            &self.storage,
            update.try_into().map_err(std::convert::Into::into)?,
            options.into(),
        )
    }

    /// Loads a graph file (i.e. triples) into the store.
    ///
    /// This function is atomic and quite slow and memory hungry. To get much better performances you might want to use [`bulk_load_graph`](Store::bulk_load_graph).
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
    ) -> Result<(), LoaderError> {
        let mut parser = GraphParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| ParseError::invalid_base_iri(base_iri, e))?;
        }
        let quads = parser
            .read_triples(reader)?
            .collect::<Result<Vec<_>, _>>()?;
        let to_graph_name = to_graph_name.into();
        self.storage.transaction(move |mut t| {
            for quad in &quads {
                t.insert(quad.as_ref().in_graph(to_graph_name))?;
            }
            Ok(())
        })
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// This function is atomic and quite slow. To get much better performances you might want to [`bulk_load_dataset`](Store::bulk_load_dataset).
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
    ) -> Result<(), LoaderError> {
        let mut parser = DatasetParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| ParseError::invalid_base_iri(base_iri, e))?;
        }
        let quads = parser.read_quads(reader)?.collect::<Result<Vec<_>, _>>()?;
        self.storage.transaction(move |mut t| {
            for quad in &quads {
                t.insert(quad.into())?;
            }
            Ok(())
        })
    }

    /// Adds a quad to this store.
    ///
    /// Returns `true` if the quad was not already in the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let quad = QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph);
    ///
    /// let store = Store::new()?;
    /// store.insert(quad)?;
    ///
    /// assert!(store.contains(quad)?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn insert<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, StorageError> {
        let quad = quad.into();
        self.storage.transaction(move |mut t| t.insert(quad))
    }

    /// Adds atomically a set of quads to this store.
    ///
    /// Warning: This operation uses a memory heavy transaction internally, use [`bulk_extend`](Store::bulk_extend) if you plan to add ten of millions of triples.
    pub fn extend(&self, quads: impl IntoIterator<Item = Quad>) -> Result<(), StorageError> {
        let quads = quads.into_iter().collect::<Vec<_>>();
        self.storage.transaction(move |mut t| {
            for quad in &quads {
                t.insert(quad.into())?;
            }
            Ok(())
        })
    }

    /// Removes a quad from this store.
    ///
    /// Returns `true` if the quad was in the store and has been removed.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let quad = QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph);
    ///
    /// let store = Store::new()?;
    /// store.insert(quad)?;
    /// store.remove(quad)?;
    ///
    /// assert!(!store.contains(quad)?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn remove<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, StorageError> {
        let quad = quad.into();
        self.storage.transaction(move |mut t| t.remove(quad))
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
    ) -> Result<(), SerializerError> {
        let mut writer = GraphSerializer::from_format(format).triple_writer(writer)?;
        for quad in self.quads_for_pattern(None, None, None, Some(from_graph_name.into())) {
            writer.write(quad?.as_ref())?;
        }
        writer.finish()?;
        Ok(())
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
    pub fn dump_dataset(
        &self,
        writer: impl Write,
        format: DatasetFormat,
    ) -> Result<(), SerializerError> {
        let mut writer = DatasetSerializer::from_format(format).quad_writer(writer)?;
        for quad in self.iter() {
            writer.write(&quad?)?;
        }
        writer.finish()?;
        Ok(())
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
    /// assert_eq!(vec![NamedOrBlankNode::from(ex)], store.named_graphs().collect::<Result<Vec<_>,_>>()?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn named_graphs(&self) -> GraphNameIter {
        let reader = self.storage.snapshot();
        GraphNameIter {
            iter: reader.named_graphs(),
            reader,
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
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Result<bool, StorageError> {
        let graph_name = EncodedTerm::from(graph_name.into());
        self.storage.snapshot().contains_named_graph(&graph_name)
    }

    /// Inserts a graph into this store.
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
    ///
    /// assert_eq!(store.named_graphs().collect::<Result<Vec<_>,_>>()?, vec![ex.into_owned().into()]);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn insert_named_graph<'a>(
        &self,
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Result<bool, StorageError> {
        let graph_name = graph_name.into();
        self.storage
            .transaction(move |mut t| t.insert_named_graph(graph_name))
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
    /// assert_eq!(1, store.len()?);
    ///
    /// store.clear_graph(ex)?;
    /// assert!(store.is_empty()?);
    /// assert_eq!(1, store.named_graphs().count());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn clear_graph<'a>(
        &self,
        graph_name: impl Into<GraphNameRef<'a>>,
    ) -> Result<(), StorageError> {
        let graph_name = graph_name.into();
        self.storage
            .transaction(move |mut t| t.clear_graph(graph_name))
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
    /// assert_eq!(1, store.len()?);
    ///
    /// store.remove_named_graph(ex)?;
    /// assert!(store.is_empty()?);
    /// assert_eq!(0, store.named_graphs().count());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn remove_named_graph<'a>(
        &self,
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Result<bool, StorageError> {
        let graph_name = graph_name.into();
        self.storage
            .transaction(move |mut t| t.remove_named_graph(graph_name))
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
    /// assert_eq!(2, store.len()?);
    ///
    /// store.clear()?;
    /// assert!(store.is_empty()?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn clear(&self) -> Result<(), StorageError> {
        self.storage.transaction(|mut t| t.clear())
    }

    /// Flushes all buffers and ensures that all writes are saved on disk.
    ///
    /// Flushes are automatically done using background threads but might lag a little bit.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn flush(&self) -> Result<(), StorageError> {
        self.storage.flush()
    }

    /// Optimizes the database for future workload.
    ///
    /// Useful to call after a batch upload or an other similar operation.
    ///
    /// Warning: Can take hours on huge databases.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn optimize(&self) -> Result<(), StorageError> {
        self.storage.compact()
    }

    /// Loads a dataset file efficiently into the store.
    ///
    /// This function is optimized for large dataset loading speed. For small files, [`load_dataset`](Store::load_dataset) might be more convenient.
    ///
    /// Warning: This method is not atomic.
    /// If the parsing fails in the middle of the file, only a part of it may be written to the store.
    /// Results might get weird if you delete data during the loading process.
    ///
    /// Warning: This method is optimized for speed. It uses multiple threads and multiple GBs of RAM on large files.
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
    /// store.bulk_load_dataset(file.as_ref(), DatasetFormat::NQuads, None)?;
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
    #[cfg(not(target_arch = "wasm32"))]
    pub fn bulk_load_dataset(
        &self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), LoaderError> {
        let mut parser = DatasetParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| ParseError::invalid_base_iri(base_iri, e))?;
        }
        bulk_load(&self.storage, parser.read_quads(reader)?)
    }

    /// Loads a dataset file efficiently into the store.
    ///
    /// This function is optimized for large dataset loading speed. For small files, [`load_graph`](Store::load_graph) might be more convenient.   
    ///
    /// Warning: This method is not atomic.
    /// If the parsing fails in the middle of the file, only a part of it may be written to the store.
    /// Results might get weird if you delete data during the loading process.
    ///
    /// Warning: This method is optimized for speed. It uses multiple threads and multiple GBs of RAM on large files.
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
    /// store.bulk_load_graph(file.as_ref(), GraphFormat::NTriples, &GraphName::DefaultGraph, None)?;
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
    #[cfg(not(target_arch = "wasm32"))]
    pub fn bulk_load_graph<'a>(
        &self,
        reader: impl BufRead,
        format: GraphFormat,
        to_graph_name: impl Into<GraphNameRef<'a>>,
        base_iri: Option<&str>,
    ) -> Result<(), LoaderError> {
        let mut parser = GraphParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| ParseError::invalid_base_iri(base_iri, e))?;
        }
        let to_graph_name = to_graph_name.into();
        bulk_load(
            &self.storage,
            parser
                .read_triples(reader)?
                .map(|r| r.map(|q| q.in_graph(to_graph_name.into_owned()))),
        )
    }

    /// Adds a set of triples to this store using bulk load.
    ///
    /// Warning: This method is not atomic.
    /// If the process fails in the middle of the file, only a part of the data may be written to the store.
    /// Results might get weird if you delete data during the loading process.
    ///
    /// Warning: This method is optimized for speed. It uses multiple threads and multiple GBs of RAM on large files.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn bulk_extend(&self, quads: impl IntoIterator<Item = Quad>) -> Result<(), StorageError> {
        bulk_load::<StorageError, _, _>(&self.storage, quads.into_iter().map(Ok))
    }
}

impl fmt::Display for Store {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self.iter() {
            writeln!(f, "{} .", t.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

/// An iterator returning the quads contained in a [`Store`].
pub struct QuadIter {
    iter: ChainedDecodingQuadIterator,
    reader: StorageReader,
}

impl Iterator for QuadIter {
    type Item = Result<Quad, StorageError>;

    fn next(&mut self) -> Option<Result<Quad, StorageError>> {
        Some(match self.iter.next()? {
            Ok(quad) => self.reader.decode_quad(&quad),
            Err(error) => Err(error),
        })
    }
}

/// An iterator returning the graph names contained in a [`Store`].
pub struct GraphNameIter {
    iter: DecodingGraphIterator,
    reader: StorageReader,
}

impl Iterator for GraphNameIter {
    type Item = Result<NamedOrBlankNode, StorageError>;

    fn next(&mut self) -> Option<Result<NamedOrBlankNode, StorageError>> {
        Some(
            self.iter
                .next()?
                .and_then(|graph_name| self.reader.decode_named_or_blank_node(&graph_name)),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

#[test]
fn store() -> Result<(), StorageError> {
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
            Literal::from(200_000_000),
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
            Literal::from(200_000_000),
            GraphName::DefaultGraph,
        ),
        named_quad.clone(),
    ];

    let store = Store::new()?;
    for t in &default_quads {
        assert!(store.insert(t)?);
    }
    assert!(!store.insert(&default_quad)?);

    assert!(store.remove(&default_quad)?);
    assert!(!store.remove(&default_quad)?);
    assert!(store.insert(&named_quad)?);
    assert!(!store.insert(&named_quad)?);
    assert!(store.insert(&default_quad)?);
    assert!(!store.insert(&default_quad)?);

    assert_eq!(store.len()?, 4);
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
