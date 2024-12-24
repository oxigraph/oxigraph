//! API to access an on-disk [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
//!
//! The entry point of the module is the [`Store`] struct.
//!
//! Usage example:
//! ```
//! use oxigraph::model::*;
//! use oxigraph::sparql::QueryResults;
//! use oxigraph::store::Store;
//!
//! let store = Store::new()?;
//!
//! // insertion
//! let ex = NamedNode::new("http://example.com")?;
//! let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
//! store.insert(&quad)?;
//!
//! // quad filter
//! let results: Result<Vec<Quad>, _> = store.quads_for_pattern(None, None, None, None).collect();
//! assert_eq!(vec![quad], results?);
//!
//! // SPARQL query
//! if let QueryResults::Solutions(mut solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
//!     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
//! };
//! # Result::<_, Box<dyn std::error::Error>>::Ok(())
//! ```
use crate::io::{RdfFormat, RdfParseError, RdfParser, RdfSerializer};
use crate::model::*;
use crate::sparql::{
    evaluate_query, evaluate_update, EvaluationError, Query, QueryExplanation, QueryOptions,
    QueryResults, Update, UpdateOptions,
};
use crate::storage::numeric_encoder::{Decoder, EncodedQuad, EncodedTerm};
pub use crate::storage::{CorruptionError, LoaderError, SerializerError, StorageError};
use crate::storage::{
    DecodingGraphIterator, DecodingQuadIterator, Storage, StorageBulkLoader, StorageReader,
    StorageWriter,
};
use std::error::Error;
use std::io::{Read, Write};
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
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
/// use oxigraph::model::*;
/// use oxigraph::sparql::QueryResults;
/// use oxigraph::store::Store;
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
/// let results: Result<Vec<Quad>, _> = store.quads_for_pattern(None, None, None, None).collect();
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
    /// New in-memory [`Store`] without RocksDB.
    pub fn new() -> Result<Self, StorageError> {
        Ok(Self {
            storage: Storage::new()?,
        })
    }

    /// Opens a read-write [`Store`] and creates it if it does not exist yet.
    ///
    /// Only one read-write [`Store`] can exist at the same time.
    /// If you want to have extra [`Store`] instance opened on the same data
    /// use [`Store::open_read_only`].
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Ok(Self {
            storage: Storage::open(path.as_ref())?,
        })
    }

    /// Opens a read-only [`Store`] from disk.
    ///
    /// Opening as read-only while having an other process writing the database is undefined behavior.
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn open_read_only(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        Ok(Self {
            storage: Storage::open_read_only(path.as_ref())?,
        })
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::QueryResults;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertions
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// store.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?;
    ///
    /// // SPARQL query
    /// if let QueryResults::Solutions(mut solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("s"),
    ///         Some(&ex.into_owned().into())
    ///     );
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
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryOptions, QueryResults};
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    /// if let QueryResults::Solutions(mut solutions) = store.query_opt(
    ///     "SELECT (<http://www.w3.org/ns/formats/N-Triples>(1) AS ?nt) WHERE {}",
    ///     QueryOptions::default().with_custom_function(
    ///         NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?,
    ///         |args| args.get(0).map(|t| Literal::from(t.to_string()).into()),
    ///     ),
    /// )? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("nt"),
    ///         Some(&Literal::from("\"1\"^^<http://www.w3.org/2001/XMLSchema#integer>").into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn query_opt(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
    ) -> Result<QueryResults, EvaluationError> {
        let (results, _) = self.explain_query_opt(query, options, false)?;
        results
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) with some options while substituting some variables with the given values.
    ///
    /// Substitution follows [RDF-dev SEP-0007](https://github.com/w3c/sparql-dev/blob/main/SEP/SEP-0007/sep-0007.md).
    ///
    /// Usage example with a custom function serializing terms to N-Triples:
    /// ```
    /// use oxigraph::model::{Literal, Variable};
    /// use oxigraph::sparql::{QueryOptions, QueryResults};
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    /// if let QueryResults::Solutions(mut solutions) = store.query_opt_with_substituted_variables(
    ///     "SELECT ?v WHERE {}",
    ///     QueryOptions::default(),
    ///     [(Variable::new("v")?, Literal::from(1).into())],
    /// )? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("v"),
    ///         Some(&Literal::from(1).into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn query_opt_with_substituted_variables(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> Result<QueryResults, EvaluationError> {
        let (results, _) = self.explain_query_opt_with_substituted_variables(
            query,
            options,
            false,
            substitutions,
        )?;
        results
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) with some options and
    /// returns a query explanation with some statistics (if enabled with the `with_stats` parameter).
    ///
    /// <div class="warning">If you want to compute statistics you need to exhaust the results iterator before having a look at them.</div>
    ///
    /// Usage example serialising the explanation with statistics in JSON:
    /// ```
    /// use oxigraph::sparql::{QueryOptions, QueryResults};
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    /// if let (Ok(QueryResults::Solutions(solutions)), explanation) = store.explain_query_opt(
    ///     "SELECT ?s WHERE { VALUES ?s { 1 2 3 } }",
    ///     QueryOptions::default(),
    ///     true,
    /// )? {
    ///     // We make sure to have read all the solutions
    ///     for _ in solutions {}
    ///     let mut buf = Vec::new();
    ///     explanation.write_in_json(&mut buf)?;
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn explain_query_opt(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
        with_stats: bool,
    ) -> Result<(Result<QueryResults, EvaluationError>, QueryExplanation), EvaluationError> {
        self.explain_query_opt_with_substituted_variables(query, options, with_stats, [])
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) with some options and
    /// returns a query explanation with some statistics (if enabled with the `with_stats` parameter).
    ///
    /// <div class="warning">If you want to compute statistics you need to exhaust the results iterator before having a look at them.</div>
    ///
    /// Usage example serialising the explanation with statistics in JSON:
    /// ```
    /// use oxigraph::sparql::{QueryOptions, QueryResults};
    /// use oxigraph::store::Store;
    /// use oxrdf::{Literal, Variable};
    ///
    /// let store = Store::new()?;
    /// if let (Ok(QueryResults::Solutions(solutions)), explanation) = store
    ///     .explain_query_opt_with_substituted_variables(
    ///         "SELECT ?s WHERE {}",
    ///         QueryOptions::default(),
    ///         true,
    ///         [(Variable::new("s")?, Literal::from(1).into())],
    ///     )?
    /// {
    ///     // We make sure to have read all the solutions
    ///     for _ in solutions {}
    ///     let mut buf = Vec::new();
    ///     explanation.write_in_json(&mut buf)?;
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn explain_query_opt_with_substituted_variables(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
        with_stats: bool,
        substitutions: impl IntoIterator<Item = (Variable, Term)>,
    ) -> Result<(Result<QueryResults, EvaluationError>, QueryExplanation), EvaluationError> {
        evaluate_query(
            self.storage.snapshot(),
            query,
            options,
            with_stats,
            substitutions,
        )
    }

    /// Retrieves quads with a filter on each quad component
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
    /// store.insert(&quad)?;
    ///
    /// // quad filter by object
    /// let results = store
    ///     .quads_for_pattern(None, None, Some((&ex).into()), None)
    ///     .collect::<Result<Vec<_>, _>>()?;
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
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
    /// store.insert(&quad)?;
    ///
    /// // quad filter by object
    /// let results = store.iter().collect::<Result<Vec<_>, _>>()?;
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
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
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
    /// <div class="warning">This function executes a full scan.</div>
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
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
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
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
    /// use oxigraph::model::*;
    /// use oxigraph::store::{StorageError, Store};
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
    pub fn transaction<T, E: Error + 'static + From<StorageError>>(
        &self,
        f: impl for<'a> Fn(Transaction<'a>) -> Result<T, E>,
    ) -> Result<T, E> {
        self.storage.transaction(|writer| f(Transaction { writer }))
    }

    /// Executes a [SPARQL 1.1 update](https://www.w3.org/TR/sparql11-update/).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// store
    ///     .update("INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }")?;
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

    /// Loads a RDF file under into the store.
    ///
    /// This function is atomic, quite slow and memory hungry. To get much better performances you might want to use the [`bulk_loader`](Store::bulk_loader).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxrdfio::RdfParser;
    ///
    /// let store = Store::new()?;
    ///
    /// // insert a dataset file (former load_dataset method)
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com/g> .";
    /// store.load_from_reader(RdfFormat::NQuads, file.as_ref())?;
    ///
    /// // insert a graph file (former load_graph method)
    /// let file = b"<> <> <> .";
    /// store.load_from_reader(
    ///     RdfParser::from_format(RdfFormat::Turtle)
    ///         .with_base_iri("http://example.com")?
    ///         .without_named_graphs() // No named graphs allowed in the input
    ///         .with_default_graph(NamedNodeRef::new("http://example.com/g2")?), // we put the file default graph inside of a named graph
    ///     file.as_ref()
    /// )?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, NamedNodeRef::new("http://example.com/g")?))?);
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, NamedNodeRef::new("http://example.com/g2")?))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn load_from_reader(
        &self,
        parser: impl Into<RdfParser>,
        reader: impl Read,
    ) -> Result<(), LoaderError> {
        let quads = parser
            .into()
            .rename_blank_nodes()
            .for_reader(reader)
            .collect::<Result<Vec<_>, _>>()?;
        self.storage.transaction(move |mut t| {
            for quad in &quads {
                t.insert(quad.as_ref())?;
            }
            Ok(())
        })
    }

    /// Loads a graph file (i.e. triples) into the store.
    ///
    /// This function is atomic, quite slow and memory hungry. To get much better performances you might want to use the [`bulk_loader`](Store::bulk_loader).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.load_graph(
    ///     file.as_ref(),
    ///     RdfFormat::NTriples,
    ///     GraphName::DefaultGraph,
    ///     None,
    /// )?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[deprecated(note = "use Store.load_from_reader instead", since = "0.4.0")]
    pub fn load_graph(
        &self,
        reader: impl Read,
        format: impl Into<RdfFormat>,
        to_graph_name: impl Into<GraphName>,
        base_iri: Option<&str>,
    ) -> Result<(), LoaderError> {
        let mut parser = RdfParser::from_format(format.into())
            .without_named_graphs()
            .with_default_graph(to_graph_name);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| LoaderError::InvalidBaseIri {
                    iri: base_iri.into(),
                    error: e,
                })?;
        }
        self.load_from_reader(parser, reader)
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// This function is atomic, quite slow and memory hungry. To get much better performances you might want to use the [`bulk_loader`](Store::bulk_loader).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let file =
    ///     b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store.load_dataset(file.as_ref(), RdfFormat::NQuads, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[deprecated(note = "use Store.load_from_reader instead", since = "0.4.0")]
    pub fn load_dataset(
        &self,
        reader: impl Read,
        format: impl Into<RdfFormat>,
        base_iri: Option<&str>,
    ) -> Result<(), LoaderError> {
        let mut parser = RdfParser::from_format(format.into());
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| LoaderError::InvalidBaseIri {
                    iri: base_iri.into(),
                    error: e,
                })?;
        }
        self.load_from_reader(parser, reader)
    }

    /// Adds a quad to this store.
    ///
    /// Returns `true` if the quad was not already in the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
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
    /// <div class="warning">
    ///
    /// This operation uses a memory heavy transaction internally, use the [`bulk_loader`](Store::bulk_loader) if you plan to add ten of millions of triples.</div>
    pub fn extend(
        &self,
        quads: impl IntoIterator<Item = impl Into<Quad>>,
    ) -> Result<(), StorageError> {
        let quads = quads.into_iter().map(Into::into).collect::<Vec<_>>();
        self.transaction(move |mut t| t.extend(&quads))
    }

    /// Removes a quad from this store.
    ///
    /// Returns `true` if the quad was in the store and has been removed.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
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

    /// Dumps the store into a file.
    ///    
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::store::Store;
    ///
    /// let file =
    ///     "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n"
    ///         .as_bytes();
    ///
    /// let store = Store::new()?;
    /// store.load_from_reader(RdfFormat::NQuads, file)?;
    ///
    /// let buffer = store.dump_to_writer(RdfFormat::NQuads, Vec::new())?;
    /// assert_eq!(file, buffer.as_slice());
    /// # std::io::Result::Ok(())
    /// ```
    pub fn dump_to_writer<W: Write>(
        &self,
        serializer: impl Into<RdfSerializer>,
        writer: W,
    ) -> Result<W, SerializerError> {
        let serializer = serializer.into();
        if !serializer.format().supports_datasets() {
            return Err(SerializerError::DatasetFormatExpected(serializer.format()));
        }
        let mut serializer = serializer.for_writer(writer);
        for quad in self {
            serializer.serialize_quad(&quad?)?;
        }
        Ok(serializer.finish()?)
    }

    /// Dumps a store graph into a file.
    ///    
    /// Usage example:
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = Store::new()?;
    /// store.load_graph(file, RdfFormat::NTriples, GraphName::DefaultGraph, None)?;
    ///
    /// let mut buffer = Vec::new();
    /// store.dump_graph_to_writer(GraphNameRef::DefaultGraph, RdfFormat::NTriples, &mut buffer)?;
    /// assert_eq!(file, buffer.as_slice());
    /// # std::io::Result::Ok(())
    /// ```
    pub fn dump_graph_to_writer<'a, W: Write>(
        &self,
        from_graph_name: impl Into<GraphNameRef<'a>>,
        serializer: impl Into<RdfSerializer>,
        writer: W,
    ) -> Result<W, SerializerError> {
        let mut serializer = serializer.into().for_writer(writer);
        for quad in self.quads_for_pattern(None, None, None, Some(from_graph_name.into())) {
            serializer.serialize_triple(quad?.as_ref())?;
        }
        Ok(serializer.finish()?)
    }

    /// Dumps a store graph into a file.
    ///    
    /// Usage example:
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = Store::new()?;
    /// store.load_graph(file, RdfFormat::NTriples, GraphName::DefaultGraph, None)?;
    ///
    /// let mut buffer = Vec::new();
    /// store.dump_graph(&mut buffer, RdfFormat::NTriples, GraphNameRef::DefaultGraph)?;
    /// assert_eq!(file, buffer.as_slice());
    /// # std::io::Result::Ok(())
    /// ```
    #[deprecated(note = "use Store.dump_graph_to_writer instead", since = "0.4.0")]
    pub fn dump_graph<'a, W: Write>(
        &self,
        writer: W,
        format: impl Into<RdfFormat>,
        from_graph_name: impl Into<GraphNameRef<'a>>,
    ) -> Result<W, SerializerError> {
        self.dump_graph_to_writer(from_graph_name, format.into(), writer)
    }

    /// Dumps the store into a file.
    ///    
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::store::Store;
    ///
    /// let file =
    ///     "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n"
    ///         .as_bytes();
    ///
    /// let store = Store::new()?;
    /// store.load_from_reader(RdfFormat::NQuads, file)?;
    ///
    /// let buffer = store.dump_dataset(Vec::new(), RdfFormat::NQuads)?;
    /// assert_eq!(file, buffer.as_slice());
    /// # std::io::Result::Ok(())
    /// ```
    #[deprecated(note = "use Store.dump_to_writer instead", since = "0.4.0")]
    pub fn dump_dataset<W: Write>(
        &self,
        writer: W,
        format: impl Into<RdfFormat>,
    ) -> Result<W, SerializerError> {
        self.dump_to_writer(format.into(), writer)
    }

    /// Returns all the store named graphs.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let store = Store::new()?;
    /// store.insert(QuadRef::new(&ex, &ex, &ex, &ex))?;
    /// store.insert(QuadRef::new(&ex, &ex, &ex, GraphNameRef::DefaultGraph))?;
    /// assert_eq!(
    ///     vec![NamedOrBlankNode::from(ex)],
    ///     store.named_graphs().collect::<Result<Vec<_>, _>>()?
    /// );
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
    /// use oxigraph::model::{NamedNode, QuadRef};
    /// use oxigraph::store::Store;
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
    /// use oxigraph::model::NamedNodeRef;
    /// use oxigraph::store::Store;
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let store = Store::new()?;
    /// store.insert_named_graph(ex)?;
    ///
    /// assert_eq!(
    ///     store.named_graphs().collect::<Result<Vec<_>, _>>()?,
    ///     vec![ex.into_owned().into()]
    /// );
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
    /// use oxigraph::model::{NamedNodeRef, QuadRef};
    /// use oxigraph::store::Store;
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
    /// use oxigraph::model::{NamedNodeRef, QuadRef};
    /// use oxigraph::store::Store;
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
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
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
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn flush(&self) -> Result<(), StorageError> {
        self.storage.flush()
    }

    /// Optimizes the database for future workload.
    ///
    /// Useful to call after a batch upload or another similar operation.
    ///
    /// <div class="warning">Can take hours on huge databases.</div>
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn optimize(&self) -> Result<(), StorageError> {
        self.storage.compact()
    }

    /// Creates database backup into the `target_directory`.
    ///
    /// After its creation, the backup is usable using [`Store::open`]
    /// like a regular Oxigraph database and operates independently from the original database.
    ///
    /// <div class="warning">
    ///
    /// Backups are only possible for on-disk databases created using [`Store::open`].</div>
    /// Temporary in-memory databases created using [`Store::new`] are not compatible with RocksDB backup system.
    ///
    /// <div class="warning">An error is raised if the `target_directory` already exists.</div>
    ///
    /// If the target directory is in the same file system as the current database,
    /// the database content will not be fully copied
    /// but hard links will be used to point to the original database immutable snapshots.
    /// This allows cheap regular backups.
    ///
    /// If you want to move your data to another RDF storage system, you should have a look at the [`Store::dump_to_writer`] function instead.
    #[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
    pub fn backup(&self, target_directory: impl AsRef<Path>) -> Result<(), StorageError> {
        self.storage.backup(target_directory.as_ref())
    }

    /// Creates a bulk loader allowing to load at lot of data quickly into the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // quads file insertion
    /// let file =
    ///     b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store
    ///     .bulk_loader()
    ///     .load_from_reader(RdfFormat::NQuads, file.as_ref())?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn bulk_loader(&self) -> BulkLoader {
        BulkLoader {
            storage: self.storage.bulk_loader(),
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
        for t in self {
            writeln!(f, "{} .", t.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

impl IntoIterator for &Store {
    type IntoIter = QuadIter;
    type Item = Result<Quad, StorageError>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An object to do operations during a transaction.
///
/// See [`Store::transaction`] for a more detailed description.
pub struct Transaction<'a> {
    writer: StorageWriter<'a>,
}

impl Transaction<'_> {
    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{EvaluationError, QueryResults};
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     if let QueryResults::Solutions(solutions) =
    ///         transaction.query("SELECT ?s WHERE { ?s ?p ?o }")?
    ///     {
    ///         for solution in solutions {
    ///             if let Some(Term::NamedNode(s)) = solution?.get("s") {
    ///                 transaction.insert(QuadRef::new(
    ///                     s,
    ///                     vocab::rdf::TYPE,
    ///                     NamedNodeRef::new_unchecked("http://example.com"),
    ///                     GraphNameRef::DefaultGraph,
    ///                 ))?;
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
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{EvaluationError, QueryOptions, QueryResults};
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     if let QueryResults::Solutions(solutions) = transaction.query_opt(
    ///         "SELECT ?s (<http://www.w3.org/ns/formats/N-Triples>(?s) AS ?nt) WHERE { ?s ?p ?o }",
    ///         QueryOptions::default().with_custom_function(
    ///             NamedNode::new_unchecked("http://www.w3.org/ns/formats/N-Triples"),
    ///             |args| args.get(0).map(|t| Literal::from(t.to_string()).into()),
    ///         ),
    ///     )? {
    ///         for solution in solutions {
    ///             let solution = solution?;
    ///             if let (Some(Term::NamedNode(s)), Some(nt)) =
    ///                 (solution.get("s"), solution.get("nt"))
    ///             {
    ///                 transaction.insert(QuadRef::new(
    ///                     s,
    ///                     NamedNodeRef::new_unchecked("http://example.com/n-triples-representation"),
    ///                     nt,
    ///                     GraphNameRef::DefaultGraph,
    ///                 ))?;
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
        let (results, _) = evaluate_query(self.writer.reader(), query, options, false, [])?;
        results
    }

    /// Retrieves quads with a filter on each quad component.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::store::{StorageError, Store};
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
    /// <div class="warning">this function executes a full scan.</div>
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
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::EvaluationError;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| {
    ///     // insertion
    ///     transaction.update(
    ///         "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
    ///     )?;
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

    /// Loads a RDF file into the store.
    ///
    /// This function is atomic, quite slow and memory hungry. To get much better performances you might want to use the [`bulk_loader`](Store::bulk_loader).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxrdfio::RdfParser;
    ///
    /// let store = Store::new()?;
    ///
    /// // insert a dataset file (former load_dataset method)
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com/g> .";
    /// store.transaction(|mut t| t.load_from_reader(RdfFormat::NQuads, file.as_ref()))?;
    ///
    /// // insert a graph file (former load_graph method)
    /// let file = b"<> <> <> .";
    /// store.transaction(|mut t|
    ///     t.load_from_reader(
    ///         RdfParser::from_format(RdfFormat::Turtle)
    ///             .with_base_iri("http://example.com")
    ///             .unwrap()
    ///             .without_named_graphs() // No named graphs allowed in the input
    ///             .with_default_graph(NamedNodeRef::new("http://example.com/g2").unwrap()), // we put the file default graph inside of a named graph
    ///         file.as_ref()
    ///     )
    /// )?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, NamedNodeRef::new("http://example.com/g")?))?);
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, NamedNodeRef::new("http://example.com/g2")?))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn load_from_reader(
        &mut self,
        parser: impl Into<RdfParser>,
        reader: impl Read,
    ) -> Result<(), LoaderError> {
        for quad in parser.into().rename_blank_nodes().for_reader(reader) {
            self.insert(quad?.as_ref())?;
        }
        Ok(())
    }

    /// Loads a graph file (i.e. triples) into the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.transaction(|mut transaction| {
    ///     transaction.load_graph(
    ///         file.as_ref(),
    ///         RdfFormat::NTriples,
    ///         GraphName::DefaultGraph,
    ///         None,
    ///     )
    /// })?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[deprecated(note = "use Transaction.load_from_reader instead", since = "0.4.0")]
    pub fn load_graph(
        &mut self,
        reader: impl Read,
        format: impl Into<RdfFormat>,
        to_graph_name: impl Into<GraphName>,
        base_iri: Option<&str>,
    ) -> Result<(), LoaderError> {
        let mut parser = RdfParser::from_format(format.into())
            .without_named_graphs()
            .with_default_graph(to_graph_name);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| LoaderError::InvalidBaseIri {
                    iri: base_iri.into(),
                    error: e,
                })?;
        }
        self.load_from_reader(parser, reader)
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let file =
    ///     b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store.transaction(|mut transaction| {
    ///     transaction.load_dataset(file.as_ref(), RdfFormat::NQuads, None)
    /// })?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
    /// # Result::<_,oxigraph::store::LoaderError>::Ok(())
    /// ```
    #[deprecated(note = "use Transaction.load_from_reader instead", since = "0.4.0")]
    pub fn load_dataset(
        &mut self,
        reader: impl Read,
        format: impl Into<RdfFormat>,
        base_iri: Option<&str>,
    ) -> Result<(), LoaderError> {
        let mut parser = RdfParser::from_format(format.into());
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| LoaderError::InvalidBaseIri {
                    iri: base_iri.into(),
                    error: e,
                })?;
        }
        self.load_from_reader(parser, reader)
    }

    /// Adds a quad to this store.
    ///
    /// Returns `true` if the quad was not already in the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// let quad = QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph);
    ///
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| transaction.insert(quad))?;
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
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
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
    /// use oxigraph::model::NamedNodeRef;
    /// use oxigraph::store::Store;
    ///
    /// let ex = NamedNodeRef::new_unchecked("http://example.com");
    /// let store = Store::new()?;
    /// store.transaction(|mut transaction| transaction.insert_named_graph(ex))?;
    /// assert_eq!(
    ///     store.named_graphs().collect::<Result<Vec<_>, _>>()?,
    ///     vec![ex.into_owned().into()]
    /// );
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
    /// use oxigraph::model::{NamedNodeRef, QuadRef};
    /// use oxigraph::store::Store;
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
    /// use oxigraph::model::{NamedNodeRef, QuadRef};
    /// use oxigraph::store::Store;
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
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
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

