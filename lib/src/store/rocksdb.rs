//! Store based on the [RocksDB](https://rocksdb.org/) key-value database.

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
    ReadableEncodedStore, WritableEncodedStore,
};
use rocksdb::*;
use std::collections::HashMap;
use std::convert::TryInto;
use std::io;
use std::io::{BufRead, Write};
use std::iter::{once, Once};
use std::mem::{take, transmute};
use std::path::Path;
use std::sync::Arc;
use std::{fmt, str};

/// Store based on the [RocksDB](https://rocksdb.org/) key-value database.
/// It encodes a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) and allows to query it using SPARQL.
///
/// To use it, the `"rocksdb"` feature needs to be activated.
///
/// Usage example:
/// ```
/// use oxigraph::RocksDbStore;
/// use oxigraph::model::*;
/// use oxigraph::sparql::QueryResults;
/// # use std::fs::remove_dir_all;
///
/// # {
/// let store = RocksDbStore::open("example.db")?;
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
/// }
/// #
/// # };
/// # remove_dir_all("example.db")?;
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone)]
pub struct RocksDbStore {
    db: Arc<DB>,
}

type EncodedTerm = crate::store::numeric_encoder::EncodedTerm<StrHash>;
type EncodedQuad = crate::store::numeric_encoder::EncodedQuad<StrHash>;

const ID2STR_CF: &str = "id2str";
const SPOG_CF: &str = "spog";
const POSG_CF: &str = "posg";
const OSPG_CF: &str = "ospg";
const GSPO_CF: &str = "gspo";
const GPOS_CF: &str = "gpos";
const GOSP_CF: &str = "gosp";
const DSPO_CF: &str = "dspo";
const DPOS_CF: &str = "dpos";
const DOSP_CF: &str = "dosp";
const GRAPHS_CF: &str = "graphs";

const COLUMN_FAMILIES: [&str; 11] = [
    ID2STR_CF, SPOG_CF, POSG_CF, OSPG_CF, GSPO_CF, GPOS_CF, GOSP_CF, DSPO_CF, DPOS_CF, DOSP_CF,
    GRAPHS_CF,
];

const MAX_TRANSACTION_SIZE: usize = 1024;

impl RocksDbStore {
    /// Opens a [`RocksDbStore`]()
    pub fn open(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        options.set_compaction_style(DBCompactionStyle::Universal);

        let this = Self {
            db: Arc::new(DB::open_cf(&options, path, &COLUMN_FAMILIES).map_err(map_err)?),
        };

        let mut version = this.ensure_version()?;
        if version == 0 {
            // We migrate to v1
            let mut transaction = this.auto_batch_writer();
            for quad in this.encoded_quads_for_pattern(None, None, None, None) {
                let quad = quad?;
                if !quad.graph_name.is_default_graph() {
                    transaction.insert_encoded_named_graph(quad.graph_name)?;
                }
            }
            transaction.apply()?;
            version = 1;
            this.set_version(version)?;
            this.flush()?;
        }

        match version {
            _ if version < LATEST_STORAGE_VERSION => Err(invalid_data_error(format!(
                "The RocksDB database is using the outdated encoding version {}. Automated migration is not supported, please dump the store dataset using a compatible Oxigraph version and load it again using the current version",
                version
            ))),
            LATEST_STORAGE_VERSION => Ok(this),
            _ => Err(invalid_data_error(format!(
                "The RocksDB database is using the too recent version {}. Upgrade to the latest Oxigraph version to load this database",
                version
            )))
        }
    }

    fn ensure_version(&self) -> Result<u64, io::Error> {
        Ok(
            if let Some(version) = self.db.get("oxversion").map_err(map_err)? {
                let mut buffer = [0; 8];
                buffer.copy_from_slice(&version);
                u64::from_be_bytes(buffer)
            } else {
                self.set_version(LATEST_STORAGE_VERSION)?;
                LATEST_STORAGE_VERSION
            },
        )
    }

    fn set_version(&self, version: u64) -> Result<(), io::Error> {
        self.db
            .put("oxversion", &version.to_be_bytes())
            .map_err(map_err)
    }

