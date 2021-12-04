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
use crate::error::invalid_input_error;
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
use std::io::{BufRead, Write};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::{fmt, io, str};

/// An on-on disk [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
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
    /// Creates a temporary [`Store`]() that will be deleted after drop.
    pub fn new() -> io::Result<Self> {
        Ok(Self {
            storage: Storage::new()?,
        })
    }

    /// Opens a [`Store`]() and creates it if it does not exist yet.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
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

    /// Returns all the quads contained in the store
    pub fn iter(&self) -> QuadIter {
        self.quads_for_pattern(None, None, None, None)
    }

    /// Checks if this store contains a given quad
    pub fn contains<'a>(&self, quad: impl Into<QuadRef<'a>>) -> io::Result<bool> {
        let quad = EncodedQuad::from(quad.into());
        self.storage.snapshot().contains(&quad)
    }

    /// Returns the number of quads in the store
    ///
    /// Warning: this function executes a full scan
    pub fn len(&self) -> io::Result<usize> {
        self.storage.snapshot().len()
    }

    /// Returns if the store is empty
    pub fn is_empty(&self) -> io::Result<bool> {
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
    pub fn update_opt(
        &self,
        update: impl TryInto<Update, Error = impl Into<EvaluationError>>,
        options: UpdateOptions,
    ) -> Result<(), EvaluationError> {
        evaluate_update(
            &self.storage,
            update.try_into().map_err(std::convert::Into::into)?,
            options,
        )
    }

    /// Loads a graph file (i.e. triples) into the store.
    ///
    /// This function is atomic and quite slow and memory hungry. To get much better performances you might want to use [`bulk_load_graph`].
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
    ) -> io::Result<()> {
        let mut parser = GraphParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(invalid_input_error)?;
        }
        let quads = parser
            .read_triples(reader)?
            .collect::<io::Result<Vec<_>>>()?;
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
    /// This function is atomic and quite slow. To get much better performances you might want to [`bulk_load_dataset`].
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
    ) -> io::Result<()> {
        let mut parser = DatasetParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(invalid_input_error)?;
        }
        let quads = parser.read_quads(reader)?.collect::<io::Result<Vec<_>>>()?;
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
    pub fn insert<'a>(&self, quad: impl Into<QuadRef<'a>>) -> io::Result<bool> {
        let quad = quad.into();
        self.storage.transaction(move |mut t| t.insert(quad))
    }

    /// Adds atomically a set of quads to this store.
    ///
    /// Warning: This operation uses a memory heavy transaction internally, use [`bulk_extend`] if you plan to add ten of millions of triples.
    pub fn extend(&self, quads: impl IntoIterator<Item = Quad>) -> io::Result<()> {
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
    pub fn remove<'a>(&self, quad: impl Into<QuadRef<'a>>) -> io::Result<bool> {
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
    ) -> io::Result<()> {
        let mut writer = GraphSerializer::from_format(format).triple_writer(writer)?;
        for quad in self.quads_for_pattern(None, None, None, Some(from_graph_name.into())) {
            writer.write(quad?.as_ref())?;
        }
        writer.finish()
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
    pub fn dump_dataset(&self, writer: impl Write, format: DatasetFormat) -> io::Result<()> {
        let mut writer = DatasetSerializer::from_format(format).quad_writer(writer)?;
        for quad in self.iter() {
            writer.write(&quad?)?;
        }
        writer.finish()
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
    ) -> io::Result<bool> {
        let graph_name = EncodedTerm::from(graph_name.into());
        self.storage.snapshot().contains_named_graph(&graph_name)
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
    ///
    /// assert_eq!(store.named_graphs().collect::<Result<Vec<_>,_>>()?, vec![ex.into_owned().into()]);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn insert_named_graph<'a>(
        &self,
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> io::Result<bool> {
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
    pub fn clear_graph<'a>(&self, graph_name: impl Into<GraphNameRef<'a>>) -> io::Result<()> {
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
    ) -> io::Result<bool> {
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
    pub fn clear(&self) -> io::Result<()> {
        self.storage.transaction(|mut t| t.clear())
    }

    /// Flushes all buffers and ensures that all writes are saved on disk.
    ///
    /// Flushes are automatically done for most platform using background threads.
    /// However, calling this method explicitly is still required for Windows and Android.
    ///
    /// An [async version](SledStore::flush_async) is also available.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn flush(&self) -> io::Result<()> {
        self.storage.flush()
    }

    /// Optimizes the database for future workload.
    ///
    /// Useful to call after a batch upload or an other similar operation.
    ///
    /// Warning: Can take hours on huge databases.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn optimize(&self) -> io::Result<()> {
        self.storage.compact()
    }

    /// Loads a dataset file efficiently into the store.
    ///
    /// This function is optimized for large dataset loading speed. For small files, [`load_dataset`] might be more convenient.
    ///
    /// Warning: This method is not atomic. If the parsing fails in the middle of the file, only a part of it may be written to the store.
    ///
    /// Warning: This method is optimized for speed. It uses multiple threads and multiple GBs of RAM on large files.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::DatasetFormat;
    /// use oxigraph::model::*;
    ///
    /// let mut store = Store::new()?;
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
        &mut self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> io::Result<()> {
        let mut parser = DatasetParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(invalid_input_error)?;
        }
        bulk_load(&self.storage, parser.read_quads(reader)?)
    }

    /// Loads a dataset file efficiently into the store.
    ///
    /// This function is optimized for large dataset loading speed. For small files, [`load_graph`] might be more convenient.   
    ///
    /// Warning: This method is not atomic. If the parsing fails in the middle of the file, only a part of it may be written to the store.
    ///
    /// Warning: This method is optimized for speed. It uses multiple threads and multiple GBs of RAM on large files.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::*;
    ///
    /// let mut store = Store::new()?;
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
        &mut self,
        reader: impl BufRead,
        format: GraphFormat,
        to_graph_name: impl Into<GraphNameRef<'a>>,
        base_iri: Option<&str>,
    ) -> io::Result<()> {
        let mut parser = GraphParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(invalid_input_error)?;
        }
        let to_graph_name = to_graph_name.into();
        bulk_load(
            &self.storage,
            parser
                .read_triples(reader)?
                .map(|r| Ok(r?.in_graph(to_graph_name.into_owned()))),
        )
    }

    /// Adds a set of triples to this store using bulk load.
    ///
    /// Warning: This method is not atomic. If the parsing fails in the middle of the file, only a part of it may be written to the store.
    ///
    /// Warning: This method is optimized for speed. It uses multiple threads and multiple GBs of RAM on large files.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn bulk_extend(&mut self, quads: impl IntoIterator<Item = Quad>) -> io::Result<()> {
        bulk_load(&self.storage, quads.into_iter().map(Ok))
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
    type Item = io::Result<Quad>;

    fn next(&mut self) -> Option<io::Result<Quad>> {
        Some(match self.iter.next()? {
            Ok(quad) => self.reader.decode_quad(&quad).map_err(|e| e.into()),
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
    type Item = io::Result<NamedOrBlankNode>;

    fn next(&mut self) -> Option<io::Result<NamedOrBlankNode>> {
        Some(
            self.iter
                .next()?
                .and_then(|graph_name| Ok(self.reader.decode_named_or_blank_node(&graph_name)?)),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

#[test]
fn store() -> io::Result<()> {
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
