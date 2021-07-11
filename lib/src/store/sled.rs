//! Store based on the [Sled](https://sled.rs/) key-value database.

use crate::error::invalid_data_error;
use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use crate::sparql::{
    evaluate_query, evaluate_update, EvaluationError, Query, QueryOptions, QueryResults, Update,
    UpdateOptions,
};
use crate::store::binary_encoder::*;
use crate::store::numeric_encoder::{
    Decoder, ReadEncoder, StrContainer, StrEncodingAware, StrLookup, WriteEncoder,
};
use crate::store::{
    dump_dataset, dump_graph, get_encoded_quad_pattern, load_dataset, load_graph,
    ReadableEncodedStore, StoreOrParseError, WritableEncodedStore,
};
use sled::transaction::{
    ConflictableTransactionError, TransactionError, Transactional, TransactionalTree,
    UnabortableTransactionError,
};
use sled::{Config, Db, Iter, Tree};
use std::convert::TryInto;
use std::error::Error;
use std::io::{BufRead, Write};
use std::iter::{once, Once};
use std::path::Path;
use std::{fmt, io, str};

/// Store based on the [Sled](https://sled.rs/) key-value database.
/// It encodes a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) and allows to query it using SPARQL.
///
/// To use it, the `"sled"` feature needs to be activated.
///
/// Warning: Sled is not stable yet and might break its storage format.
///
/// Usage example:
/// ```
/// use oxigraph::SledStore;
/// use oxigraph::sparql::QueryResults;
/// use oxigraph::model::*;
/// # use std::fs::remove_dir_all;
///
/// # {
/// let store = SledStore::open("example.db")?;
///
/// // insertion
/// let ex = NamedNode::new("http://example.com")?;
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
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
pub struct SledStore {
    default: Db,
    id2str: Tree,
    spog: Tree,
    posg: Tree,
    ospg: Tree,
    gspo: Tree,
    gpos: Tree,
    gosp: Tree,
    dspo: Tree,
    dpos: Tree,
    dosp: Tree,
    graphs: Tree,
}

type EncodedTerm = crate::store::numeric_encoder::EncodedTerm<StrHash>;
type EncodedQuad = crate::store::numeric_encoder::EncodedQuad<StrHash>;

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

impl SledStore {
    /// Creates a temporary [`SledStore`]() that will be deleted after drop.
    pub fn new() -> Result<Self, io::Error> {
        Self::do_open(&Config::new().temporary(true))
    }

