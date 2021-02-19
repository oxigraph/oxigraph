//! In-memory store.

use crate::error::{invalid_input_error, UnwrapInfallible};
use crate::io::{DatasetFormat, DatasetParser, GraphFormat, GraphParser};
use crate::model::*;
use crate::sparql::{
    evaluate_query, evaluate_update, EvaluationError, Query, QueryOptions, QueryResults, Update,
    UpdateOptions,
};
use crate::store::numeric_encoder::{
    Decoder, ReadEncoder, StrContainer, StrEncodingAware, StrId, StrLookup, WriteEncoder,
};
use crate::store::{
    dump_dataset, dump_graph, get_encoded_quad_pattern, load_dataset, load_graph,
    ReadableEncodedStore, WritableEncodedStore,
};
use lasso::{LargeSpur, ThreadedRodeo};
use std::collections::hash_map::DefaultHasher;
use std::collections::{HashMap, HashSet};
use std::convert::{Infallible, TryInto};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, Write};
use std::iter::FromIterator;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::vec::IntoIter;
use std::{fmt, io};

/// In-memory store.
/// It encodes a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) and allows to query it using SPARQL.
/// It is cheap to build using the [`MemoryStore::new()`] method.
///
/// Usage example:
/// ```
/// use oxigraph::MemoryStore;
/// use oxigraph::model::*;
/// use oxigraph::sparql::QueryResults;
///
/// let store = MemoryStore::new();
///
/// // insertion
/// let ex = NamedNode::new("http://example.com")?;
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// store.insert(quad.clone());
///
/// // quad filter
/// let results: Vec<Quad> = store.quads_for_pattern(Some(ex.as_ref().into()), None, None, None).collect();
/// assert_eq!(vec![quad], results);
///
/// // SPARQL query
/// if let QueryResults::Solutions(mut solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// }
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone)]
pub struct MemoryStore {
    indexes: Arc<RwLock<MemoryStoreIndexes>>,
    strings: Arc<ThreadedRodeo<LargeSpur>>,
}

type TripleMap<T> = HashMap<T, HashMap<T, HashSet<T>>>;
type QuadMap<T> = HashMap<T, TripleMap<T>>;
type EncodedTerm = crate::store::numeric_encoder::EncodedTerm<LargeSpur>;
type EncodedQuad = crate::store::numeric_encoder::EncodedQuad<LargeSpur>;

#[derive(Default)]
struct MemoryStoreIndexes {
    spog: QuadMap<EncodedTerm>,
    posg: QuadMap<EncodedTerm>,
    ospg: QuadMap<EncodedTerm>,
    gspo: QuadMap<EncodedTerm>,
    gpos: QuadMap<EncodedTerm>,
    gosp: QuadMap<EncodedTerm>,
    default_spo: TripleMap<EncodedTerm>,
    default_pos: TripleMap<EncodedTerm>,
    default_osp: TripleMap<EncodedTerm>,
}