    fn flush(&self) -> Result<(), io::Error> {
        let mut options = FlushOptions::new();
        options.set_wait(true);
        self.db.flush_opt(&options).map_err(map_err)?;
        Ok(())
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
    ) -> RocksDbQuadIter {
        RocksDbQuadIter {
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
    pub fn iter(&self) -> RocksDbQuadIter {
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
        let default = self
            .db
            .full_iterator_cf(self.dspo_cf(), IteratorMode::Start)
            .count();
        let named = self
            .db
            .full_iterator_cf(self.gspo_cf(), IteratorMode::Start)
            .count();
        default + named
    }

    /// Returns if the store is empty
    pub fn is_empty(&self) -> bool {
        let default = self
            .db
            .full_iterator_cf(self.dspo_cf(), IteratorMode::Start)
            .next()
            .is_none();
        let named = self
            .db
            .full_iterator_cf(self.gspo_cf(), IteratorMode::Start)
            .next()
            .is_none();
        default && named
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
        let mut writer = self.auto_batch_writer();
        evaluate_update(
            self.clone(),
            &mut writer,
            update.try_into().map_err(|e| e.into())?,
            options,
        )?;
        Ok(writer.apply()?)
    }

    /// Executes an ACID transaction.
    ///
    /// The transaction is executed if the given closure returns `Ok`.
    /// The transaction is rollbacked if the closure returns `Err`.
    ///
    /// The transaction data are stored in memory while the transaction is not committed or rollbacked.
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::transaction()) for a usage example.
    pub fn transaction<'a, E: From<io::Error>>(
        &'a self,
        f: impl FnOnce(&mut RocksDbTransaction<'a>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut transaction = RocksDbTransaction {
            store: self,
            batch: WriteBatch::default(),
            buffer: Vec::new(),
            new_strings: HashMap::new(),
        };
        f(&mut transaction)?;
        Ok(transaction.apply()?)
    }