    /// Opens a [`SledStore`]() and creates it if it does not exist yet.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        Self::do_open(&Config::new().path(path))
    }

    fn do_open(config: &Config) -> Result<Self, io::Error> {
        let db = config.open()?;
        let this = Self {
            default: db.clone(),
            id2str: db.open_tree("id2str")?,
            spog: db.open_tree("spog")?,
            posg: db.open_tree("posg")?,
            ospg: db.open_tree("ospg")?,
            gspo: db.open_tree("gspo")?,
            gpos: db.open_tree("gpos")?,
            gosp: db.open_tree("gosp")?,
            dspo: db.open_tree("dspo")?,
            dpos: db.open_tree("dpos")?,
            dosp: db.open_tree("dosp")?,
            graphs: db.open_tree("graphs")?,
        };

        let mut version = this.ensure_version()?;
        if version == 0 {
            // We migrate to v1
            for quad in this.encoded_quads_for_pattern(None, None, None, None) {
                let mut this_mut = &this;
                let quad = quad?;
                if !quad.graph_name.is_default_graph() {
                    this_mut.insert_encoded_named_graph(quad.graph_name)?;
                }
            }
            version = 1;
            this.set_version(version)?;
            this.graphs.flush()?;
        }

        match version {
            _ if version < LATEST_STORAGE_VERSION => Err(invalid_data_error(format!(
                "The Sled database is using the outdated encoding version {}. Automated migration is not supported, please dump the store dataset using a compatible Oxigraph version and load it again using the current version",
                version
            ))),
            LATEST_STORAGE_VERSION => Ok(this),
            _ => Err(invalid_data_error(format!(
                "The Sled database is using the too recent version {}. Upgrade to the latest Oxigraph version to load this database",
                version
            )))
        }
    }

    fn ensure_version(&self) -> Result<u64, io::Error> {
        Ok(if let Some(version) = self.default.get("oxversion")? {
            let mut buffer = [0; 8];
            buffer.copy_from_slice(&version);
            u64::from_be_bytes(buffer)
        } else {
            self.set_version(LATEST_STORAGE_VERSION)?;
            LATEST_STORAGE_VERSION
        })
    }

    fn set_version(&self, version: u64) -> Result<(), io::Error> {
        self.default.insert("oxversion", &version.to_be_bytes())?;
        Ok(())
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/).
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::QueryResults;
    ///
    /// let store = SledStore::new()?;
    ///
    /// // insertions
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// store.insert(QuadRef::new(ex, ex, ex, None))?;
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
        evaluate_query(self.clone(), query, options)
    }

    /// Retrieves quads with a filter on each quad component
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::model::*;
    ///
    /// let store = SledStore::new()?;
    ///
    /// // insertion
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
    /// store.insert(&quad)?;
    ///
    /// // quad filter by object
    /// let results = store.quads_for_pattern(None, None, Some((&ex).into()), None).collect::<Result<Vec<_>,_>>()?;
    /// assert_eq!(vec![quad], results);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn quads_for_pattern(
        &self,
        subject: Option<NamedOrBlankNodeRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
        graph_name: Option<GraphNameRef<'_>>,
    ) -> SledQuadIter {
        SledQuadIter {
            inner: match get_encoded_quad_pattern(self, subject, predicate, object, graph_name) {
                Ok(Some((subject, predicate, object, graph_name))) => QuadIterInner::Quads {
                    iter: self.encoded_quads_for_pattern(subject, predicate, object, graph_name),
                    store: self.clone(),
                },
                Ok(None) => QuadIterInner::Empty,
                Err(error) => QuadIterInner::Error(once(error)),
            },
        }
    }

    /// Returns all the quads contained in the store
    pub fn iter(&self) -> SledQuadIter {
        self.quads_for_pattern(None, None, None, None)
    }

    /// Checks if this store contains a given quad
    pub fn contains<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<bool, io::Error> {
        if let Some(quad) = self.get_encoded_quad(quad.into())? {
            self.contains_encoded(&quad)
        } else {
            Ok(false)
        }
    }

    /// Returns the number of quads in the store
    ///
    /// Warning: this function executes a full scan
    pub fn len(&self) -> usize {
        self.gspo.len() + self.dspo.len()
    }

    /// Returns if the store is empty
    pub fn is_empty(&self) -> bool {
        self.gspo.is_empty() && self.dspo.is_empty()
    }

    /// Executes a [SPARQL 1.1 update](https://www.w3.org/TR/sparql11-update/).
    ///
    /// The store does not track the existence of empty named graphs.
    /// This method has no ACID guarantees.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::model::*;
    ///
    /// let store = SledStore::new()?;
    ///
    /// // insertion
    /// store.update("INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }")?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com").unwrap();
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, None))?);
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
    /// The transaction is rollbacked if the closure returns `Err`.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::model::*;
    /// use oxigraph::store::sled::SledConflictableTransactionError;
    /// use std::convert::Infallible;
    ///
    /// let store = SledStore::new()?;
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let quad = QuadRef::new(ex, ex, ex, ex);
    ///
    /// // transaction
    /// store.transaction(|transaction| {
    ///     transaction.insert(quad)?;
    ///     Ok(()) as Result<(),SledConflictableTransactionError<Infallible>>
    /// })?;
    ///
    /// assert!(store.contains(quad)?);
    /// assert!(store.contains_named_graph(ex)?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn transaction<T, E>(
        &self,
        f: impl Fn(SledTransaction<'_>) -> Result<T, SledConflictableTransactionError<E>>,
    ) -> Result<T, SledTransactionError<E>> {
        Ok((
            &self.id2str,
            &self.spog,
            &self.posg,
            &self.ospg,
            &self.gspo,
            &self.gpos,
            &self.gosp,
            &self.dspo,
            &self.dpos,
            &self.dosp,
            &self.graphs,
        )
            .transaction(
                move |(id2str, spog, posg, ospg, gspo, gpos, gosp, dspo, dpos, dosp, graphs)| {
                    Ok(f(SledTransaction {
                        id2str,
                        spog,
                        posg,
                        ospg,
                        gspo,
                        gpos,
                        gosp,
                        dspo,
                        dpos,
                        dosp,
                        graphs,
                    })?)
                },
            )?)
    }

    /// Loads a graph file (i.e. triples) into the store
    ///
    /// Warning: This functions saves the triples in a not atomic way.
    /// If the parsing fails in the middle of the file only a part of it may be written to the store.
    /// It might leave the store in a bad state if a crash happens during a triple insertion.
    /// Use a (memory greedy) [transaction](SledStore::transaction()) if you do not want that.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::*;
    ///
    /// let store = SledStore::new()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.load_graph(file.as_ref(), GraphFormat::NTriples, &GraphName::DefaultGraph, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, None))?);
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
        let mut this = self;
        load_graph(&mut this, reader, format, to_graph_name.into(), base_iri)?;
        Ok(())
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// Warning: This functions saves the triples in a not atomic way.
    /// If the parsing fails in the middle of the file, only a part of it may be written to the store.
    /// It might leave the store in a bad state if a crash happens during a quad insertion.
    /// Use a (memory greedy) [transaction](SledStore::transaction()) if you do not want that.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::io::DatasetFormat;
    /// use oxigraph::model::*;
    ///
    /// let store = SledStore::new()?;
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
        let mut this = self;
        load_dataset(&mut this, reader, format, base_iri)?;
        Ok(())
    }

    /// Adds a quad to this store.
    ///
    /// This method is optimized for performances and is not atomic.
    /// It might leave the store in a bad state if a crash happens during the insertion.
    /// Use a (memory greedy) [transaction](SledStore::transaction()) if you do not want that.
    pub fn insert<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<(), io::Error> {
        let mut this = self;
        let quad = this.encode_quad(quad.into())?;
        this.insert_encoded(&quad)
    }

    /// Removes a quad from this store.
    ///
    /// This method is optimized for performances and is not atomic.
    /// It might leave the store in a bad state if a crash happens during the removal.
    /// Use a (memory greedy) [transaction](SledStore::transaction()) if you do not want that.
    pub fn remove<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<(), io::Error> {
        if let Some(quad) = self.get_encoded_quad(quad.into())? {
            let mut this = self;
            this.remove_encoded(&quad)
        } else {
            Ok(())
        }
    }

    /// Dumps a store graph into a file.
    ///    
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::GraphName;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = SledStore::new()?;
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
    /// use oxigraph::SledStore;
    /// use oxigraph::io::DatasetFormat;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let store = SledStore::new()?;
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
    /// use oxigraph::SledStore;
    /// use oxigraph::model::{NamedNode, QuadRef, NamedOrBlankNode};
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let store = SledStore::new()?;
    /// store.insert(QuadRef::new(&ex, &ex, &ex, &ex))?;
    /// store.insert(QuadRef::new(&ex, &ex, &ex, None))?;
    /// assert_eq!(vec![NamedOrBlankNode::from(ex)], store.named_graphs().collect::<Result<Vec<_>,_>>()?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn named_graphs(&self) -> SledGraphNameIter {
        SledGraphNameIter {
            iter: self.encoded_named_graphs(),
            store: self.clone(),
        }
    }

    /// Checks if the store contains a given graph
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::model::{NamedNode, QuadRef};
    ///
    /// let ex = NamedNode::new("http://example.com")?;
    /// let store = SledStore::new()?;
    /// store.insert(QuadRef::new(&ex, &ex, &ex, &ex))?;
    /// assert!(store.contains_named_graph(&ex)?);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn contains_named_graph<'a>(
        &self,
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Result<bool, io::Error> {
        if let Some(graph_name) = self.get_encoded_named_or_blank_node(graph_name.into())? {
            self.contains_encoded_named_graph(graph_name)
        } else {
            Ok(false)
        }
    }

    /// Inserts a graph into this store
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::model::NamedNodeRef;
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let store = SledStore::new()?;
    /// store.insert_named_graph(ex)?;
    /// assert_eq!(store.named_graphs().count(), 1);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn insert_named_graph<'a>(
        &self,
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Result<(), io::Error> {
        let mut this = self;
        let graph_name = this.encode_named_or_blank_node(graph_name.into())?;
        this.insert_encoded_named_graph(graph_name)
    }

    /// Clears a graph from this store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::model::{NamedNodeRef, QuadRef};
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let quad = QuadRef::new(ex, ex, ex, ex);
    /// let store = SledStore::new()?;
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
        if let Some(graph_name) = self.get_encoded_graph_name(graph_name.into())? {
            let mut this = self;
            this.clear_encoded_graph(graph_name)
        } else {
            Ok(())
        }
    }

    /// Removes a graph from this store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::model::{NamedNodeRef, QuadRef};
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let quad = QuadRef::new(ex, ex, ex, ex);
    /// let store = SledStore::new()?;
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
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Result<(), io::Error> {
        if let Some(graph_name) = self.get_encoded_named_or_blank_node(graph_name.into())? {
            let mut this = self;
            this.remove_encoded_named_graph(graph_name)
        } else {
            Ok(())
        }
    }

    /// Clears the store.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::model::{NamedNodeRef, QuadRef};
    ///
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// let store = SledStore::new()?;
    /// store.insert(QuadRef::new(ex, ex, ex, ex))?;
    /// store.insert(QuadRef::new(ex, ex, ex, None))?;    
    /// assert_eq!(2, store.len());
    ///
    /// store.clear()?;
    /// assert!(store.is_empty());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn clear(&self) -> Result<(), io::Error> {
        let mut this = self;
        (&mut this).clear()
    }

    /// Flushes all buffers and ensures that all writes are saved on disk.
    ///
    /// Flushes are automatically done for most platform using background threads.
    /// However, calling this method explicitly is still required for Windows and Android.
    ///
    /// An [async version](SledStore::flush_async) is also available.
    pub fn flush(&self) -> Result<(), io::Error> {
        self.default.flush()?;
        Ok(())
    }

    /// Asynchronously flushes all buffers and ensures that all writes are saved on disk.
    ///
    /// Flushes are automatically done for most platform using background threads.
    /// However, calling this method explicitly is still required for Windows and Android.
    ///
    /// A [sync version](SledStore::flush) is also available.
    pub async fn flush_async(&self) -> Result<(), io::Error> {
        self.default.flush_async().await?;
        Ok(())
    }

    fn contains_encoded(&self, quad: &EncodedQuad) -> Result<bool, io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            Ok(self.dspo.contains_key(buffer)?)
        } else {
            write_gspo_quad(&mut buffer, quad);
            Ok(self.gspo.contains_key(buffer)?)
        }
    }
    fn quads(&self) -> DecodingQuadsIterator {
        DecodingQuadsIterator::pair(
            self.dspo_quads(Vec::default()),
            self.gspo_quads(Vec::default()),
        )
    }

    fn quads_for_subject(&self, subject: EncodedTerm) -> DecodingQuadsIterator {
        DecodingQuadsIterator::pair(
            self.dspo_quads(encode_term(subject)),
            self.spog_quads(encode_term(subject)),
        )
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::pair(
            self.dspo_quads(encode_term_pair(subject, predicate)),
            self.spog_quads(encode_term_pair(subject, predicate)),
        )
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::pair(
            self.dspo_quads(encode_term_triple(subject, predicate, object)),
            self.spog_quads(encode_term_triple(subject, predicate, object)),
        )
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::pair(
            self.dosp_quads(encode_term_pair(object, subject)),
            self.ospg_quads(encode_term_pair(object, subject)),
        )
    }

    fn quads_for_predicate(&self, predicate: EncodedTerm) -> DecodingQuadsIterator {
        DecodingQuadsIterator::pair(
            self.dpos_quads(encode_term(predicate)),
            self.posg_quads(encode_term(predicate)),
        )
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::pair(
            self.dpos_quads(encode_term_pair(predicate, object)),
            self.posg_quads(encode_term_pair(predicate, object)),
        )
    }

    fn quads_for_object(&self, object: EncodedTerm) -> DecodingQuadsIterator {
        DecodingQuadsIterator::pair(
            self.dosp_quads(encode_term(object)),
            self.ospg_quads(encode_term(object)),
        )
    }

    fn quads_for_graph(&self, graph_name: EncodedTerm) -> DecodingQuadsIterator {
        DecodingQuadsIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(Vec::default())
        } else {
            self.gspo_quads(encode_term(graph_name))
        })
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(encode_term(subject))
        } else {
            self.gspo_quads(encode_term_pair(graph_name, subject))
        })
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(encode_term_pair(subject, predicate))
        } else {
            self.gspo_quads(encode_term_triple(graph_name, subject, predicate))
        })
    }

    fn quads_for_subject_predicate_object_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(encode_term_triple(subject, predicate, object))
        } else {
            self.gspo_quads(encode_term_quad(graph_name, subject, predicate, object))
        })
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(encode_term_pair(object, subject))
        } else {
            self.gosp_quads(encode_term_triple(graph_name, object, subject))
        })
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(encode_term(predicate))
        } else {
            self.gpos_quads(encode_term_pair(graph_name, predicate))
        })
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(encode_term_pair(predicate, object))
        } else {
            self.gpos_quads(encode_term_triple(graph_name, predicate, object))
        })
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingQuadsIterator {
        DecodingQuadsIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(encode_term(object))
        } else {
            self.gosp_quads(encode_term_pair(graph_name, object))
        })
    }

    fn spog_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.spog, prefix, QuadEncoding::Spog)
    }

    fn posg_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.posg, prefix, QuadEncoding::Posg)
    }

    fn ospg_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.ospg, prefix, QuadEncoding::Ospg)
    }

    fn gspo_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gspo, prefix, QuadEncoding::Gspo)
    }

    fn gpos_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gpos, prefix, QuadEncoding::Gpos)
    }

    fn gosp_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gosp, prefix, QuadEncoding::Gosp)
    }

    fn dspo_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.dspo, prefix, QuadEncoding::Dspo)
    }

    fn dpos_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.dpos, prefix, QuadEncoding::Dpos)
    }

    fn dosp_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.dosp, prefix, QuadEncoding::Dosp)
    }

    fn inner_quads(
        &self,
        tree: &Tree,
        prefix: impl AsRef<[u8]>,
        encoding: QuadEncoding,
    ) -> DecodingQuadIterator {
        DecodingQuadIterator {
            iter: tree.scan_prefix(prefix),
            encoding,
        }
    }
}