impl Default for MemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryStore {
    /// Constructs a new [`MemoryStore`]()
    pub fn new() -> Self {
        Self {
            indexes: Arc::new(RwLock::default()),
            strings: Arc::new(ThreadedRodeo::new()),
        }
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/).
    ///
    /// The default query options are used.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::QueryResults;
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertions
    /// let ex = NamedNode::new("http://example.com")?;
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
    ///
    /// // SPARQL query
    /// if let QueryResults::Solutions(mut solutions) =  store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
    ///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
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
        evaluate_query(self.clone(), query, options)
    }

    /// Retrieves quads with a filter on each quad component
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::*;
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
    /// store.insert(quad.clone());
    ///
    /// // quad filter by object
    /// let results: Vec<Quad> = store.quads_for_pattern(None, None, Some((&ex).into()), None).collect();
    /// assert_eq!(vec![quad], results);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn quads_for_pattern(
        &self,
        subject: Option<NamedOrBlankNodeRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
        graph_name: Option<GraphNameRef<'_>>,
    ) -> MemoryQuadIter {
        let quads = if let Some((subject, predicate, object, graph_name)) =
            get_encoded_quad_pattern(self, subject, predicate, object, graph_name)
                .unwrap_infallible()
        {
            self.encoded_quads_for_pattern_inner(subject, predicate, object, graph_name)
        } else {
            Vec::new()
        };
        MemoryQuadIter {
            iter: quads.into_iter(),
            store: self.clone(),
        }
    }

    /// Returns all the quads contained in the store
    pub fn iter(&self) -> MemoryQuadIter {
        MemoryQuadIter {
            iter: self.encoded_quads().into_iter(),
            store: self.clone(),
        }
    }

    /// Checks if this store contains a given quad
    pub fn contains<'a>(&self, quad: impl Into<QuadRef<'a>>) -> bool {
        self.get_encoded_quad(quad.into())
            .unwrap_infallible()
            .map_or(false, |q| self.contains_encoded(&q))
    }

    /// Returns the number of quads in the store
    ///
    /// Warning: this function executes a full scan
    pub fn len(&self) -> usize {
        let indexes = self.indexes();
        let default: usize = indexes
            .default_spo
            .values()
            .map(|v| v.values().map(|v| v.len()).sum::<usize>())
            .sum();
        let named: usize = indexes
            .spog
            .values()
            .map(|v| {
                v.values()
                    .map(|v| v.values().map(|v| v.len()).sum::<usize>())
                    .sum::<usize>()
            })
            .sum();
        default + named
    }

    /// Returns if the store is empty
    pub fn is_empty(&self) -> bool {
        let indexes = self.indexes();
        indexes.default_spo.is_empty() && indexes.spog.is_empty()
    }

    /// Executes a [SPARQL 1.1 update](https://www.w3.org/TR/sparql11-update/).
    ///
    /// The store does not track the existence of empty named graphs.
    /// This method has no ACID guarantees.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::*;
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// store.update("INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }")?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com").unwrap();
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, None)));
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
            self.clone(),
            &mut &*self,
            update.try_into().map_err(|e| e.into())?,
            options,
        )
    }

    /// Executes an ACID transaction.
    ///
    /// The transaction is executed if the given closure returns `Ok`.
    /// The transaction if rollbacked if the closure returns `Err`.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::*;
    /// use std::convert::Infallible;
    ///
    /// let store = MemoryStore::new();
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone());
    ///
    /// store.transaction(|transaction| {
    ///     transaction.insert(quad.clone());
    ///     Ok(()) as Result<(),Infallible>
    /// })?;
    ///
    /// assert!(store.contains(&quad));
    /// assert!(store.contains_named_graph(&ex));
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn transaction<E>(
        &self,
        f: impl FnOnce(&mut MemoryTransaction) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut transaction = MemoryTransaction { ops: Vec::new() };
        f(&mut transaction)?;

        let mut this = self;
        let mut indexes = self.indexes_mut();
        for op in transaction.ops {
            match op {
                TransactionOp::Insert(quad) => {
                    let quad = this.encode_quad(quad.as_ref()).unwrap_infallible();
                    indexes.insert_encoded(&quad).unwrap_infallible()
                }
                TransactionOp::Delete(quad) => {
                    let quad = this.encode_quad(quad.as_ref()).unwrap_infallible();
                    indexes.remove_encoded(&quad).unwrap_infallible()
                }
            }
        }
        Ok(())
    }

    /// Loads a graph file (i.e. triples) into the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::*;
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.load_graph(file.as_ref(), GraphFormat::NTriples, &GraphName::DefaultGraph, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, None)));
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// Warning: This functions saves the triples during the parsing.
    /// If the parsing fails in the middle of the file, the triples read before stay in the store.
    /// Use a (memory greedy) [transaction](MemoryStore::transaction()) if you do not want that.
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    pub fn load_graph<'a>(
        &self,
        reader: impl BufRead,
        format: GraphFormat,
        to_graph_name: impl Into<GraphNameRef<'a>>,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        let mut store = self;
        load_graph(&mut store, reader, format, to_graph_name.into(), base_iri)?;
        Ok(())
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::io::DatasetFormat;
    /// use oxigraph::model::*;
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store.load_dataset(file.as_ref(), DatasetFormat::NQuads, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex)));
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// Warning: This functions saves the quads during the parsing.
    /// If the parsing fails in the middle of the file, the quads read before stay in the store.
    /// Use a (memory greedy) [transaction](MemoryStore::transaction()) if you do not want that.
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    pub fn load_dataset(
        &self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        let mut store = self;
        load_dataset(&mut store, reader, format, base_iri)?;
        Ok(())
    }

    /// Adds a quad to this store.
    #[allow(clippy::needless_pass_by_value)]
    pub fn insert(&self, quad: impl Into<Quad>) {
        let mut this = self;
        let quad = this.encode_quad(quad.into().as_ref()).unwrap_infallible();
        this.insert_encoded(&quad).unwrap_infallible();
    }

    /// Removes a quad from this store.
    pub fn remove<'a>(&self, quad: impl Into<QuadRef<'a>>) {
        if let Some(quad) = self.get_encoded_quad(quad.into()).unwrap_infallible() {
            let mut this = self;
            this.remove_encoded(&quad).unwrap_infallible();
        }
    }

    /// Returns if the current dataset is [isomorphic](https://www.w3.org/TR/rdf11-concepts/#dfn-dataset-isomorphism) with another one.
    ///
    /// It is implemented using the canonicalization approach presented in
    /// [Canonical Forms for Isomorphic and Equivalent RDF Graphs: Algorithms for Leaning and Labelling Blank Nodes, Aidan Hogan, 2017](http://aidanhogan.com/docs/rdf-canonicalisation.pdf)
    ///
    /// Warning: This implementation worst-case complexity is in O(b!) with b the number of blank nodes in the input graphs.
    pub fn is_isomorphic(&self, other: &Self) -> bool {
        iso_canonicalize(self) == iso_canonicalize(other)
    }

    /// Dumps a store graph into a file.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::GraphName;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = MemoryStore::new();
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
                .map(|q| Ok(q.into())),
            writer,
            format,
        )
    }

    /// Dumps the store into a file.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::io::DatasetFormat;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = MemoryStore::new();
    /// store.load_dataset(file, DatasetFormat::NQuads, None)?;
    ///
    /// let mut buffer = Vec::new();
    /// store.dump_dataset(&mut buffer, DatasetFormat::NQuads)?;
    /// assert_eq!(file, buffer.as_slice());
    /// # std::io::Result::Ok(())
    /// ```
    pub fn dump_dataset(&self, writer: impl Write, format: DatasetFormat) -> Result<(), io::Error> {
        dump_dataset(self.iter().map(Ok), writer, format)
    }

    /// Returns all the store named graphs
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::{NamedNode, Quad, NamedOrBlankNode};
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let store = MemoryStore::new();
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone()));
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), None));     
    /// assert_eq!(vec![NamedOrBlankNode::from(ex)], store.named_graphs().collect::<Vec<_>>());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn named_graphs(&self) -> MemoryGraphNameIter {
        MemoryGraphNameIter {
            iter: self.encoded_named_graphs(),
            store: self.clone(),
        }
    }

    /// Checks if the store contains a given graph
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::{NamedNode, Quad};
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let store = MemoryStore::new();
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone()));
    /// assert!(store.contains_named_graph(&ex));
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn contains_named_graph<'a>(&self, graph_name: impl Into<NamedOrBlankNodeRef<'a>>) -> bool {
        if let Some(graph_name) = self
            .get_encoded_named_or_blank_node(graph_name.into())
            .unwrap_infallible()
        {
            self.contains_encoded_named_graph(graph_name)
                .unwrap_infallible()
        } else {
            false
        }
    }

    /// Inserts a graph into this store
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::NamedNode;
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let store = MemoryStore::new();
    /// store.insert_named_graph(ex.clone());
    /// assert_eq!(store.named_graphs().count(), 1);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn insert_named_graph(&self, graph_name: impl Into<NamedOrBlankNode>) {
        let mut this = self;
        let graph_name = this
            .encode_named_or_blank_node(graph_name.into().as_ref())
            .unwrap_infallible();
        this.insert_encoded_named_graph(graph_name)
            .unwrap_infallible()
    }

    /// Clears a graph from this store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::{NamedNode, Quad};
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone());
    /// let store = MemoryStore::new();
    /// store.insert(quad.clone());
    /// assert_eq!(1, store.len());
    ///
    /// store.clear_graph(&ex);
    /// assert!(store.is_empty());
    /// assert_eq!(1, store.named_graphs().count());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn clear_graph<'a>(&self, graph_name: impl Into<GraphNameRef<'a>>) {
        if let Some(graph_name) = self
            .get_encoded_graph_name(graph_name.into())
            .unwrap_infallible()
        {
            let mut this = self;
            this.clear_encoded_graph(graph_name).unwrap_infallible()
        }
    }

    /// Removes a graph from this store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::{NamedNode, Quad};
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone());
    /// let store = MemoryStore::new();
    /// store.insert(quad.clone());
    /// assert_eq!(1, store.len());
    ///
    /// store.remove_named_graph(&ex);
    /// assert!(store.is_empty());
    /// assert_eq!(0, store.named_graphs().count());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn remove_named_graph<'a>(&self, graph_name: impl Into<NamedOrBlankNodeRef<'a>>) {
        if let Some(graph_name) = self
            .get_encoded_named_or_blank_node(graph_name.into())
            .unwrap_infallible()
        {
            let mut this = self;
            this.remove_encoded_named_graph(graph_name)
                .unwrap_infallible()
        }
    }

    /// Clears the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::model::{NamedNode, Quad};
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let store = MemoryStore::new();
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone()));
    /// store.insert(Quad::new(ex.clone(), ex.clone(), ex, None));
    /// assert_eq!(2, store.len());
    ///
    /// store.clear();
    /// assert!(store.is_empty());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn clear(&self) {
        self.indexes_mut().clear().unwrap_infallible()
    }

    #[allow(clippy::expect_used)]
    fn indexes(&self) -> RwLockReadGuard<'_, MemoryStoreIndexes> {
        self.indexes
            .read()
            .expect("the Memory store mutex has been poisoned because of a panic")
    }

    #[allow(clippy::expect_used)]
    fn indexes_mut(&self) -> RwLockWriteGuard<'_, MemoryStoreIndexes> {
        self.indexes
            .write()
            .expect("the Memory store mutex has been poisoned because of a panic")
    }

    fn contains_encoded(&self, quad: &EncodedQuad) -> bool {
        let indexes = self.indexes();
        if quad.graph_name.is_default_graph() {
            indexes.default_spo.get(&quad.subject).map_or(false, |po| {
                po.get(&quad.predicate)
                    .map_or(false, |o| o.contains(&quad.object))
            })
        } else {
            indexes.spog.get(&quad.subject).map_or(false, |pog| {
                pog.get(&quad.predicate).map_or(false, |og| {
                    og.get(&quad.object)
                        .map_or(false, |g| g.contains(&quad.graph_name))
                })
            })
        }
    }

    fn encoded_quads_for_pattern_inner(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Vec<EncodedQuad> {
        match subject {
            Some(subject) => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            let quad = EncodedQuad::new(subject, predicate, object, graph_name);
                            if self.contains_encoded(&quad) {
                                vec![quad]
                            } else {
                                vec![]
                            }
                        }
                        None => self
                            .encoded_quads_for_subject_predicate_object(subject, predicate, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.encoded_quads_for_subject_predicate_graph(
                            subject, predicate, graph_name,
                        ),
                        None => self.encoded_quads_for_subject_predicate(subject, predicate),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            self.encoded_quads_for_subject_object_graph(subject, object, graph_name)
                        }
                        None => self.encoded_quads_for_subject_object(subject, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            self.encoded_quads_for_subject_graph(subject, graph_name)
                        }
                        None => self.encoded_quads_for_subject(subject),
                    },
                },
            },
            None => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self.encoded_quads_for_predicate_object_graph(
                            predicate, object, graph_name,
                        ),
                        None => self.encoded_quads_for_predicate_object(predicate, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            self.encoded_quads_for_predicate_graph(predicate, graph_name)
                        }
                        None => self.encoded_quads_for_predicate(predicate),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self.encoded_quads_for_object_graph(object, graph_name),
                        None => self.encoded_quads_for_object(object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.encoded_quads_for_graph(graph_name),
                        None => self.encoded_quads(),
                    },
                },
            },
        }
    }

    fn encoded_quads(&self) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        let default = triple_map_flatten(&indexes.default_spo)
            .map(|(s, p, o)| EncodedQuad::new(s, p, o, EncodedTerm::DefaultGraph));
        let named =
            quad_map_flatten(&indexes.gspo).map(|(g, s, p, o)| EncodedQuad::new(s, p, o, g));
        default.chain(named).collect()
    }

    fn encoded_quads_for_subject(&self, subject: EncodedTerm) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        let default = option_pair_map_flatten(indexes.default_spo.get(&subject))
            .map(|(p, o)| EncodedQuad::new(subject, p, o, EncodedTerm::DefaultGraph));
        let named = option_triple_map_flatten(indexes.spog.get(&subject))
            .map(|(p, o, g)| EncodedQuad::new(subject, p, o, g));
        default.chain(named).collect()
    }

    fn encoded_quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        let default = option_set_flatten(
            indexes
                .default_spo
                .get(&subject)
                .and_then(|po| po.get(&predicate)),
        )
        .map(|o| EncodedQuad::new(subject, predicate, o, EncodedTerm::DefaultGraph));
        let named = option_pair_map_flatten(
            indexes
                .spog
                .get(&subject)
                .and_then(|pog| pog.get(&predicate)),
        )
        .map(|(o, g)| EncodedQuad::new(subject, predicate, o, g));
        default.chain(named).collect()
    }

    fn encoded_quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        let default = indexes
            .default_spo
            .get(&subject)
            .and_then(|po| po.get(&predicate))
            .and_then(|o| o.get(&object))
            .map(|_| EncodedQuad::new(subject, predicate, object, EncodedTerm::DefaultGraph))
            .into_iter();
        let named = option_set_flatten(
            indexes
                .spog
                .get(&subject)
                .and_then(|pog| pog.get(&predicate))
                .and_then(|og| og.get(&object)),
        )
        .map(|g| EncodedQuad::new(subject, predicate, object, g));
        default.chain(named).collect()
    }

    fn encoded_quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        let default = option_set_flatten(
            indexes
                .default_osp
                .get(&object)
                .and_then(|sp| sp.get(&subject)),
        )
        .map(|p| EncodedQuad::new(subject, p, object, EncodedTerm::DefaultGraph));
        let named =
            option_pair_map_flatten(indexes.ospg.get(&object).and_then(|spg| spg.get(&subject)))
                .map(|(p, g)| EncodedQuad::new(subject, p, object, g));
        default.chain(named).collect()
    }

    fn encoded_quads_for_predicate(&self, predicate: EncodedTerm) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        let default = option_pair_map_flatten(indexes.default_pos.get(&predicate))
            .map(|(o, s)| EncodedQuad::new(s, predicate, o, EncodedTerm::DefaultGraph));
        let named = option_triple_map_flatten(indexes.posg.get(&predicate))
            .map(|(o, s, g)| EncodedQuad::new(s, predicate, o, g));
        default.chain(named).collect()
    }

    fn encoded_quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        let default = option_set_flatten(
            indexes
                .default_pos
                .get(&predicate)
                .and_then(|os| os.get(&object)),
        )
        .map(|s| EncodedQuad::new(s, predicate, object, EncodedTerm::DefaultGraph));
        let named = option_pair_map_flatten(
            indexes
                .posg
                .get(&predicate)
                .and_then(|osg| osg.get(&object)),
        )
        .map(|(s, g)| EncodedQuad::new(s, predicate, object, g));
        default.chain(named).collect()
    }

    fn encoded_quads_for_object(&self, object: EncodedTerm) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        let default = option_pair_map_flatten(indexes.default_osp.get(&object))
            .map(|(s, p)| EncodedQuad::new(s, p, object, EncodedTerm::DefaultGraph));
        let named = option_triple_map_flatten(indexes.ospg.get(&object))
            .map(|(s, p, g)| EncodedQuad::new(s, p, object, g));
        default.chain(named).collect()
    }

    fn encoded_quads_for_graph(&self, graph_name: EncodedTerm) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        option_triple_map_flatten(if graph_name.is_default_graph() {
            Some(&indexes.default_spo)
        } else {
            indexes.gspo.get(&graph_name)
        })
        .map(|(s, p, o)| EncodedQuad::new(s, p, o, graph_name))
        .collect()
    }

    fn encoded_quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        option_pair_map_flatten(if graph_name.is_default_graph() {
            indexes.default_spo.get(&subject)
        } else {
            indexes
                .gspo
                .get(&graph_name)
                .and_then(|spo| spo.get(&subject))
        })
        .map(|(p, o)| EncodedQuad::new(subject, p, o, graph_name))
        .collect()
    }

    fn encoded_quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        option_set_flatten(
            if graph_name.is_default_graph() {
                indexes.default_spo.get(&subject)
            } else {
                indexes
                    .gspo
                    .get(&graph_name)
                    .and_then(|spo| spo.get(&subject))
            }
            .and_then(|po| po.get(&predicate)),
        )
        .map(|o| EncodedQuad::new(subject, predicate, o, graph_name))
        .collect()
    }

    fn encoded_quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        option_set_flatten(
            if graph_name.is_default_graph() {
                indexes.default_osp.get(&object)
            } else {
                indexes
                    .gosp
                    .get(&graph_name)
                    .and_then(|osp| osp.get(&object))
            }
            .and_then(|sp| sp.get(&subject)),
        )
        .map(|p| EncodedQuad::new(subject, p, object, graph_name))
        .collect()
    }

    fn encoded_quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        option_pair_map_flatten(if graph_name.is_default_graph() {
            indexes.default_pos.get(&predicate)
        } else {
            indexes
                .gpos
                .get(&graph_name)
                .and_then(|pos| pos.get(&predicate))
        })
        .map(|(o, s)| EncodedQuad::new(s, predicate, o, graph_name))
        .collect()
    }

    fn encoded_quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        option_set_flatten(
            if graph_name.is_default_graph() {
                indexes.default_pos.get(&predicate)
            } else {
                indexes
                    .gpos
                    .get(&graph_name)
                    .and_then(|pos| pos.get(&predicate))
            }
            .and_then(|os| os.get(&object)),
        )
        .map(|s| EncodedQuad::new(s, predicate, object, graph_name))
        .collect()
    }

    fn encoded_quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let indexes = self.indexes();
        option_pair_map_flatten(if graph_name.is_default_graph() {
            indexes.default_osp.get(&object)
        } else {
            indexes
                .gosp
                .get(&graph_name)
                .and_then(|osp| osp.get(&object))
        })
        .map(|(s, p)| EncodedQuad::new(s, p, object, graph_name))
        .collect()
    }
}