    /// Loads a graph file (i.e. triples) into the store
    ///
    /// Warning: This functions saves the triples in batch. If the parsing fails in the middle of the file,
    /// only a part of it may be written. Use a (memory greedy) [transaction](RocksDbStore::transaction()) if you do not want that.
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
        let mut transaction = self.auto_batch_writer();
        load_graph(
            &mut transaction,
            reader,
            format,
            to_graph_name.into(),
            base_iri,
        )?;
        transaction.apply()
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// Warning: This functions saves the quads in batch. If the parsing fails in the middle of the file,
    /// only a part of it may be written. Use a (memory greedy) [transaction](RocksDbStore::transaction()) if you do not want that.
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
        let mut transaction = self.auto_batch_writer();
        load_dataset(&mut transaction, reader, format, base_iri)?;
        transaction.apply()
    }

    /// Adds a quad to this store.
    /// This operation is atomic and could not leave the store in a bad state.
    pub fn insert<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<(), io::Error> {
        let mut transaction = self.auto_batch_writer();
        let quad = transaction.encode_quad(quad.into())?;
        transaction.insert_encoded(&quad)?;
        transaction.apply()
    }

    /// Removes a quad from this store.
    /// This operation is atomic and could not leave the store in a bad state.
    pub fn remove<'a>(&self, quad: impl Into<QuadRef<'a>>) -> Result<(), io::Error> {
        if let Some(quad) = self.get_encoded_quad(quad.into())? {
            let mut transaction = self.auto_batch_writer();
            transaction.remove_encoded(&quad)?;
            transaction.apply()
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
    pub fn dump_dataset(&self, writer: impl Write, syntax: DatasetFormat) -> Result<(), io::Error> {
        dump_dataset(self.iter(), writer, syntax)
    }

    /// Returns all the store named graphs
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::named_graphs()) for a usage example.
    pub fn named_graphs(&self) -> impl Iterator<Item = Result<NamedOrBlankNode, io::Error>> {
        let this = self.clone();
        self.encoded_named_graphs()
            .map(move |g| Ok(this.decode_named_or_blank_node(g?)?))
    }

    /// Checks if the store contains a given graph
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::contains_named_graph()) for a usage example.
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
    /// See [`MemoryStore`](super::memory::MemoryStore::insert_named_graph()) for a usage example.
    pub fn insert_named_graph<'a>(
        &self,
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Result<(), io::Error> {
        let mut transaction = self.auto_batch_writer();
        let graph_name = transaction.encode_named_or_blank_node(graph_name.into())?;
        transaction.insert_encoded_named_graph(graph_name)?;
        transaction.apply()
    }

    /// Clears a graph from this store.
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::clear_graph()) for a usage example.
    pub fn clear_graph<'a>(
        &self,
        graph_name: impl Into<GraphNameRef<'a>>,
    ) -> Result<(), io::Error> {
        if let Some(graph_name) = self.get_encoded_graph_name(graph_name.into())? {
            let mut transaction = self.auto_batch_writer();
            transaction.clear_encoded_graph(graph_name)?;
            transaction.apply()
        } else {
            Ok(())
        }
    }

    /// Removes a graph from this store.
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::remove_named_graph()) for a usage example.
    pub fn remove_named_graph<'a>(
        &self,
        graph_name: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Result<(), io::Error> {
        if let Some(graph_name) = self.get_encoded_named_or_blank_node(graph_name.into())? {
            let mut transaction = self.auto_batch_writer();
            transaction.remove_encoded_named_graph(graph_name)?;
            transaction.apply()
        } else {
            Ok(())
        }
    }

    /// Clears the store.
    ///
    /// See [`MemoryStore`](super::memory::MemoryStore::clear()) for a usage example.
    pub fn clear(&self) -> Result<(), io::Error> {
        let mut transaction = self.auto_batch_writer();
        transaction.clear()?;
        transaction.apply()
    }

    fn id2str_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, ID2STR_CF)
    }

    fn spog_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, SPOG_CF)
    }

    fn posg_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, POSG_CF)
    }

    fn ospg_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, OSPG_CF)
    }

    fn gspo_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, GSPO_CF)
    }

    fn gpos_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, GPOS_CF)
    }

    fn gosp_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, GOSP_CF)
    }

    fn dspo_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, DSPO_CF)
    }

    fn dpos_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, DPOS_CF)
    }

    fn dosp_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, DOSP_CF)
    }

    fn graphs_cf(&self) -> &ColumnFamily {
        get_cf(&self.db, GRAPHS_CF)
    }
    fn auto_batch_writer(&self) -> AutoBatchWriter<'_> {
        AutoBatchWriter {
            store: self,
            batch: WriteBatch::default(),
            buffer: Vec::default(),
        }
    }

    fn contains_encoded(&self, quad: &EncodedQuad) -> Result<bool, io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            Ok(self
                .db
                .get_pinned_cf(self.dspo_cf(), &buffer)
                .map_err(map_err)?
                .is_some())
        } else {
            write_gspo_quad(&mut buffer, quad);
            Ok(self
                .db
                .get_pinned_cf(self.gspo_cf(), &buffer)
                .map_err(map_err)?
                .is_some())
        }
    }

    fn quads(&self) -> DecodingIndexesIterator {
        DecodingIndexesIterator::pair(
            self.dspo_quads(Vec::default()),
            self.gspo_quads(Vec::default()),
        )
    }

    fn quads_for_subject(&self, subject: EncodedTerm) -> DecodingIndexesIterator {
        DecodingIndexesIterator::pair(
            self.dspo_quads(encode_term(subject)),
            self.spog_quads(encode_term(subject)),
        )
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::pair(
            self.dspo_quads(encode_term_pair(subject, predicate)),
            self.spog_quads(encode_term_pair(subject, predicate)),
        )
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::pair(
            self.dspo_quads(encode_term_triple(subject, predicate, object)),
            self.spog_quads(encode_term_triple(subject, predicate, object)),
        )
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::pair(
            self.dosp_quads(encode_term_pair(object, subject)),
            self.ospg_quads(encode_term_pair(object, subject)),
        )
    }

    fn quads_for_predicate(&self, predicate: EncodedTerm) -> DecodingIndexesIterator {
        DecodingIndexesIterator::pair(
            self.dpos_quads(encode_term(predicate)),
            self.posg_quads(encode_term(predicate)),
        )
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::pair(
            self.dpos_quads(encode_term_pair(predicate, object)),
            self.posg_quads(encode_term_pair(predicate, object)),
        )
    }

    fn quads_for_object(&self, object: EncodedTerm) -> DecodingIndexesIterator {
        DecodingIndexesIterator::pair(
            self.dosp_quads(encode_term(object)),
            self.ospg_quads(encode_term(object)),
        )
    }

    fn quads_for_graph(&self, graph_name: EncodedTerm) -> DecodingIndexesIterator {
        DecodingIndexesIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(Vec::default())
        } else {
            self.gspo_quads(encode_term(graph_name))
        })
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::new(if graph_name.is_default_graph() {
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
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::new(if graph_name.is_default_graph() {
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
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::new(if graph_name.is_default_graph() {
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
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(encode_term_pair(object, subject))
        } else {
            self.gosp_quads(encode_term_triple(graph_name, object, subject))
        })
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::new(if graph_name.is_default_graph() {
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
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(encode_term_pair(predicate, object))
        } else {
            self.gpos_quads(encode_term_triple(graph_name, predicate, object))
        })
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexesIterator {
        DecodingIndexesIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(encode_term(object))
        } else {
            self.gosp_quads(encode_term_pair(graph_name, object))
        })
    }

    fn spog_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.spog_cf(), prefix, QuadEncoding::Spog)
    }

    fn posg_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.posg_cf(), prefix, QuadEncoding::Posg)
    }

    fn ospg_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.ospg_cf(), prefix, QuadEncoding::Ospg)
    }

    fn gspo_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.gspo_cf(), prefix, QuadEncoding::Gspo)
    }

    fn gpos_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.gpos_cf(), prefix, QuadEncoding::Gpos)
    }

    fn gosp_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.gosp_cf(), prefix, QuadEncoding::Gosp)
    }

    fn dspo_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.dspo_cf(), prefix, QuadEncoding::Dspo)
    }

    fn dpos_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.dpos_cf(), prefix, QuadEncoding::Dpos)
    }

    fn dosp_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.dosp_cf(), prefix, QuadEncoding::Dosp)
    }

    fn inner_quads(
        &self,
        cf: &ColumnFamily,
        prefix: Vec<u8>,
        encoding: QuadEncoding,
    ) -> DecodingIndexIterator {
        let mut iter = self.db_iter(cf);
        iter.iter.seek(&prefix);
        DecodingIndexIterator {
            iter,
            prefix,
            encoding,
        }
    }

    #[allow(unsafe_code)]
    fn db_iter(&self, cf: &ColumnFamily) -> StaticDbRowIterator {
        // Valid because it's the same database so db can't be dropped before iter
        unsafe { StaticDbRowIterator::new(self.db.raw_iterator_cf(cf), self.db.clone()) }
    }
}