impl fmt::Display for SledStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self.iter() {
            writeln!(f, "{}", t.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

impl StrEncodingAware for SledStore {
    type Error = io::Error;
    type StrId = StrHash;
}

impl StrLookup for SledStore {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, io::Error> {
        self.id2str
            .get(id.to_be_bytes())?
            .map(|v| String::from_utf8(v.to_vec()))
            .transpose()
            .map_err(invalid_data_error)
    }

    fn get_str_id(&self, value: &str) -> Result<Option<StrHash>, io::Error> {
        let id = StrHash::new(value);
        Ok(if self.id2str.contains_key(&id.to_be_bytes())? {
            Some(id)
        } else {
            None
        })
    }
}

impl ReadableEncodedStore for SledStore {
    type QuadsIter = DecodingQuadsIterator;
    type GraphsIter = DecodingGraphIterator;

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> DecodingQuadsIterator {
        match subject {
            Some(subject) => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self.quads_for_subject_predicate_object_graph(
                            subject, predicate, object, graph_name,
                        ),
                        None => self.quads_for_subject_predicate_object(subject, predicate, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            self.quads_for_subject_predicate_graph(subject, predicate, graph_name)
                        }
                        None => self.quads_for_subject_predicate(subject, predicate),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            self.quads_for_subject_object_graph(subject, object, graph_name)
                        }
                        None => self.quads_for_subject_object(subject, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.quads_for_subject_graph(subject, graph_name),
                        None => self.quads_for_subject(subject),
                    },
                },
            },
            None => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            self.quads_for_predicate_object_graph(predicate, object, graph_name)
                        }
                        None => self.quads_for_predicate_object(predicate, object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.quads_for_predicate_graph(predicate, graph_name),
                        None => self.quads_for_predicate(predicate),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self.quads_for_object_graph(object, graph_name),
                        None => self.quads_for_object(object),
                    },
                    None => match graph_name {
                        Some(graph_name) => self.quads_for_graph(graph_name),
                        None => self.quads(),
                    },
                },
            },
        }
    }

    fn encoded_named_graphs(&self) -> DecodingGraphIterator {
        DecodingGraphIterator {
            iter: self.graphs.iter(),
        }
    }

    fn contains_encoded_named_graph(&self, graph_name: EncodedTerm) -> Result<bool, io::Error> {
        Ok(self.graphs.contains_key(&encode_term(graph_name))?)
    }
}