impl StrEncodingAware for MemoryStore {
    type Error = Infallible;
    type StrId = LargeSpur;
}

impl StrLookup for MemoryStore {
    fn get_str(&self, id: LargeSpur) -> Result<Option<String>, Infallible> {
        //TODO: avoid copy by adding a lifetime limit to get_str
        Ok(self.strings.try_resolve(&id).map(|e| e.to_owned()))
    }

    fn get_str_id(&self, value: &str) -> Result<Option<LargeSpur>, Infallible> {
        Ok(self.strings.get(value))
    }
}

impl<'a> StrContainer for &'a MemoryStore {
    fn insert_str(&mut self, value: &str) -> Result<LargeSpur, Infallible> {
        Ok(self.strings.get_or_intern(value))
    }
}

impl<'a> ReadableEncodedStore for MemoryStore {
    type QuadsIter = EncodedQuadsIter;
    type GraphsIter = EncodedGraphsIter;

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> EncodedQuadsIter {
        EncodedQuadsIter {
            iter: self
                .encoded_quads_for_pattern_inner(subject, predicate, object, graph_name)
                .into_iter(),
        }
    }

    fn encoded_named_graphs(&self) -> Self::GraphsIter {
        EncodedGraphsIter {
            iter: self
                .indexes()
                .gosp
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .into_iter(),
        }
    }

    fn contains_encoded_named_graph(&self, graph_name: EncodedTerm) -> Result<bool, Infallible> {
        Ok(self.indexes().gspo.contains_key(&graph_name))
    }
}