impl fmt::Display for RocksDbStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self.iter() {
            writeln!(f, "{}", t.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

impl StrEncodingAware for RocksDbStore {
    type Error = io::Error;
    type StrId = StrHash;
}

impl StrLookup for RocksDbStore {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, io::Error> {
        self.db
            .get_cf(self.id2str_cf(), &id.to_be_bytes())
            .map_err(map_err)?
            .map(String::from_utf8)
            .transpose()
            .map_err(invalid_data_error)
    }

    fn get_str_id(&self, value: &str) -> Result<Option<StrHash>, io::Error> {
        let id = StrHash::new(value);
        Ok(
            if self
                .db
                .get_cf(self.id2str_cf(), &id.to_be_bytes())
                .map_err(map_err)?
                .is_some()
            {
                Some(id)
            } else {
                None
            },
        )
    }
}

impl ReadableEncodedStore for RocksDbStore {
    type QuadsIter = DecodingIndexesIterator;
    type GraphsIter = DecodingGraphIterator;

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> DecodingIndexesIterator {
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
        let mut iter = self.db_iter(self.graphs_cf());
        iter.iter.seek_to_first();
        DecodingGraphIterator { iter }
    }

    fn contains_encoded_named_graph(&self, graph_name: EncodedTerm) -> Result<bool, io::Error> {
        Ok(self
            .db
            .get_cf(self.graphs_cf(), &encode_term(graph_name))
            .map_err(map_err)?
            .is_some())
    }
}

struct AutoBatchWriter<'a> {
    store: &'a RocksDbStore,
    batch: WriteBatch,
    buffer: Vec<u8>,
}

impl AutoBatchWriter<'_> {
    fn apply(self) -> Result<(), io::Error> {
        self.store.db.write(self.batch).map_err(map_err)
    }

    fn apply_if_big(&mut self) -> Result<(), io::Error> {
        if self.batch.len() > MAX_TRANSACTION_SIZE {
            self.store
                .db
                .write(take(&mut self.batch))
                .map_err(map_err)?;
        }
        Ok(())
    }