impl IntoIterator for &Transaction<'_> {
    type IntoIter = QuadIter;
    type Item = Result<Quad, StorageError>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// An iterator returning the quads contained in a [`Store`].
pub struct QuadIter {
    iter: DecodingQuadIterator,
    reader: StorageReader,
}

impl Iterator for QuadIter {
    type Item = Result<Quad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
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

    fn next(&mut self) -> Option<Self::Item> {
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
/// <div class="warning">The operations provided here are not atomic.
/// If the operation fails in the middle, only a part of the data may be written to the store.
/// Results might get weird if you delete data during the loading process.</div>
///
/// Memory usage is configurable using [`with_max_memory_size_in_megabytes`](Self::with_max_memory_size_in_megabytes)
/// and the number of used threads with [`with_num_threads`](Self::with_num_threads).
/// By default the memory consumption target (excluding the system and RocksDB internal consumption)
/// is around 2GB per thread and 2 threads.
/// These targets are considered per loaded file.
///
/// Usage example with loading a dataset:
/// ```
/// use oxigraph::io::RdfFormat;
/// use oxigraph::model::*;
/// use oxigraph::store::Store;
///
/// let store = Store::new()?;
///
/// // quads file insertion
/// let file =
///     b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
/// store
///     .bulk_loader()
///     .load_from_reader(RdfFormat::NQuads, file.as_ref())?;
///
/// // we inspect the store contents
/// let ex = NamedNodeRef::new("http://example.com")?;
/// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct BulkLoader {
    storage: StorageBulkLoader,
    on_parse_error: Option<Box<dyn Fn(RdfParseError) -> Result<(), RdfParseError>>>,
}

impl BulkLoader {
    /// Sets the maximal number of threads to be used by the bulk loader per operation.
    ///
    /// This number must be at last 2 (one for parsing and one for loading).
    ///
    /// The default value is 2.
    pub fn with_num_threads(mut self, num_threads: usize) -> Self {
        self.storage = self.storage.with_num_threads(num_threads);
        self
    }