impl<'a> WritableEncodedStore for &'a MemoryStore {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<(), Infallible> {
        self.indexes_mut().insert_encoded(quad)
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<(), Infallible> {
        self.indexes_mut().remove_encoded(quad)
    }

    fn insert_encoded_named_graph(&mut self, graph_name: EncodedTerm) -> Result<(), Infallible> {
        self.indexes_mut().insert_encoded_named_graph(graph_name)
    }

    fn clear_encoded_graph(&mut self, graph_name: EncodedTerm) -> Result<(), Infallible> {
        self.indexes_mut().clear_encoded_graph(graph_name)
    }

    fn remove_encoded_named_graph(&mut self, graph_name: EncodedTerm) -> Result<(), Infallible> {
        self.indexes_mut().remove_encoded_named_graph(graph_name)
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        self.indexes_mut().clear()
    }
}

impl StrEncodingAware for MemoryStoreIndexes {
    type Error = Infallible;
    type StrId = LargeSpur;
}

impl WritableEncodedStore for MemoryStoreIndexes {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<(), Infallible> {
        if quad.graph_name.is_default_graph() {
            insert_into_triple_map(
                &mut self.default_spo,
                quad.subject,
                quad.predicate,
                quad.object,
            );
            insert_into_triple_map(
                &mut self.default_pos,
                quad.predicate,
                quad.object,
                quad.subject,
            );
            insert_into_triple_map(
                &mut self.default_osp,
                quad.object,
                quad.subject,
                quad.predicate,
            );
        } else {
            insert_into_quad_map(
                &mut self.gspo,
                quad.graph_name,
                quad.subject,
                quad.predicate,
                quad.object,
            );
            insert_into_quad_map(
                &mut self.gpos,
                quad.graph_name,
                quad.predicate,
                quad.object,
                quad.subject,
            );
            insert_into_quad_map(
                &mut self.gosp,
                quad.graph_name,
                quad.object,
                quad.subject,
                quad.predicate,
            );
            insert_into_quad_map(
                &mut self.spog,
                quad.subject,
                quad.predicate,
                quad.object,
                quad.graph_name,
            );
            insert_into_quad_map(
                &mut self.posg,
                quad.predicate,
                quad.object,
                quad.subject,
                quad.graph_name,
            );
            insert_into_quad_map(
                &mut self.ospg,
                quad.object,
                quad.subject,
                quad.predicate,
                quad.graph_name,
            );
        }
        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<(), Infallible> {
        if quad.graph_name.is_default_graph() {
            remove_from_triple_map(
                &mut self.default_spo,
                &quad.subject,
                &quad.predicate,
                &quad.object,
            );
            remove_from_triple_map(
                &mut self.default_pos,
                &quad.predicate,
                &quad.object,
                &quad.subject,
            );
            remove_from_triple_map(
                &mut self.default_osp,
                &quad.object,
                &quad.subject,
                &quad.predicate,
            );
        } else {
            if let Some(spo) = self.gspo.get_mut(&quad.graph_name) {
                remove_from_triple_map(spo, &quad.subject, &quad.predicate, &quad.object);
            }
            if let Some(pos) = self.gpos.get_mut(&quad.graph_name) {
                remove_from_triple_map(pos, &quad.predicate, &quad.object, &quad.subject);
            }
            if let Some(osp) = self.gosp.get_mut(&quad.graph_name) {
                remove_from_triple_map(osp, &quad.object, &quad.subject, &quad.predicate);
            }
            remove_from_quad_map(
                &mut self.spog,
                &quad.subject,
                &quad.predicate,
                &quad.object,
                &quad.graph_name,
            );
            remove_from_quad_map(
                &mut self.posg,
                &quad.predicate,
                &quad.object,
                &quad.subject,
                &quad.graph_name,
            );
            remove_from_quad_map(
                &mut self.ospg,
                &quad.object,
                &quad.subject,
                &quad.predicate,
                &quad.graph_name,
            );
        }
        Ok(())
    }