    fn clear_cf(&mut self, cf: &ColumnFamily) {
        self.batch.delete_range_cf(
            cf,
            [
                u8::MIN,
                u8::MIN,
                u8::MIN,
                u8::MIN,
                u8::MIN,
                u8::MIN,
                u8::MIN,
                u8::MIN,
            ],
            [
                u8::MAX,
                u8::MAX,
                u8::MAX,
                u8::MAX,
                u8::MAX,
                u8::MAX,
                u8::MAX,
                u8::MAX,
            ],
        )
    }
}

impl StrEncodingAware for AutoBatchWriter<'_> {
    type Error = io::Error;
    type StrId = StrHash;
}

impl StrContainer for AutoBatchWriter<'_> {
    fn insert_str(&mut self, value: &str) -> Result<StrHash, io::Error> {
        let key = StrHash::new(value);
        self.batch
            .put_cf(self.store.id2str_cf(), &key.to_be_bytes(), value);
        Ok(key)
    }
}

impl WritableEncodedStore for AutoBatchWriter<'_> {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<(), io::Error> {
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.dspo_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_pos_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.dpos_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_osp_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.dosp_cf(), &self.buffer, &[]);
        } else {
            write_spog_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.spog_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_posg_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.posg_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_ospg_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.ospg_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_gspo_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.gspo_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_gpos_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.gpos_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_gosp_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.gosp_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_term(&mut self.buffer, quad.graph_name);
            self.batch.put_cf(self.store.graphs_cf(), &self.buffer, &[]);
        }
        self.buffer.clear();

        self.apply_if_big()
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<(), io::Error> {
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.dspo_cf(), &self.buffer);
            self.buffer.clear();

            write_pos_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.dpos_cf(), &self.buffer);
            self.buffer.clear();

            write_osp_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.dosp_cf(), &self.buffer);
        } else {
            write_spog_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.spog_cf(), &self.buffer);
            self.buffer.clear();

            write_posg_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.posg_cf(), &self.buffer);
            self.buffer.clear();

            write_ospg_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.ospg_cf(), &self.buffer);
            self.buffer.clear();

            write_gspo_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.gspo_cf(), &self.buffer);
            self.buffer.clear();

            write_gpos_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.gpos_cf(), &self.buffer);
            self.buffer.clear();

            write_gosp_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.gosp_cf(), &self.buffer);
        }
        self.buffer.clear();

        self.apply_if_big()
    }

    fn insert_encoded_named_graph(&mut self, graph_name: EncodedTerm) -> Result<(), io::Error> {
        self.batch
            .put_cf(self.store.graphs_cf(), &encode_term(graph_name), &[]);
        self.apply_if_big()
    }

    fn clear_encoded_graph(&mut self, graph_name: EncodedTerm) -> Result<(), io::Error> {
        if graph_name.is_default_graph() {
            self.clear_cf(self.store.dspo_cf());
            self.clear_cf(self.store.dpos_cf());
            self.clear_cf(self.store.dosp_cf());
        } else {
            for quad in self.store.quads_for_graph(graph_name) {
                self.remove_encoded(&quad?)?;
            }
        }
        self.apply_if_big()
    }

    fn remove_encoded_named_graph(&mut self, graph_name: EncodedTerm) -> Result<(), io::Error> {
        for quad in self.store.quads_for_graph(graph_name) {
            self.remove_encoded(&quad?)?;
        }
        self.batch
            .delete_cf(self.store.graphs_cf(), &encode_term(graph_name));
        self.apply_if_big()
    }

    fn clear(&mut self) -> Result<(), io::Error> {
        self.clear_cf(self.store.spog_cf());
        self.clear_cf(self.store.posg_cf());
        self.clear_cf(self.store.ospg_cf());
        self.clear_cf(self.store.gspo_cf());
        self.clear_cf(self.store.gpos_cf());
        self.clear_cf(self.store.gosp_cf());
        self.clear_cf(self.store.dspo_cf());
        self.clear_cf(self.store.dpos_cf());
        self.clear_cf(self.store.dosp_cf());
        self.clear_cf(self.store.graphs_cf());
        self.clear_cf(self.store.id2str_cf());
        self.apply_if_big()
    }
}