    #[doc(hidden)]
    #[deprecated(note = "Use with_num_threads", since = "0.4.0")]
    pub fn set_num_threads(self, num_threads: usize) -> Self {
        self.with_num_threads(num_threads)
    }

    /// Sets a rough idea of the maximal amount of memory to be used by this operation.
    ///
    /// This number must be at last a few megabytes per thread.
    ///
    /// Memory used by RocksDB and the system is not taken into account in this limit.
    /// Note that depending on the system behavior this amount might never be reached or be blown up
    /// (for example if the data contains very long IRIs or literals).
    ///
    /// By default, a target 2GB per used thread is used.
    pub fn with_max_memory_size_in_megabytes(mut self, max_memory_size: usize) -> Self {
        self.storage = self
            .storage
            .with_max_memory_size_in_megabytes(max_memory_size);
        self
    }

    #[doc(hidden)]
    #[deprecated(note = "Use with_max_memory_size_in_megabytes", since = "0.4.0")]
    pub fn set_max_memory_size_in_megabytes(self, max_memory_size: usize) -> Self {
        self.with_max_memory_size_in_megabytes(max_memory_size)
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
        callback: impl Fn(RdfParseError) -> Result<(), RdfParseError> + 'static,
    ) -> Self {
        self.on_parse_error = Some(Box::new(callback));
        self
    }