impl<'a> StrContainer for &'a SledStore {
    fn insert_str(&mut self, value: &str) -> Result<StrHash, io::Error> {
        let key = StrHash::new(value);
        self.id2str.insert(key.to_be_bytes().as_ref(), value)?;
        Ok(key)
    }
}

impl<'a> WritableEncodedStore for &'a SledStore {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<(), io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            self.dspo.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_pos_quad(&mut buffer, quad);
            self.dpos.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_osp_quad(&mut buffer, quad);
            self.dosp.insert(buffer.as_slice(), &[])?;
        } else {
            write_spog_quad(&mut buffer, quad);
            self.spog.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_posg_quad(&mut buffer, quad);
            self.posg.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_ospg_quad(&mut buffer, quad);
            self.ospg.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_gspo_quad(&mut buffer, quad);
            self.gspo.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_gpos_quad(&mut buffer, quad);
            self.gpos.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_gosp_quad(&mut buffer, quad);
            self.gosp.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_term(&mut buffer, quad.graph_name);
            self.graphs.insert(&buffer, &[])?;
        }

        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<(), io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            self.dspo.remove(buffer.as_slice())?;
            buffer.clear();

            write_pos_quad(&mut buffer, quad);
            self.dpos.remove(buffer.as_slice())?;
            buffer.clear();

            write_osp_quad(&mut buffer, quad);
            self.dosp.remove(buffer.as_slice())?;
        } else {
            write_spog_quad(&mut buffer, quad);
            self.spog.remove(buffer.as_slice())?;
            buffer.clear();

            write_posg_quad(&mut buffer, quad);
            self.posg.remove(buffer.as_slice())?;
            buffer.clear();

            write_ospg_quad(&mut buffer, quad);
            self.ospg.remove(buffer.as_slice())?;
            buffer.clear();

            write_gspo_quad(&mut buffer, quad);
            self.gspo.remove(buffer.as_slice())?;
            buffer.clear();

            write_gpos_quad(&mut buffer, quad);
            self.gpos.remove(buffer.as_slice())?;
            buffer.clear();

            write_gosp_quad(&mut buffer, quad);
            self.gosp.remove(buffer.as_slice())?;
        }

        Ok(())
    }

    fn insert_encoded_named_graph(&mut self, graph_name: EncodedTerm) -> Result<(), io::Error> {
        self.graphs.insert(&encode_term(graph_name), &[])?;
        Ok(())
    }

    fn clear_encoded_graph(&mut self, graph_name: EncodedTerm) -> Result<(), io::Error> {
        if graph_name.is_default_graph() {
            self.dspo.clear()?;
            self.dpos.clear()?;
            self.dosp.clear()?;
        } else {
            for quad in self.quads_for_graph(graph_name) {
                self.remove_encoded(&quad?)?;
            }
        }
        Ok(())
    }

    fn remove_encoded_named_graph(&mut self, graph_name: EncodedTerm) -> Result<(), io::Error> {
        for quad in self.quads_for_graph(graph_name) {
            self.remove_encoded(&quad?)?;
        }
        self.graphs.remove(&encode_term(graph_name))?;
        Ok(())
    }

    fn clear(&mut self) -> Result<(), io::Error> {
        self.dspo.clear()?;
        self.dpos.clear()?;
        self.dosp.clear()?;
        self.gspo.clear()?;
        self.gpos.clear()?;
        self.gosp.clear()?;
        self.spog.clear()?;
        self.posg.clear()?;
        self.ospg.clear()?;
        self.graphs.clear()?;
        self.id2str.clear()?;
        Ok(())
    }
}