/// Allows inserting and deleting quads during an ACID transaction with the [`RocksDbStore`].
pub struct RocksDbTransaction<'a> {
    store: &'a RocksDbStore,
    batch: WriteBatch,
    buffer: Vec<u8>,
    new_strings: HashMap<StrHash, String>,
}

impl RocksDbTransaction<'_> {
    /// Loads a graph file (i.e. triples) into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content is temporarily stored in main memory.
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
        &mut self,
        reader: impl BufRead,
        syntax: GraphFormat,
        to_graph_name: impl Into<GraphNameRef<'a>>,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        load_graph(self, reader, syntax, to_graph_name.into(), base_iri)?;
        Ok(())
    }

    /// Loads a dataset file (i.e. quads) into the store. into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content is temporarily stored in main memory.
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
        &mut self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        load_dataset(self, reader, format, base_iri)?;
        Ok(())
    }

    /// Adds a quad to this store during the transaction.
    pub fn insert<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> Result<(), io::Error> {
        let quad = self.encode_quad(quad.into())?;
        self.insert_encoded(&quad)
    }

    /// Removes a quad from this store during the transaction.
    pub fn remove<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> Result<(), io::Error> {
        // Works because all strings could be encoded
        if let Some(quad) = self.get_encoded_quad(quad.into()).unwrap() {
            self.remove_encoded(&quad)
        } else {
            Ok(())
        }
    }

    fn apply(self) -> Result<(), io::Error> {
        self.store.db.write(self.batch).map_err(map_err)
    }
}

impl StrEncodingAware for RocksDbTransaction<'_> {
    type Error = io::Error;
    type StrId = StrHash;
}

impl StrLookup for RocksDbTransaction<'_> {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, io::Error> {
        if let Some(str) = self.new_strings.get(&id) {
            Ok(Some(str.clone()))
        } else {
            self.store.get_str(id)
        }
    }

    fn get_str_id(&self, value: &str) -> Result<Option<StrHash>, io::Error> {
        let id = StrHash::new(value);
        Ok(
            if self.new_strings.contains_key(&id)
                || self
                    .store
                    .db
                    .get_cf(self.store.id2str_cf(), &id.to_be_bytes())
                    .map_err(map_err)?
                    .is_some()
            {
                Some(id)
            } else {
                None
            },
        )
    }
}

impl StrContainer for RocksDbTransaction<'_> {
    fn insert_str(&mut self, value: &str) -> Result<StrHash, io::Error> {
        let key = StrHash::new(value);
        self.batch
            .put_cf(self.store.id2str_cf(), &key.to_be_bytes(), value);
        self.new_strings.insert(key, value.to_owned());
        Ok(key)
    }
}

impl WritableEncodedStore for RocksDbTransaction<'_> {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<(), io::Error> {
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.dspo_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_pos_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.dpos_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_osp_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.dosp_cf(), &self.buffer, &[]);
        } else {
            write_spog_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.spog_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_posg_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.posg_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_ospg_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.ospg_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_gspo_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.gspo_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_gpos_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.gpos_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_gosp_quad(&mut self.buffer, quad);
            self.batch.put_cf(self.store.gosp_cf(), &self.buffer, &[]);
            self.buffer.clear();

            write_term(&mut self.buffer, quad.graph_name);
            self.batch.put_cf(self.store.graphs_cf(), &self.buffer, &[]);
        }
        self.buffer.clear();

        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<(), io::Error> {
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.dspo_cf(), &self.buffer);
            self.buffer.clear();

            write_pos_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.dpos_cf(), &self.buffer);
            self.buffer.clear();

            write_osp_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.dosp_cf(), &self.buffer);
        } else {
            write_spog_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.spog_cf(), &self.buffer);
            self.buffer.clear();

            write_posg_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.posg_cf(), &self.buffer);
            self.buffer.clear();

            write_ospg_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.ospg_cf(), &self.buffer);
            self.buffer.clear();

            write_gspo_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.gspo_cf(), &self.buffer);
            self.buffer.clear();

            write_gpos_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.gpos_cf(), &self.buffer);
            self.buffer.clear();

            write_gosp_quad(&mut self.buffer, quad);
            self.batch.delete_cf(self.store.gosp_cf(), &self.buffer);
        }
        self.buffer.clear();

        Ok(())
    }

    fn insert_encoded_named_graph(&mut self, graph_name: EncodedTerm) -> Result<(), io::Error> {
        self.batch
            .put_cf(self.store.graphs_cf(), &encode_term(graph_name), &[]);
        Ok(())
    }

    fn clear_encoded_graph(&mut self, _: EncodedTerm) -> Result<(), io::Error> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "CLEAR is not implemented in RocksDB transactions",
        ))
    }

    fn remove_encoded_named_graph(&mut self, _: EncodedTerm) -> Result<(), io::Error> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "DROP is not implemented in RocksDB transactions",
        ))
    }

    fn clear(&mut self) -> Result<(), Self::Error> {
        Err(io::Error::new(
            io::ErrorKind::Other,
            "CLEAR ALL is not implemented in RocksDB transactions",
        ))
    }
}

