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
//! # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
use crate::storage::numeric_encoder::{Decoder, EncodedQuad, EncodedTerm};
#[cfg(not(target_family = "wasm"))]
use crate::storage::StorageBulkLoader;
use crate::storage::{
    ChainedDecodingQuadIterator, DecodingGraphIterator, Storage, StorageReader, StorageWriter,
};
pub use crate::storage::{CorruptionError, LoaderError, SerializerError, StorageError};
use std::error::Error;
use std::io::{BufRead, Write};
#[cfg(not(target_family = "wasm"))]
use std::path::Path;
use std::{fmt, str};

/// An on-disk [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
/// Allows to query and update it using SPARQL.
/// It is based on the [RocksDB](https://rocksdb.org/) key-value store.
///
/// This store ensures the "repeatable read" isolation level: the store only exposes changes that have
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
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone)]
pub struct Store {
    storage: Storage,
}

impl Store {
    /// Creates a temporary [`Store`] that will be deleted after drop.
    pub fn new() -> Result<Self, StorageError> {
        Ok(Self {
            storage: Storage::new()?,
        })
    }

    /// Opens a [`Store`] and creates it if it does not exist yet.
    #[cfg(not(target_family = "wasm"))]
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn query(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
    ) -> Result<QueryResults, EvaluationError> {
        self.query_opt(query, QueryOptions::default())
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) with some options.
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn query_opt(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
    ) -> Result<QueryResults, EvaluationError> {
        evaluate_query(self.storage.snapshot(), query, options)
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
    /// let results = store.iter().collect::<Result<Vec<_>,_>>()?;
    /// assert_eq!(vec![quad], results);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn iter(&self) -> QuadIter {
        self.quads_for_pattern(None, None, None, None)
    }