    fn insert_encoded_named_graph(&mut self, graph_name: EncodedTerm) -> Result<(), Infallible> {
        self.gspo.entry(graph_name).or_default();
        self.gpos.entry(graph_name).or_default();
        self.gosp.entry(graph_name).or_default();
        Ok(())
    }

    fn clear_encoded_graph(&mut self, graph_name: EncodedTerm) -> Result<(), Infallible> {
        if graph_name.is_default_graph() {
            self.default_spo.clear();
            self.default_pos.clear();
            self.default_osp.clear();
        } else {
            if let Some(spo) = self.gspo.get(&graph_name) {
                for (s, po) in spo {
                    for (p, os) in po {
                        for o in os {
                            remove_from_quad_map(&mut self.spog, s, p, o, &graph_name);
                            remove_from_quad_map(&mut self.posg, p, o, s, &graph_name);
                            remove_from_quad_map(&mut self.ospg, o, s, p, &graph_name);
                        }
                    }
                }
            }
            if let Some(spo) = self.gspo.get_mut(&graph_name) {
                spo.clear();
            }
            if let Some(pos) = self.gpos.get_mut(&graph_name) {
                pos.clear();
            }
            if let Some(osp) = self.gosp.get_mut(&graph_name) {
                osp.clear();
            }
        }
        Ok(())
    }