#[allow(clippy::expect_used)]
fn get_cf<'a>(db: &'a DB, name: &str) -> &'a ColumnFamily {
    db.cf_handle(name)
        .expect("A column family that should exist in RocksDB does not exist")
}

struct StaticDbRowIterator {
    iter: DBRawIterator<'static>,
    _db: Arc<DB>, // needed to ensure that DB still lives while iter is used
}

impl StaticDbRowIterator {
    /// Creates a static iterator from a non static one by keeping a ARC reference to the database
    /// Caller must ensure that the iterator belongs to the same database
    ///
    /// This unsafe method is required to get static iterators and ease the usage of the library
    /// and make streaming Python bindings possible
    #[allow(unsafe_code)]
    unsafe fn new(iter: DBRawIterator<'_>, db: Arc<DB>) -> Self {
        Self {
            iter: transmute(iter),
            _db: db,
        }
    }

    fn key(&self) -> Option<&[u8]> {
        self.iter.key()
    }

    fn next(&mut self) {
        self.iter.next()
    }
}

pub(crate) struct DecodingIndexesIterator {
    first: DecodingIndexIterator,
    second: Option<DecodingIndexIterator>,
}

impl DecodingIndexesIterator {
    fn new(first: DecodingIndexIterator) -> Self {
        Self {
            first,
            second: None,
        }
    }

    fn pair(first: DecodingIndexIterator, second: DecodingIndexIterator) -> Self {
        Self {
            first,
            second: Some(second),
        }
    }
}

impl Iterator for DecodingIndexesIterator {
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

struct DecodingIndexIterator {
    iter: StaticDbRowIterator,
    prefix: Vec<u8>,
    encoding: QuadEncoding,
}

impl Iterator for DecodingIndexIterator {
    type Item = Result<EncodedQuad, io::Error>;

    fn next(&mut self) -> Option<Result<EncodedQuad, io::Error>> {
        if let Some(key) = self.iter.key() {
            if key.starts_with(&self.prefix) {
                let result = self.encoding.decode(key);
                self.iter.next();
                Some(result)
            } else {
                None
            }
        } else {
            None
        }
    }
}

fn map_err(e: Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

/// An iterator returning the quads contained in a [`RocksDbStore`].
pub struct RocksDbQuadIter {
    inner: QuadIterInner,
}

enum QuadIterInner {
    Quads {
        iter: DecodingIndexesIterator,
        store: RocksDbStore,
    },
    Error(Once<io::Error>),
    Empty,
}

impl Iterator for RocksDbQuadIter {
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

pub(crate) struct DecodingGraphIterator {
    iter: StaticDbRowIterator,
}

impl Iterator for DecodingGraphIterator {
    type Item = Result<EncodedTerm, io::Error>;

    fn next(&mut self) -> Option<Result<EncodedTerm, io::Error>> {
        if let Some(key) = self.iter.key() {
            let result = decode_term(key);
            self.iter.next();
            Some(result)
        } else {
            None
        }
    }
}
#[test]
fn store() -> Result<(), io::Error> {
    use crate::model::*;
    use rand::random;
    use std::env::temp_dir;
    use std::fs::remove_dir_all;

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

    let mut repo_path = temp_dir();
    repo_path.push(random::<u128>().to_string());

    {
        let store = RocksDbStore::open(&repo_path)?;
        for t in &default_quads {
            store.insert(t)?;
        }

        store.transaction(|t| {
            t.remove(&default_quad)?;
            t.insert(&named_quad)?;
            t.insert(&default_quad)
        })?;

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
    }

    remove_dir_all(&repo_path)?;
    Ok(())
}