    /// Checks if this store contains a given quad.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let quad = QuadRef::new(ex, ex, ex, ex);
    ///
    /// let store = Store::new()?;
    /// assert!(!store.contains(quad)?);
    ///
    /// store.insert(quad)?;
    /// assert!(store.contains(quad)?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn contains<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, StorageError> {
        let quad = EncodedQuad::from(quad.into());
        self.storage.snapshot().contains(&quad)
    }

    /// Returns the number of quads in the store.
    ///
    /// Warning: this function executes a full scan.
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn len(&self) -> Result<usize, StorageError> {
        self.storage.snapshot().len()
    }

    /// Returns if the store is empty.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    ///
    /// let store = Store::new()?;
    /// assert!(store.is_empty()?);
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// store.insert(QuadRef::new(ex, ex, ex, ex))?;
    /// assert!(!store.is_empty()?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn is_empty(&self) -> Result<bool, StorageError> {
        self.storage.snapshot().is_empty()
    }

    /// Executes a transaction.
    ///
    /// Transactions ensure the "repeatable read" isolation level: the store only exposes changes that have
    /// been "committed" (i.e. no partial writes) and the exposed state does not change for the complete duration
    /// of a read operation (e.g. a SPARQL query) or a read/write operation (e.g. a SPARQL update).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::{StorageError, Store};
    /// use oxigraph::model::*;
    ///
    /// let store = Store::new()?;
    /// let a = NamedNodeRef::new("http://example.com/a")?;
    /// let b = NamedNodeRef::new("http://example.com/b")?;
    ///
    /// // Copy all triples about ex:a to triples about ex:b
    /// store.transaction(|mut transaction| {
    ///     for q in transaction.quads_for_pattern(Some(a.into()), None, None, None) {
    ///         let q = q?;
    ///         transaction.insert(QuadRef::new(b, &q.predicate, &q.object, &q.graph_name))?;
    ///     }
    ///     Result::<_, StorageError>::Ok(())
    /// })?;
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn transaction<'a, 'b: 'a, T, E: Error + 'static + From<StorageError>>(
        &'b self,
        f: impl Fn(Transaction<'a>) -> Result<T, E>,
    ) -> Result<T, E> {
        self.storage.transaction(|writer| f(Transaction { writer }))
    }

    /// Executes a [SPARQL 1.1 update](https://www.w3.org/TR/sparql11-update/).
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
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn update_opt(
        &self,
        update: impl TryInto<Update, Error = impl Into<EvaluationError>>,
        options: impl Into<UpdateOptions>,
    ) -> Result<(), EvaluationError> {
        let update = update.try_into().map_err(Into::into)?;
        let options = options.into();
        self.storage
            .transaction(|mut t| evaluate_update(&mut t, &update, &options))
    }

    /// Loads a graph file (i.e. triples) into the store.
    ///
    /// This function is atomic, quite slow and memory hungry. To get much better performances you might want to use the [`bulk_loader`](Store::bulk_loader).
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
    /// store.load_graph(file.as_ref(), GraphFormat::NTriples, GraphNameRef::DefaultGraph, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
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
    /// This function is atomic, quite slow and memory hungry. To get much better performances you might want to use the [`bulk_loader`](Store::bulk_loader).
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
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
    /// assert!(store.insert(quad)?);
    /// assert!(!store.insert(quad)?);
    ///
    /// assert!(store.contains(quad)?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn insert<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, StorageError> {
        let quad = quad.into();
        self.transaction(|mut t| t.insert(quad))
    }

    /// Adds atomically a set of quads to this store.
    ///
    /// Warning: This operation uses a memory heavy transaction internally, use the [`bulk_loader`](Store::bulk_loader) if you plan to add ten of millions of triples.
    pub fn extend(
        &self,
        quads: impl IntoIterator<Item = impl Into<Quad>>,
    ) -> Result<(), StorageError> {
        let quads = quads.into_iter().map(|q| q.into()).collect::<Vec<_>>();
        self.transaction(move |mut t| t.extend(&quads))
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
    /// assert!(store.remove(quad)?);
    /// assert!(!store.remove(quad)?);
    ///
    /// assert!(!store.contains(quad)?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn remove<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, StorageError> {
        let quad = quad.into();
        self.transaction(move |mut t| t.remove(quad))
    }

    /// Dumps a store graph into a file.
    ///    
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::GraphNameRef;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = Store::new()?;
    /// store.load_graph(file, GraphFormat::NTriples, GraphNameRef::DefaultGraph, None)?;
    ///
    /// let mut buffer = Vec::new();
    /// store.dump_graph(&mut buffer, GraphFormat::NTriples, GraphNameRef::DefaultGraph)?;
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

    /// Returns all the store named graphs.
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn insert_named_graph<'a>(
        &self,
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Result<bool, StorageError> {
        let graph_name = graph_name.into();
        self.transaction(|mut t| t.insert_named_graph(graph_name))
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn clear_graph<'a>(
        &self,
        graph_name: impl Into<GraphNameRef<'a>>,
    ) -> Result<(), StorageError> {
        let graph_name = graph_name.into();
        self.transaction(|mut t| t.clear_graph(graph_name))
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
    /// assert!(store.remove_named_graph(ex)?);
    /// assert!(store.is_empty()?);
    /// assert_eq!(0, store.named_graphs().count());
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn remove_named_graph<'a>(
        &self,
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Result<bool, StorageError> {
        let graph_name = graph_name.into();
        self.transaction(|mut t| t.remove_named_graph(graph_name))
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn clear(&self) -> Result<(), StorageError> {
        self.transaction(|mut t| t.clear())
    }

    /// Flushes all buffers and ensures that all writes are saved on disk.
    ///
    /// Flushes are automatically done using background threads but might lag a little bit.
    #[cfg(not(target_family = "wasm"))]
    pub fn flush(&self) -> Result<(), StorageError> {
        self.storage.flush()
    }

    /// Optimizes the database for future workload.
    ///
    /// Useful to call after a batch upload or another similar operation.
    ///
    /// Warning: Can take hours on huge databases.
    #[cfg(not(target_family = "wasm"))]
    pub fn optimize(&self) -> Result<(), StorageError> {
        self.storage.compact()
    }

    /// Creates database backup into the `target_directory`.
    ///
    /// After its creation, the backup is usable using [`Store::open`]
    /// like a regular Oxigraph database and operates independently from the original database.
    ///
    /// Warning: Backups are only possible for on-disk databases created using [`Store::open`].
    /// Temporary in-memory databases created using [`Store::new`] are not compatible with RocksDB backup system.
    ///
    /// Warning: An error is raised if the `target_directory` already exists.
    ///
    /// If the target directory is in the same file system as the current database,
    /// the database content will not be fully copied
    /// but hard links will be used to point to the original database immutable snapshots.
    /// This allows cheap regular backups.
    ///
    /// If you want to move your data to another RDF storage system, you should have a look at the [`Store::dump_dataset`] function instead.
    #[cfg(not(target_family = "wasm"))]
    pub fn backup(&self, target_directory: impl AsRef<Path>) -> Result<(), StorageError> {
        self.storage.backup(target_directory.as_ref())
    }

    /// Creates a bulk loader allowing to load at lot of data quickly into the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::DatasetFormat;
    /// use oxigraph::model::*;
    ///
    /// let store = Store::new()?;
    ///
    /// // quads file insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store.bulk_loader().load_dataset(file.as_ref(), DatasetFormat::NQuads, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[cfg(not(target_family = "wasm"))]
    pub fn bulk_loader(&self) -> BulkLoader {
        BulkLoader {
            storage: StorageBulkLoader::new(self.storage.clone()),
            on_parse_error: None,
        }
    }

    /// Validates that all the store invariants held in the data
    #[doc(hidden)]
    pub fn validate(&self) -> Result<(), StorageError> {
        self.storage.snapshot().validate()
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

/// An object to do operations during a transaction.
///
/// See [`Store::transaction`] for a more detailed description.
pub struct Transaction<'a> {
    writer: StorageWriter<'a>,
}

impl<'a> Transaction<'a> {
    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{EvaluationError, QueryResults};
    ///
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     if let QueryResults::Solutions(solutions) = transaction.query("SELECT ?s WHERE { ?s ?p ?o }")? {
    ///         for solution in solutions {
    ///             if let Some(Term::NamedNode(s)) =  solution?.get("s") {
    ///                 transaction.insert(QuadRef::new(s, vocab::rdf::TYPE, NamedNodeRef::new_unchecked("http://example.com"), GraphNameRef::DefaultGraph))?;
    ///             }
    ///         }
    ///     }
    ///     Result::<_, EvaluationError>::Ok(())
    /// })?;
    /// # Result::<_, EvaluationError>::Ok(())
    /// ```
    pub fn query(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
    ) -> Result<QueryResults, EvaluationError> {
        self.query_opt(query, QueryOptions::default())
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) with some options.
    ///
    /// Usage example with a custom function serializing terms to N-Triples:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{EvaluationError, QueryOptions, QueryResults};
    ///
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     if let QueryResults::Solutions(solutions) = transaction.query_opt(
    ///         "SELECT ?s (<http://www.w3.org/ns/formats/N-Triples>(?s) AS ?nt) WHERE { ?s ?p ?o }",
    ///         QueryOptions::default().with_custom_function(
    ///             NamedNode::new_unchecked("http://www.w3.org/ns/formats/N-Triples"),
    ///             |args| args.get(0).map(|t| Literal::from(t.to_string()).into())
    ///         )
    ///     )? {
    ///         for solution in solutions {
    ///             let solution = solution?;
    ///             if let (Some(Term::NamedNode(s)), Some(nt)) = (solution.get("s"), solution.get("nt")) {
    ///                 transaction.insert(QuadRef::new(s, NamedNodeRef::new_unchecked("http://example.com/n-triples-representation"), nt, GraphNameRef::DefaultGraph))?;
    ///             }
    ///         }
    ///     }
    ///     Result::<_, EvaluationError>::Ok(())
    /// })?;
    /// # Result::<_, EvaluationError>::Ok(())
    /// ```
    pub fn query_opt(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
    ) -> Result<QueryResults, EvaluationError> {
        evaluate_query(self.writer.reader(), query, options)
    }

    /// Retrieves quads with a filter on each quad component.
    ///
    /// Usage example:
    /// Usage example:
    /// ```
    /// use oxigraph::store::{StorageError, Store};
    /// use oxigraph::model::*;
    ///
    /// let store = Store::new()?;
    /// let a = NamedNodeRef::new("http://example.com/a")?;
    /// let b = NamedNodeRef::new("http://example.com/b")?;
    ///
    /// // Copy all triples about ex:a to triples about ex:b
    /// store.transaction(|mut transaction| {
    ///     for q in transaction.quads_for_pattern(Some(a.into()), None, None, None) {
    ///         let q = q?;
    ///         transaction.insert(QuadRef::new(b, &q.predicate, &q.object, &q.graph_name))?;
    ///     }
    ///     Result::<_, StorageError>::Ok(())
    /// })?;
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn quads_for_pattern(
        &self,
        subject: Option<SubjectRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
        graph_name: Option<GraphNameRef<'_>>,
    ) -> QuadIter {
        let reader = self.writer.reader();
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
    pub fn contains<'b>(&self, quad: impl Into<QuadRef<'b>>) -> Result<bool, StorageError> {
        let quad = EncodedQuad::from(quad.into());
        self.writer.reader().contains(&quad)
    }

    /// Returns the number of quads in the store.
    ///
    /// Warning: this function executes a full scan.
    pub fn len(&self) -> Result<usize, StorageError> {
        self.writer.reader().len()
    }

    /// Returns if the store is empty.
    pub fn is_empty(&self) -> Result<bool, StorageError> {
        self.writer.reader().is_empty()
    }

    /// Executes a [SPARQL 1.1 update](https://www.w3.org/TR/sparql11-update/).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::EvaluationError;
    ///
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     // insertion
    ///     transaction.update("INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }")?;
    ///
    ///     // we inspect the store contents
    ///     let ex = NamedNodeRef::new_unchecked("http://example.com");
    ///     assert!(transaction.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    ///     Result::<_, EvaluationError>::Ok(())
    /// })?;
    /// # Result::<_, EvaluationError>::Ok(())
    /// ```
    pub fn update(
        &mut self,
        update: impl TryInto<Update, Error = impl Into<EvaluationError>>,
    ) -> Result<(), EvaluationError> {
        self.update_opt(update, UpdateOptions::default())
    }

    /// Executes a [SPARQL 1.1 update](https://www.w3.org/TR/sparql11-update/) with some options.
    pub fn update_opt(
        &mut self,
        update: impl TryInto<Update, Error = impl Into<EvaluationError>>,
        options: impl Into<UpdateOptions>,
    ) -> Result<(), EvaluationError> {
        evaluate_update(
            &mut self.writer,
            &update.try_into().map_err(Into::into)?,
            &options.into(),
        )
    }

    /// Loads a graph file (i.e. triples) into the store.
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
    /// store.transaction(|mut transaction| {
    ///     transaction.load_graph(file.as_ref(), GraphFormat::NTriples, GraphNameRef::DefaultGraph, None)
    /// })?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    /// # Result::<_,oxigraph::store::LoaderError>::Ok(())
    /// ```
    pub fn load_graph<'b>(
        &mut self,
        reader: impl BufRead,
        format: GraphFormat,
        to_graph_name: impl Into<GraphNameRef<'b>>,
        base_iri: Option<&str>,
    ) -> Result<(), LoaderError> {
        let mut parser = GraphParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| ParseError::invalid_base_iri(base_iri, e))?;
        }
        let to_graph_name = to_graph_name.into();
        for triple in parser.read_triples(reader)? {
            self.writer
                .insert(triple?.as_ref().in_graph(to_graph_name))?;
        }
        Ok(())
    }

    /// Loads a dataset file (i.e. quads) into the store.
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
    /// store.transaction(|mut transaction| {
    ///     transaction.load_dataset(file.as_ref(), DatasetFormat::NQuads, None)
    /// })?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
    /// # Result::<_,oxigraph::store::LoaderError>::Ok(())
    /// ```
    pub fn load_dataset(
        &mut self,
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
        for quad in parser.read_quads(reader)? {
            self.writer.insert(quad?.as_ref())?;
        }
        Ok(())
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
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// let quad = QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph);
    ///
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     transaction.insert(quad)
    /// })?;
    /// assert!(store.contains(quad)?);
    /// # Result::<_,oxigraph::store::StorageError>::Ok(())
    /// ```
    pub fn insert<'b>(&mut self, quad: impl Into<QuadRef<'b>>) -> Result<bool, StorageError> {
        self.writer.insert(quad.into())
    }

    /// Adds a set of quads to this store.
    pub fn extend<'b>(
        &mut self,
        quads: impl IntoIterator<Item = impl Into<QuadRef<'b>>>,
    ) -> Result<(), StorageError> {
        for quad in quads {
            self.writer.insert(quad.into())?;
        }
        Ok(())
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
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// let quad = QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph);
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     transaction.insert(quad)?;
    ///     transaction.remove(quad)
    /// })?;
    /// assert!(!store.contains(quad)?);
    /// # Result::<_,oxigraph::store::StorageError>::Ok(())
    /// ```
    pub fn remove<'b>(&mut self, quad: impl Into<QuadRef<'b>>) -> Result<bool, StorageError> {
        self.writer.remove(quad.into())
    }

    /// Returns all the store named graphs.
    pub fn named_graphs(&self) -> GraphNameIter {
        let reader = self.writer.reader();
        GraphNameIter {
            iter: reader.named_graphs(),
            reader,
        }
    }

    /// Checks if the store contains a given graph.
    pub fn contains_named_graph<'b>(
        &self,
        graph_name: impl Into<NamedOrBlankNodeRef<'b>>,
    ) -> Result<bool, StorageError> {
        self.writer
            .reader()
            .contains_named_graph(&EncodedTerm::from(graph_name.into()))
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
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     transaction.insert_named_graph(ex)
    /// })?;
    /// assert_eq!(store.named_graphs().collect::<Result<Vec<_>,_>>()?, vec![ex.into_owned().into()]);
    /// # Result::<_,oxigraph::store::StorageError>::Ok(())
    /// ```
    pub fn insert_named_graph<'b>(
        &mut self,
        graph_name: impl Into<NamedOrBlankNodeRef<'b>>,
    ) -> Result<bool, StorageError> {
        self.writer.insert_named_graph(graph_name.into())
    }

    /// Clears a graph from this store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::{NamedNodeRef, QuadRef};
    ///
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// let quad = QuadRef::new(ex, ex, ex, ex);
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     transaction.insert(quad)?;
    ///     transaction.clear_graph(ex)
    /// })?;
    /// assert!(store.is_empty()?);
    /// assert_eq!(1, store.named_graphs().count());
    /// # Result::<_,oxigraph::store::StorageError>::Ok(())
    /// ```
    pub fn clear_graph<'b>(
        &mut self,
        graph_name: impl Into<GraphNameRef<'b>>,
    ) -> Result<(), StorageError> {
        self.writer.clear_graph(graph_name.into())
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
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// let quad = QuadRef::new(ex, ex, ex, ex);
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     transaction.insert(quad)?;
    ///     transaction.remove_named_graph(ex)
    /// })?;
    /// assert!(store.is_empty()?);
    /// assert_eq!(0, store.named_graphs().count());
    /// # Result::<_,oxigraph::store::StorageError>::Ok(())
    /// ```
    pub fn remove_named_graph<'b>(
        &mut self,
        graph_name: impl Into<NamedOrBlankNodeRef<'b>>,
    ) -> Result<bool, StorageError> {
        self.writer.remove_named_graph(graph_name.into())
    }

    /// Clears the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    ///
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     transaction.insert(QuadRef::new(ex, ex, ex, ex))?;
    ///     transaction.clear()
    /// })?;
    /// assert!(store.is_empty()?);
    /// # Result::<_,oxigraph::store::StorageError>::Ok(())
    /// ```
    pub fn clear(&mut self) -> Result<(), StorageError> {
        self.writer.clear()
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

/// A bulk loader allowing to load at lot of data quickly into the store.
///
/// Warning: The operations provided here are not atomic.
/// If the operation fails in the middle, only a part of the data may be written to the store.
/// Results might get weird if you delete data during the loading process.
///
/// Warning: It is optimized for speed.
/// Memory usage is configurable using [`BulkLoader::set_max_memory_size_in_megabytes`]
/// and the number of used threads with [`BulkLoader::set_num_threads`].
/// By default the memory consumption target (excluding the system and RocksDB internal consumption)
/// is 1GB per thread and the number of threads is set to the number of logical CPU cores provided by the system.
/// These targets are considered per loaded file.
///
/// Usage example with loading a dataset:
/// ```
/// use oxigraph::store::Store;
/// use oxigraph::io::DatasetFormat;
/// use oxigraph::model::*;
///
/// let store = Store::new()?;
///
/// // quads file insertion
/// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
/// store.bulk_loader().load_dataset(file.as_ref(), DatasetFormat::NQuads, None)?;
///
/// // we inspect the store contents
/// let ex = NamedNodeRef::new("http://example.com")?;
/// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[cfg(not(target_family = "wasm"))]
pub struct BulkLoader {
    storage: StorageBulkLoader,
    on_parse_error: Option<Box<dyn Fn(ParseError) -> Result<(), ParseError>>>,
}

#[cfg(not(target_family = "wasm"))]
impl BulkLoader {
    /// Sets the maximal number of threads to be used by the bulk loader per operation.
    ///
    /// This number must be at last 2 (one for parsing and one for loading).
    ///
    /// By default this is the number of logical CPU cores provided by the system except if
    /// [`BulkLoader::set_max_memory_size_in_megabytes`] is set. In this case at least one 1GB is reserved
    /// per used thread.
    pub fn set_num_threads(mut self, num_threads: usize) -> Self {
        self.storage = self.storage.set_num_threads(num_threads);
        self
    }

    /// Sets the maximal number of memory used by this operation.
    ///
    /// This number must be at last a few megabytes per thread.
    ///
    /// Memory used by RocksDB and the system is not taken into account in this limit.
    /// Note that depending on the system behavior this amount might never be reached.
    ///
    /// By default, at most 1GB per used thread is used
    /// (i.e. at most GBs at the number of available logical CPU cores in total).
    pub fn set_max_memory_size_in_megabytes(mut self, max_memory_size: usize) -> Self {
        self.storage = self
            .storage
            .set_max_memory_size_in_megabytes(max_memory_size);
        self
    }

    /// Adds a `callback` evaluated from time to time with the number of loaded triples.
    pub fn on_progress(mut self, callback: impl Fn(u64) + 'static) -> Self {
        self.storage = self.storage.on_progress(callback);
        self
    }

    /// Adds a `callback` catching all parse errors and choosing if the parsing should continue
    /// by returning `Ok` or fail by returning `Err`.
    ///
    /// By default the parsing fails.
    pub fn on_parse_error(
        mut self,
        callback: impl Fn(ParseError) -> Result<(), ParseError> + 'static,
    ) -> Self {
        self.on_parse_error = Some(Box::new(callback));
        self
    }

    /// Loads a dataset file using the bulk loader.
    ///
    /// This function is optimized for large dataset loading speed. For small files, [`Store::load_dataset`] might be more convenient.
    ///
    /// Warning: This method is not atomic.
    /// If the parsing fails in the middle of the file, only a part of it may be written to the store.
    /// Results might get weird if you delete data during the loading process.
    ///
    /// Warning: This method is optimized for speed. See [the struct](BulkLoader) documentation for more details.
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
    /// store.bulk_loader().load_dataset(file.as_ref(), DatasetFormat::NQuads, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
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
        self.storage
            .load(parser.read_quads(reader)?.filter_map(|r| match r {
                Ok(q) => Some(Ok(q)),
                Err(e) => {
                    if let Some(callback) = &self.on_parse_error {
                        if let Err(e) = callback(e) {
                            Some(Err(e))
                        } else {
                            None
                        }
                    } else {
                        Some(Err(e))
                    }
                }
            }))
    }

    /// Loads a graph file using the bulk loader.
    ///
    /// This function is optimized for large graph loading speed. For small files, [`Store::load_graph`] might be more convenient.   
    ///
    /// Warning: This method is not atomic.
    /// If the parsing fails in the middle of the file, only a part of it may be written to the store.
    /// Results might get weird if you delete data during the loading process.
    ///
    /// Warning: This method is optimized for speed. See [the struct](BulkLoader) documentation for more details.
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
    /// store.bulk_loader().load_graph(file.as_ref(), GraphFormat::NTriples, GraphNameRef::DefaultGraph, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
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
        let to_graph_name = to_graph_name.into();
        self.storage
            .load(parser.read_triples(reader)?.filter_map(|r| match r {
                Ok(q) => Some(Ok(q.in_graph(to_graph_name.into_owned()))),
                Err(e) => {
                    if let Some(callback) = &self.on_parse_error {
                        if let Err(e) = callback(e) {
                            Some(Err(e))
                        } else {
                            None
                        }
                    } else {
                        Some(Err(e))
                    }
                }
            }))
    }

    /// Adds a set of quads using the bulk loader.
    ///
    /// Warning: This method is not atomic.
    /// If the process fails in the middle of the file, only a part of the data may be written to the store.
    /// Results might get weird if you delete data during the loading process.
    ///
    /// Warning: This method is optimized for speed. See [the struct](BulkLoader) documentation for more details.
    pub fn load_quads(&self, quads: impl IntoIterator<Item = Quad>) -> Result<(), StorageError> {
        self.storage
            .load::<StorageError, _, _>(quads.into_iter().map(Ok))
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