/// Allows inserting and deleting quads during an ACID transaction with the [`SledStore`].
pub struct SledTransaction<'a> {
    id2str: &'a TransactionalTree,
    spog: &'a TransactionalTree,
    posg: &'a TransactionalTree,
    ospg: &'a TransactionalTree,
    gspo: &'a TransactionalTree,
    gpos: &'a TransactionalTree,
    gosp: &'a TransactionalTree,
    dspo: &'a TransactionalTree,
    dpos: &'a TransactionalTree,
    dosp: &'a TransactionalTree,
    graphs: &'a TransactionalTree,
}

impl SledTransaction<'_> {
    /// Loads a graph file (i.e. triples) into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content might be temporarily stored in main memory.
    /// Do not use for big files.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::SledStore;
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::sled::SledConflictableTransactionError;
    ///
    /// let store = SledStore::new()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// store.transaction(|transaction| {
    ///     transaction.load_graph(file.as_ref(), GraphFormat::NTriples, &GraphName::DefaultGraph, None)?;
    ///     Ok(()) as Result<(),SledConflictableTransactionError<std::io::Error>>
    /// })?;
    ///
    /// // we inspect the store content
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(store.contains(QuadRef::new(ex, ex, ex, None))?);
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
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut this = self;
        load_graph(&mut this, reader, format, to_graph_name.into(), base_iri)?;
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
    /// use oxigraph::SledStore;
    /// use oxigraph::io::DatasetFormat;
    /// use oxigraph::model::*;
    /// use oxigraph::store::sled::SledConflictableTransactionError;
    ///
    /// let store = SledStore::new()?;
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> <http://example.com> .";
    /// store.transaction(|transaction| {
    ///     transaction.load_dataset(file.as_ref(), DatasetFormat::NQuads, None)?;
    ///     Ok(()) as Result<(),SledConflictableTransactionError<std::io::Error>>
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
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut this = self;
        load_dataset(&mut this, reader, format, base_iri)?;
        Ok(())
    }

    /// Adds a quad to this store during the transaction.
    pub fn insert<'a>(
        &self,
        quad: impl Into<QuadRef<'a>>,
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut this = self;
        let quad = this.encode_quad(quad.into())?;
        this.insert_encoded(&quad)
    }

    /// Removes a quad from this store during the transaction.
    pub fn remove<'a>(
        &self,
        quad: impl Into<QuadRef<'a>>,
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut this = self;
        if let Some(quad) = this.get_encoded_quad(quad.into())? {
            this.remove_encoded(&quad)
        } else {
            Ok(())
        }
    }
}