    fn remove_encoded_named_graph(&mut self, graph_name: EncodedTerm) -> Result<(), Infallible> {
        if let Some(spo) = self.gspo.get(&graph_name) {
            for (s, po) in spo {
                for (p, os) in po {
                    for o in os {
                        remove_from_quad_map(&mut self.spog, s, p, o, &graph_name);
                        remove_from_quad_map(&mut self.posg, p, o, s, &graph_name);
                        remove_from_quad_map(&mut self.ospg, o, s, p, &graph_name);
                    }
                }
            }
        }
        self.gspo.remove(&graph_name);
        self.gpos.remove(&graph_name);
        self.gosp.remove(&graph_name);
        Ok(())
    }

    fn clear(&mut self) -> Result<(), Infallible> {
        *self = MemoryStoreIndexes::default();
        Ok(())
    }
}

fn insert_into_triple_map<T: Eq + Hash>(map: &mut TripleMap<T>, e1: T, e2: T, e3: T) {
    map.entry(e1).or_default().entry(e2).or_default().insert(e3);
}

fn insert_into_quad_map<T: Eq + Hash>(map: &mut QuadMap<T>, e1: T, e2: T, e3: T, e4: T) {
    insert_into_triple_map(map.entry(e1).or_default(), e2, e3, e4);
}

fn remove_from_triple_map<T: Eq + Hash>(map1: &mut TripleMap<T>, e1: &T, e2: &T, e3: &T) {
    let mut map2empty = false;
    if let Some(map2) = map1.get_mut(e1) {
        let mut set3empty = false;
        if let Some(set3) = map2.get_mut(e2) {
            set3.remove(e3);
            set3empty = set3.is_empty();
        }
        if set3empty {
            map2.remove(e2);
        }
        map2empty = map2.is_empty();
    }
    if map2empty {
        map1.remove(e1);
    }
}

fn remove_from_quad_map<T: Eq + Hash>(quad_map: &mut QuadMap<T>, e1: &T, e2: &T, e3: &T, e4: &T) {
    let mut triple_map_empty = false;
    if let Some(triple_map) = quad_map.get_mut(e1) {
        remove_from_triple_map(triple_map, e2, e3, e4);
        triple_map_empty = triple_map.is_empty();
    }
    if triple_map_empty {
        quad_map.remove(e1);
    }
}

fn option_set_flatten<T: Clone>(i: Option<&HashSet<T>>) -> impl Iterator<Item = T> + '_ {
    i.into_iter().flat_map(|s| s.iter().cloned())
}

fn option_pair_map_flatten<T: Copy>(
    i: Option<&HashMap<T, HashSet<T>>>,
) -> impl Iterator<Item = (T, T)> + '_ {
    i.into_iter().flat_map(|kv| {
        kv.iter().flat_map(|(k, vs)| {
            let k = *k;
            vs.iter().map(move |v| (k, *v))
        })
    })
}

fn triple_map_flatten<T: Copy>(spo: &TripleMap<T>) -> impl Iterator<Item = (T, T, T)> + '_ {
    spo.iter().flat_map(|(s, po)| {
        let s = *s;
        po.iter().flat_map(move |(p, os)| {
            let p = *p;
            os.iter().map(move |o| (s, p, *o))
        })
    })
}

