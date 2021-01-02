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
        };

        let version = this.ensure_version()?;
        if version != LATEST_STORAGE_VERSION {
            return Err(invalid_data_error(format!(
                "The Sled database is still using the encoding version {}, please upgrade it",
                version
            )));
        }

        Ok(this)
    }

    fn ensure_version(&self) -> Result<u64, io::Error> {
        Ok(if let Some(version) = self.default.get("oxversion")? {
            let mut buffer = [0; 8];
            buffer.copy_from_slice(&version);
            u64::from_be_bytes(buffer)
        } else {
            self.default
                .insert("oxversion", &LATEST_STORAGE_VERSION.to_be_bytes())?;
            LATEST_STORAGE_VERSION
        })
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/).
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::query()) for a usage example.
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
    /// See [`MemoryStore`](super::memory::MemoryStore::quads_for_pattern()) for a usage example.
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
    /// See [`MemoryStore`](super::memory::MemoryStore::update()) for a usage example.
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
    /// let ex = NamedNode::new("http://example.com")?;
    /// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
    ///
    /// // transaction
    /// store.transaction(|transaction| {
    ///     transaction.insert(&quad)?;
    ///     Ok(()) as Result<(),SledConflictableTransactionError<Infallible>>
    /// })?;
    ///
    /// // quad filter
    /// assert!(store.contains(&quad)?);
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
        )
            .transaction(
                move |(id2str, spog, posg, ospg, gspo, gpos, gosp, dspo, dpos, dosp)| {
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
                    })?)
                },
            )?)
    }

    /// Loads a graph file (i.e. triples) into the store
    ///
    /// Warning: This functions saves the triples in a not atomic way. If the parsing fails in the middle of the file,
    /// only a part of it may be written to the store.
    /// Also, this method is optimized for performances and is not atomic.
    /// It might leave the store in a bad state if a crash happens during a triple insertion.
    /// Use a (memory greedy) [transaction](SledStore::transaction()) if you do not want that.
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::load_graph()) for a usage example.
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
    /// Warning: This functions saves the triples in a not atomic way. If the parsing fails in the middle of the file,
    /// only a part of it may be written to the store.
    /// Also, this method is optimized for performances and is not atomic.
    /// It might leave the store in a bad state if a crash happens during a quad insertion.
    /// Use a (memory greedy) [transaction](SledStore::transaction()) if you do not want that.
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::load_dataset()) for a usage example.
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
    /// See [`MemoryStore`](super::memory::MemoryStore::dump_graph()) for a usage example.
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
    /// See [`MemoryStore`](super::memory::MemoryStore::dump_dataset()) for a usage example.
    pub fn dump_dataset(&self, writer: impl Write, format: DatasetFormat) -> Result<(), io::Error> {
        dump_dataset(self.iter(), writer, format)
    }

    /// Removes a graph from this store.
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::drop_graph()) for a usage example.
    pub fn drop_graph<'a>(&self, graph_name: impl Into<GraphNameRef<'a>>) -> Result<(), io::Error> {
        if let Some(graph_name) = self.get_encoded_graph_name(graph_name.into())? {
            for quad in self.encoded_quads_for_pattern(None, None, None, Some(graph_name)) {
                let mut this = self;
                this.remove_encoded(&quad?)?;
            }
        }
        Ok(())
    }

    /// Clears the store.
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::clear()) for a usage example.
    pub fn clear(&self) -> Result<(), io::Error> {
        self.dspo.clear()?;
        self.dpos.clear()?;
        self.dosp.clear()?;
        self.gspo.clear()?;
        self.gpos.clear()?;
        self.gosp.clear()?;
        self.spog.clear()?;
        self.posg.clear()?;
        self.ospg.clear()?;
        self.id2str.clear()?;
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
        self.inner_quads(&self.spog, prefix, QuadEncoding::SPOG)
    }

    fn posg_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.posg, prefix, QuadEncoding::POSG)
    }

    fn ospg_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.ospg, prefix, QuadEncoding::OSPG)
    }

    fn gspo_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gspo, prefix, QuadEncoding::GSPO)
    }

    fn gpos_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gpos, prefix, QuadEncoding::GPOS)
    }

    fn gosp_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gosp, prefix, QuadEncoding::GOSP)
    }

    fn dspo_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.dspo, prefix, QuadEncoding::DSPO)
    }

    fn dpos_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.dpos, prefix, QuadEncoding::DPOS)
    }

    fn dosp_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.dosp, prefix, QuadEncoding::DOSP)
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
            buffer.clear();
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
            buffer.clear();
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
            buffer.clear();
        }

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
}

impl SledTransaction<'_> {
    /// Loads a graph file (i.e. triples) into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content might be temporarily stored in main memory.
    /// Do not use for big files.
    ///
    /// See [`MemoryTransaction`](super::memory::MemoryTransaction::load_graph()) for a usage example.
    ///
    /// If the file parsing fails in the middle of the file, the triples read before are still
    /// considered by the transaction. Rollback the transaction by making the transaction closure
    /// return an error if you don't want that.
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
    /// See [`MemoryTransaction`](super::memory::MemoryTransaction::load_dataset()) for a usage example.
    ///
    /// If the file parsing fails in the middle of the file, the quads read before are still
    /// considered by the transaction. Rollback the transaction by making the transaction closure
    /// return an error if you don't want that.
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
            buffer.clear();
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
        }

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
            buffer.clear();
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
            buffer.clear();
        }

        Ok(())
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