impl<'a> StrEncodingAware for &'a SledTransaction<'a> {
    type Error = SledUnabortableTransactionError;
    type StrId = StrHash;
}

impl<'a> StrLookup for &'a SledTransaction<'a> {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, SledUnabortableTransactionError> {
        self.id2str
            .get(id.to_be_bytes())?
            .map(|v| String::from_utf8(v.to_vec()))
            .transpose()
            .map_err(|e| SledUnabortableTransactionError::Storage(invalid_data_error(e)))
    }

    fn get_str_id(&self, value: &str) -> Result<Option<StrHash>, SledUnabortableTransactionError> {
        let id = StrHash::new(value);
        Ok(if self.id2str.get(&id.to_be_bytes())?.is_some() {
            Some(id)
        } else {
            None
        })
    }
}

impl<'a> StrContainer for &'a SledTransaction<'a> {
    fn insert_str(&mut self, value: &str) -> Result<StrHash, SledUnabortableTransactionError> {
        let key = StrHash::new(value);
        self.id2str.insert(key.to_be_bytes().as_ref(), value)?;
        Ok(key)
    }
}

impl<'a> WritableEncodedStore for &'a SledTransaction<'a> {
    fn insert_encoded(
        &mut self,
        quad: &EncodedQuad,
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            self.dspo.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_pos_quad(&mut buffer, quad);
            self.dpos.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_osp_quad(&mut buffer, quad);
            self.dosp.insert(buffer.as_slice(), &[])?;
        } else {
            write_spog_quad(&mut buffer, quad);
            self.spog.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_posg_quad(&mut buffer, quad);
            self.posg.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_ospg_quad(&mut buffer, quad);
            self.ospg.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_gspo_quad(&mut buffer, quad);
            self.gspo.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_gpos_quad(&mut buffer, quad);
            self.gpos.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_gosp_quad(&mut buffer, quad);
            self.gosp.insert(buffer.as_slice(), &[])?;
            buffer.clear();

            write_term(&mut buffer, quad.graph_name);
            self.graphs.insert(buffer.as_slice(), &[])?;
        }
        buffer.clear();

        Ok(())
    }

    fn remove_encoded(
        &mut self,
        quad: &EncodedQuad,
    ) -> Result<(), SledUnabortableTransactionError> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            self.dspo.remove(buffer.as_slice())?;
            buffer.clear();

            write_pos_quad(&mut buffer, quad);
            self.dpos.remove(buffer.as_slice())?;
            buffer.clear();

            write_osp_quad(&mut buffer, quad);
            self.dosp.remove(buffer.as_slice())?;
        } else {
            write_spog_quad(&mut buffer, quad);
            self.spog.remove(buffer.as_slice())?;
            buffer.clear();

            write_posg_quad(&mut buffer, quad);
            self.posg.remove(buffer.as_slice())?;
            buffer.clear();

            write_ospg_quad(&mut buffer, quad);
            self.ospg.remove(buffer.as_slice())?;
            buffer.clear();

            write_gspo_quad(&mut buffer, quad);
            self.gspo.remove(buffer.as_slice())?;
            buffer.clear();

            write_gpos_quad(&mut buffer, quad);
            self.gpos.remove(buffer.as_slice())?;
            buffer.clear();

            write_gosp_quad(&mut buffer, quad);
            self.gosp.remove(buffer.as_slice())?;
        }
        buffer.clear();

        Ok(())
    }

    fn insert_encoded_named_graph(
        &mut self,
        graph_name: EncodedTerm,
    ) -> Result<(), SledUnabortableTransactionError> {
        self.graphs.insert(encode_term(graph_name), &[])?;
        Ok(())
    }

    fn clear_encoded_graph(
        &mut self,
        _: EncodedTerm,
    ) -> Result<(), SledUnabortableTransactionError> {
        Err(SledUnabortableTransactionError::Storage(io::Error::new(
            io::ErrorKind::Other,
            "CLEAR is not implemented in Sled transactions",
        )))
    }

    fn remove_encoded_named_graph(
        &mut self,
        _: EncodedTerm,
    ) -> Result<(), SledUnabortableTransactionError> {
        Err(SledUnabortableTransactionError::Storage(io::Error::new(
            io::ErrorKind::Other,
            "DROP is not implemented in Sled transactions",
        )))
    }

    fn clear(&mut self) -> Result<(), SledUnabortableTransactionError> {
        Err(SledUnabortableTransactionError::Storage(io::Error::new(
            io::ErrorKind::Other,
            "CLEAR ALL is not implemented in Sled transactions",
        )))
    }
}