fn option_triple_map_flatten<T: Copy>(
    i: Option<&TripleMap<T>>,
) -> impl Iterator<Item = (T, T, T)> + '_ {
    i.into_iter().flat_map(|spo| {
        spo.iter().flat_map(|(s, po)| {
            let s = *s;
            po.iter().flat_map(move |(p, os)| {
                let p = *p;
                os.iter().map(move |o| (s, p, *o))
            })
        })
    })
}

fn quad_map_flatten<T: Copy>(gspo: &QuadMap<T>) -> impl Iterator<Item = (T, T, T, T)> + '_ {
    gspo.iter().flat_map(|(g, spo)| {
        let g = *g;
        spo.iter().flat_map(move |(s, po)| {
            let s = *s;
            po.iter().flat_map(move |(p, os)| {
                let p = *p;
                os.iter().map(move |o| (g, s, p, *o))
            })
        })
    })
}

/// Allows inserting and deleting quads during an ACID transaction with the [`MemoryStore`].
pub struct MemoryTransaction {
    ops: Vec<TransactionOp>,
}

enum TransactionOp {
    Insert(Quad),
    Delete(Quad),
}

impl MemoryTransaction {
    /// Loads a graph file (i.e. triples) into the store during the transaction.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::*;
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.transaction(|transaction| {
    ///     transaction.load_graph(file.as_ref(), GraphFormat::NTriples, &GraphName::DefaultGraph, None)
    /// })?;
    ///
    /// // we inspect the store content
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, None)));
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// If the file parsing fails in the middle of the file, the triples read before are still
    /// considered by the transaction. Rollback the transaction by making the transaction closure
    /// return an error if you don't want that.
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    pub fn load_graph<'a>(
        &mut self,
        reader: impl BufRead,
        format: GraphFormat,
        to_graph_name: impl Into<GraphNameRef<'a>>,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        let to_graph_name = to_graph_name.into();
        let mut parser = GraphParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(invalid_input_error)?;
        }
        for triple in parser.read_triples(reader)? {
            self.ops
                .push(TransactionOp::Insert(triple?.in_graph(to_graph_name)));
        }
        Ok(())
    }

    /// Loads a dataset file (i.e. quads) into the store during the transaction.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::MemoryStore;
    /// use oxigraph::io::DatasetFormat;
    /// use oxigraph::model::*;
    ///
    /// let store = MemoryStore::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store.transaction(|transaction| {
    ///     transaction.load_dataset(file.as_ref(), DatasetFormat::NQuads, None)
    /// })?;    
    ///
    /// // we inspect the store content
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, ex)));
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// If the file parsing fails in the middle of the file, the quads read before are still
    /// considered by the transaction. Rollback the transaction by making the transaction closure
    /// return an error if you don't want that.
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    pub fn load_dataset(
        &mut self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        let mut parser = DatasetParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(invalid_input_error)?;
        }
        for quad in parser.read_quads(reader)? {
            self.ops.push(TransactionOp::Insert(quad?));
        }
        Ok(())
    }

    /// Adds a quad to this store during the transaction.
    #[allow(clippy::needless_pass_by_value)]
    pub fn insert(&mut self, quad: Quad) {
        self.ops.push(TransactionOp::Insert(quad))
    }

    /// Removes a quad from this store during the transaction.
    pub fn remove(&mut self, quad: Quad) {
        self.ops.push(TransactionOp::Delete(quad))
    }
}

impl PartialEq for MemoryStore {
    fn eq(&self, other: &Self) -> bool {
        self.indexes().spog == other.indexes().spog
    }
}

impl Eq for MemoryStore {}

impl FromIterator<Quad> for MemoryStore {
    fn from_iter<I: IntoIterator<Item = Quad>>(iter: I) -> Self {
        let mut store = MemoryStore::new();
        store.extend(iter);
        store
    }
}

impl Extend<Quad> for MemoryStore {
    fn extend<T: IntoIterator<Item = Quad>>(&mut self, iter: T) {
        for quad in iter {
            self.insert(quad);
        }
    }
}

impl IntoIterator for MemoryStore {
    type Item = Quad;
    type IntoIter = MemoryQuadIter;

    fn into_iter(self) -> MemoryQuadIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a MemoryStore {
    type Item = Quad;
    type IntoIter = MemoryQuadIter;

    fn into_iter(self) -> MemoryQuadIter {
        self.iter()
    }
}

impl fmt::Display for MemoryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self {
            writeln!(f, "{}", t)?;
        }
        Ok(())
    }
}

pub(crate) struct EncodedQuadsIter {
    iter: IntoIter<EncodedQuad>,
}

impl Iterator for EncodedQuadsIter {
    type Item = Result<EncodedQuad, Infallible>;

    fn next(&mut self) -> Option<Result<EncodedQuad, Infallible>> {
        self.iter.next().map(Ok)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn fold<Acc, G>(self, init: Acc, mut g: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.fold(init, |acc, elt| g(acc, Ok(elt)))
    }
}

pub(crate) struct EncodedGraphsIter {
    iter: IntoIter<EncodedTerm>,
}

impl Iterator for EncodedGraphsIter {
    type Item = Result<EncodedTerm, Infallible>;

    fn next(&mut self) -> Option<Result<EncodedTerm, Infallible>> {
        self.iter.next().map(Ok)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    fn fold<Acc, G>(self, init: Acc, mut g: G) -> Acc
    where
        G: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.fold(init, |acc, elt| g(acc, Ok(elt)))
    }
}

/// An iterator returning the quads contained in a [`MemoryStore`].
pub struct MemoryQuadIter {
    iter: IntoIter<EncodedQuad>,
    store: MemoryStore,
}

impl Iterator for MemoryQuadIter {
    type Item = Quad;