    /// Loads a file using the bulk loader.
    ///
    /// This function is optimized for large dataset loading speed. For small files, [`Store::load_from_reader`] might be more convenient.
    ///
    /// <div class="warning">This method is not atomic.
    /// If the parsing fails in the middle of the file, only a part of it may be written to the store.
    /// Results might get weird if you delete data during the loading process.</div>
    ///
    /// This method is optimized for speed. See [the struct](Self) documentation for more details.
    ///
    /// To get better speed on valid datasets, consider enabling [`RdfParser::unchecked`] option to skip some validations.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::io::{RdfParser, RdfFormat};
    /// use oxigraph::model::*;
    ///
    /// let store = Store::new()?;
    ///
    /// // insert a dataset file (former load_dataset method)
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com/g> .";
    /// store.bulk_loader().load_from_reader(
    ///     RdfParser::from_format(RdfFormat::NQuads).unchecked(), // we inject a custom parser with options
    ///     file.as_ref()
    /// )?;
    ///
    /// // insert a graph file (former load_graph method)
    /// let file = b"<> <> <> .";
    /// store.bulk_loader().load_from_reader(
    ///     RdfParser::from_format(RdfFormat::Turtle)
    ///         .with_base_iri("http://example.com")?
    ///         .without_named_graphs() // No named graphs allowed in the input
    ///         .with_default_graph(NamedNodeRef::new("http://example.com/g2")?), // we put the file default graph inside of a named graph
    ///     file.as_ref()
    /// )?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, NamedNodeRef::new("http://example.com/g")?))?);
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, NamedNodeRef::new("http://example.com/g2")?))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn load_from_reader(
        &self,
        parser: impl Into<RdfParser>,
        reader: impl Read,
    ) -> Result<(), LoaderError> {
        self.load_ok_quads(
            parser
                .into()
                .rename_blank_nodes()
                .for_reader(reader)
                .filter_map(|r| match r {
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
                }),
        )
    }

    /// Loads a dataset file using the bulk loader.
    ///
    /// This function is optimized for large dataset loading speed. For small files, [`Store::load_dataset`] might be more convenient.
    ///
    /// <div class="warning">This method is not atomic.
    /// If the parsing fails in the middle of the file, only a part of it may be written to the store.
    /// Results might get weird if you delete data during the loading process.</div>
    ///
    /// This method is optimized for speed. See [the struct](Self) documentation for more details.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let file =
    ///     b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store
    ///     .bulk_loader()
    ///     .load_dataset(file.as_ref(), RdfFormat::NQuads, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[deprecated(note = "use BulkLoader.load_from_reader instead", since = "0.4.0")]
    pub fn load_dataset(
        &self,
        reader: impl Read,
        format: impl Into<RdfFormat>,
        base_iri: Option<&str>,
    ) -> Result<(), LoaderError> {
        let mut parser = RdfParser::from_format(format.into()).rename_blank_nodes();
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| LoaderError::InvalidBaseIri {
                    iri: base_iri.into(),
                    error: e,
                })?;
        }
        self.load_ok_quads(parser.for_reader(reader).filter_map(|r| match r {
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
    /// <div class="warning">This method is not atomic.
    /// If the parsing fails in the middle of the file, only a part of it may be written to the store.
    /// Results might get weird if you delete data during the loading process.</div>
    ///
    /// This method is optimized for speed. See [the struct](Self) documentation for more details.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::RdfFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.bulk_loader().load_graph(
    ///     file.as_ref(),
    ///     RdfFormat::NTriples,
    ///     GraphName::DefaultGraph,
    ///     None,
    /// )?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[deprecated(note = "use BulkLoader.load_from_reader instead", since = "0.4.0")]
    pub fn load_graph(
        &self,
        reader: impl Read,
        format: impl Into<RdfFormat>,
        to_graph_name: impl Into<GraphName>,
        base_iri: Option<&str>,
    ) -> Result<(), LoaderError> {
        let mut parser = RdfParser::from_format(format.into())
            .without_named_graphs()
            .with_default_graph(to_graph_name)
            .rename_blank_nodes();
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| LoaderError::InvalidBaseIri {
                    iri: base_iri.into(),
                    error: e,
                })?;
        }
        self.load_ok_quads(parser.for_reader(reader).filter_map(|r| match r {
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

    /// Adds a set of quads using the bulk loader.
    ///
    /// <div class="warning">This method is not atomic.
    /// If the process fails in the middle of the file, only a part of the data may be written to the store.
    /// Results might get weird if you delete data during the loading process.</div>
    ///
    /// This method is optimized for speed. See [the struct](Self) documentation for more details.
    pub fn load_quads(
        &self,
        quads: impl IntoIterator<Item = impl Into<Quad>>,
    ) -> Result<(), StorageError> {
        self.load_ok_quads(quads.into_iter().map(Ok::<_, StorageError>))
    }

    /// Adds a set of quads using the bulk loader while breaking in the middle of the process in case of error.
    ///
    /// <div class="warning">This method is not atomic.
    /// If the process fails in the middle of the file, only a part of the data may be written to the store.
    /// Results might get weird if you delete data during the loading process.</div>
    ///
    /// This method is optimized for speed. See [the struct](Self) documentation for more details.
    pub fn load_ok_quads<EI, EO: From<StorageError> + From<EI>>(
        &self,
        quads: impl IntoIterator<Item = Result<impl Into<Quad>, EI>>,
    ) -> Result<(), EO> {
        self.storage
            .load(quads.into_iter().map(|q| q.map(Into::into)))
    }
}

#[cfg(test)]
#[allow(clippy::panic_in_result_fn)]
mod tests {
    use super::*;

    #[test]
    fn test_send_sync() {
        fn is_send_sync<T: Send + Sync>() {}
        is_send_sync::<Store>();
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
        let mut default_quads = vec![
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
            named_quad.clone(),
            Quad::new(
                main_s.clone(),
                main_p.clone(),
                Literal::from(200_000_000),
                GraphName::DefaultGraph,
            ),
            default_quad.clone(),
            Quad::new(
                main_s.clone(),
                main_p.clone(),
                Literal::from(0),
                GraphName::DefaultGraph,
            ),
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
        store.validate()?;

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
            vec![named_quad.clone(), default_quad.clone()]
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
        default_quads.reverse();
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
            vec![named_quad.clone(), default_quad.clone()]
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
            vec![named_quad.clone(), default_quad.clone()]
        );
        assert_eq!(
            store
                .quads_for_pattern(None, None, Some(main_o.as_ref()), None)
                .collect::<Result<Vec<_>, _>>()?,
            vec![named_quad.clone(), default_quad.clone()]
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
}