/// Error returned by a Sled transaction
#[derive(Debug)]
pub enum SledTransactionError<T> {
    /// A failure returned by the API user that have aborted the transaction
    Abort(T),
    /// A storage related error
    Storage(io::Error),
}

impl<T: fmt::Display> fmt::Display for SledTransactionError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Abort(e) => e.fmt(f),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl<T: Error + 'static> Error for SledTransactionError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Abort(e) => Some(e),
            Self::Storage(e) => Some(e),
        }
    }
}

impl<T> From<TransactionError<T>> for SledTransactionError<T> {
    fn from(e: TransactionError<T>) -> Self {
        match e {
            TransactionError::Abort(e) => Self::Abort(e),
            TransactionError::Storage(e) => Self::Storage(e.into()),
        }
    }
}

impl<T: Into<io::Error>> From<SledTransactionError<T>> for io::Error {
    fn from(e: SledTransactionError<T>) -> Self {
        match e {
            SledTransactionError::Abort(e) => e.into(),
            SledTransactionError::Storage(e) => e,
        }
    }
}

/// An error returned from the transaction methods.
/// Should be returned as it is
#[derive(Debug)]
pub enum SledUnabortableTransactionError {
    #[doc(hidden)]
    Conflict,
    /// A regular error
    Storage(io::Error),
}

impl fmt::Display for SledUnabortableTransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(f, "Transaction conflict"),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl Error for SledUnabortableTransactionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(e) => Some(e),
            _ => None,
        }
    }
}

impl From<SledUnabortableTransactionError> for EvaluationError {
    fn from(e: SledUnabortableTransactionError) -> Self {
        match e {
            SledUnabortableTransactionError::Storage(e) => Self::Io(e),
            SledUnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

impl From<StoreOrParseError<SledUnabortableTransactionError>> for SledUnabortableTransactionError {
    fn from(e: StoreOrParseError<SledUnabortableTransactionError>) -> Self {
        match e {
            StoreOrParseError::Store(e) => e,
            StoreOrParseError::Parse(e) => Self::Storage(e),
        }
    }
}

impl From<UnabortableTransactionError> for SledUnabortableTransactionError {
    fn from(e: UnabortableTransactionError) -> Self {
        match e {
            UnabortableTransactionError::Storage(e) => Self::Storage(e.into()),
            UnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

/// An error returned from the transaction closure
#[derive(Debug)]
pub enum SledConflictableTransactionError<T> {
    /// A failure returned by the user that will abort the transaction
    Abort(T),
    #[doc(hidden)]
    Conflict,
    /// A storage related error
    Storage(io::Error),
}

impl<T: fmt::Display> fmt::Display for SledConflictableTransactionError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(f, "Transaction conflict"),
            Self::Storage(e) => e.fmt(f),
            Self::Abort(e) => e.fmt(f),
        }
    }
}

impl<T: Error + 'static> Error for SledConflictableTransactionError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Abort(e) => Some(e),
            Self::Storage(e) => Some(e),
            _ => None,
        }
    }
}