    fn next(&mut self) -> Option<Quad> {
        Some(self.store.decode_quad(&self.iter.next()?).unwrap())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

/// An iterator returning the graph names contained in a [`MemoryStore`].
pub struct MemoryGraphNameIter {
    iter: EncodedGraphsIter,
    store: MemoryStore,
}

impl Iterator for MemoryGraphNameIter {
    type Item = NamedOrBlankNode;

    fn next(&mut self) -> Option<NamedOrBlankNode> {
        Some(
            self.store
                .decode_named_or_blank_node(self.iter.next()?.unwrap_infallible())
                .unwrap(),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl StrId for LargeSpur {}

// Isomorphism implementation

fn iso_canonicalize(g: &MemoryStore) -> Vec<String> {
    let bnodes = bnodes(g);
    let (hash, partition) = hash_bnodes(g, bnodes.into_iter().map(|bnode| (bnode, 0)).collect());
    distinguish(g, &hash, &partition)
}

fn distinguish(
    g: &MemoryStore,
    hash: &HashMap<EncodedTerm, u64>,
    partition: &[(u64, Vec<EncodedTerm>)],
) -> Vec<String> {
    let b_prime = partition
        .iter()
        .find_map(|(_, b)| if b.len() > 1 { Some(b) } else { None });
    if let Some(b_prime) = b_prime {
        b_prime
            .iter()
            .map(|b| {
                let mut hash_prime = hash.clone();
                hash_prime.insert(*b, hash_tuple((hash_prime[b], 22)));
                let (hash_prime_prime, partition_prime) = hash_bnodes(g, hash_prime);
                distinguish(g, &hash_prime_prime, &partition_prime)
            })
            .fold(None, |a, b| {
                Some(if let Some(a) = a {
                    if a <= b {
                        a
                    } else {
                        b
                    }
                } else {
                    b
                })
            })
            .unwrap_or_else(Vec::new)
    } else {
        label(g, hash)
    }
}

fn hash_bnodes(
    g: &MemoryStore,
    mut hashes: HashMap<EncodedTerm, u64>,
) -> (HashMap<EncodedTerm, u64>, Vec<(u64, Vec<EncodedTerm>)>) {
    let mut to_hash = Vec::new();
    let mut partition: HashMap<u64, Vec<EncodedTerm>> = HashMap::new();
    let mut partition_len = 0;
    loop {
        //TODO: improve termination
        let mut new_hashes = HashMap::new();
        for (bnode, old_hash) in &hashes {
            for q in g.encoded_quads_for_subject(*bnode) {
                to_hash.push((
                    hash_term(q.predicate, &hashes, g),
                    hash_term(q.object, &hashes, g),
                    hash_term(q.graph_name, &hashes, g),
                    0,
                ));
            }
            for q in g.encoded_quads_for_object(*bnode) {
                to_hash.push((
                    hash_term(q.subject, &hashes, g),
                    hash_term(q.predicate, &hashes, g),
                    hash_term(q.graph_name, &hashes, g),
                    1,
                ));
            }
            for q in g.encoded_quads_for_graph(*bnode) {
                to_hash.push((
                    hash_term(q.subject, &hashes, g),
                    hash_term(q.predicate, &hashes, g),
                    hash_term(q.object, &hashes, g),
                    2,
                ));
            }
            to_hash.sort_unstable();
            let hash = hash_tuple((old_hash, &to_hash));
            to_hash.clear();
            new_hashes.insert(*bnode, hash);
            partition.entry(hash).or_default().push(*bnode);
        }
        if partition.len() == partition_len {
            let mut partition: Vec<_> = partition.into_iter().collect();
            partition.sort_by(|(h1, b1), (h2, b2)| (b1.len(), h1).cmp(&(b2.len(), h2)));
            return (hashes, partition);
        }
        hashes = new_hashes;
        partition_len = partition.len();
        partition.clear();
    }
}

fn bnodes(g: &MemoryStore) -> HashSet<EncodedTerm> {
    let mut bnodes = HashSet::new();
    for q in g.encoded_quads() {
        if q.subject.is_blank_node() {
            bnodes.insert(q.subject);
        }
        if q.object.is_blank_node() {
            bnodes.insert(q.object);
        }
        if q.graph_name.is_blank_node() {
            bnodes.insert(q.graph_name);
        }
    }
    bnodes
}

fn label(g: &MemoryStore, hashes: &HashMap<EncodedTerm, u64>) -> Vec<String> {
    //TODO: better representation?
    let mut data: Vec<_> = g
        .encoded_quads()
        .into_iter()
        .map(|q| {
            g.decode_quad(&EncodedQuad {
                subject: map_term(q.subject, hashes),
                predicate: map_term(q.predicate, hashes),
                object: map_term(q.object, hashes),
                graph_name: map_term(q.graph_name, hashes),
            })
            .unwrap()
            .to_string()
        })
        .collect();
    data.sort();
    data
}

fn map_term(term: EncodedTerm, bnodes_hash: &HashMap<EncodedTerm, u64>) -> EncodedTerm {
    if term.is_blank_node() {
        EncodedTerm::NumericalBlankNode {
            id: (*bnodes_hash.get(&term).unwrap()).into(),
        }
    } else {
        term
    }
}

fn hash_term(term: EncodedTerm, bnodes_hash: &HashMap<EncodedTerm, u64>, g: &MemoryStore) -> u64 {
    if term.is_blank_node() {
        *bnodes_hash.get(&term).unwrap()
    } else if let Ok(term) = g.decode_term(term) {
        hash_tuple(term)
    } else {
        0
    }
}

fn hash_tuple(v: impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    v.hash(&mut hasher);
    hasher.finish()
}