impl<T> From<SledUnabortableTransactionError> for SledConflictableTransactionError<T> {
    fn from(e: SledUnabortableTransactionError) -> Self {
        match e {
            SledUnabortableTransactionError::Storage(e) => Self::Storage(e),
            SledUnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

impl<T> From<SledConflictableTransactionError<T>> for ConflictableTransactionError<T> {
    fn from(e: SledConflictableTransactionError<T>) -> Self {
        match e {
            SledConflictableTransactionError::Abort(e) => ConflictableTransactionError::Abort(e),
            SledConflictableTransactionError::Conflict => ConflictableTransactionError::Conflict,
            SledConflictableTransactionError::Storage(e) => {
                ConflictableTransactionError::Storage(e.into())
            }
        }
    }
}

pub(crate) struct DecodingQuadsIterator {
    first: DecodingQuadIterator,
    second: Option<DecodingQuadIterator>,
}

impl DecodingQuadsIterator {
    fn new(first: DecodingQuadIterator) -> Self {
        Self {
            first,
            second: None,
        }
    }

    fn pair(first: DecodingQuadIterator, second: DecodingQuadIterator) -> Self {
        Self {
            first,
            second: Some(second),
        }
    }
}

impl Iterator for DecodingQuadsIterator {
    type Item = Result<EncodedQuad, io::Error>;

    fn next(&mut self) -> Option<Result<EncodedQuad, io::Error>> {
        if let Some(result) = self.first.next() {
            Some(result)
        } else if let Some(second) = self.second.as_mut() {
            second.next()
        } else {
            None
        }
    }
}

pub(crate) struct DecodingQuadIterator {
    iter: Iter,
    encoding: QuadEncoding,
}

impl Iterator for DecodingQuadIterator {
    type Item = Result<EncodedQuad, io::Error>;

    fn next(&mut self) -> Option<Result<EncodedQuad, io::Error>> {
        Some(match self.iter.next()? {
            Ok((encoded, _)) => self.encoding.decode(&encoded),
            Err(error) => Err(error.into()),
        })
    }
}

pub(crate) struct DecodingGraphIterator {
    iter: Iter,
}

impl Iterator for DecodingGraphIterator {
    type Item = Result<EncodedTerm, io::Error>;

    fn next(&mut self) -> Option<Result<EncodedTerm, io::Error>> {
        Some(match self.iter.next()? {
            Ok((encoded, _)) => decode_term(&encoded),
            Err(error) => Err(error.into()),
        })
    }
}

/// An iterator returning the quads contained in a [`SledStore`].
pub struct SledQuadIter {
    inner: QuadIterInner,
}

enum QuadIterInner {
    Quads {
        iter: DecodingQuadsIterator,
        store: SledStore,
    },
    Error(Once<io::Error>),
    Empty,
}

impl Iterator for SledQuadIter {
    type Item = Result<Quad, io::Error>;

    fn next(&mut self) -> Option<Result<Quad, io::Error>> {
        match &mut self.inner {
            QuadIterInner::Quads { iter, store } => Some(match iter.next()? {
                Ok(quad) => store.decode_quad(&quad).map_err(|e| e.into()),
                Err(error) => Err(error),
            }),
            QuadIterInner::Error(iter) => iter.next().map(Err),
            QuadIterInner::Empty => None,
        }
    }
}

/// An iterator returning the graph names contained in a [`SledStore`].
pub struct SledGraphNameIter {
    iter: DecodingGraphIterator,
    store: SledStore,
}

impl Iterator for SledGraphNameIter {
    type Item = Result<NamedOrBlankNode, io::Error>;

    fn next(&mut self) -> Option<Result<NamedOrBlankNode, io::Error>> {
        Some(
            self.iter
                .next()?
                .and_then(|graph_name| Ok(self.store.decode_named_or_blank_node(graph_name)?)),
        )
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

#[test]
fn store() -> Result<(), io::Error> {
    use crate::model::*;

    let main_s = NamedOrBlankNode::from(BlankNode::default());
    let main_p = NamedNode::new("http://example.com").unwrap();
    let main_o = Term::from(Literal::from(1));
    let main_g = GraphName::from(BlankNode::default());

    let default_quad = Quad::new(main_s.clone(), main_p.clone(), main_o.clone(), None);
    let named_quad = Quad::new(
        main_s.clone(),
        main_p.clone(),
        main_o.clone(),
        main_g.clone(),
    );
    let default_quads = vec![
        Quad::new(main_s.clone(), main_p.clone(), Literal::from(0), None),
        default_quad.clone(),
        Quad::new(
            main_s.clone(),
            main_p.clone(),
            Literal::from(200000000),
            None,
        ),
    ];
    let all_quads = vec![
        Quad::new(main_s.clone(), main_p.clone(), Literal::from(0), None),
        default_quad.clone(),
        Quad::new(
            main_s.clone(),
            main_p.clone(),
            Literal::from(200000000),
            None,
        ),
        named_quad.clone(),
    ];

    let store = SledStore::new()?;
    for t in &default_quads {
        store.insert(t)?;
    }

    let result: Result<_, SledTransactionError<io::Error>> = store.transaction(|t| {
        t.remove(&default_quad)?;
        t.insert(&named_quad)?;
        t.insert(&default_quad)?;
        Ok(())
    });
    result?;

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
