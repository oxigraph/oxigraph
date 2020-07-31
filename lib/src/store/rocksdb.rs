//! Store based on the [RocksDB](https://rocksdb.org/) key-value database.

use crate::error::{invalid_data_error, UnwrapInfallible};
use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use crate::sparql::{EvaluationError, Query, QueryOptions, QueryResult, SimplePreparedQuery};
use crate::store::numeric_encoder::*;
use crate::store::{
    dump_dataset, dump_graph, load_dataset, load_graph, ReadableEncodedStore, WritableEncodedStore,
};
use rocksdb::*;
use std::convert::{Infallible, TryInto};
use std::io;
use std::io::{BufRead, Cursor, Write};
use std::mem::{take, transmute};
use std::path::Path;
use std::sync::Arc;
use std::{fmt, str};

/// Store based on the [RocksDB](https://rocksdb.org/) key-value database.
/// It encodes a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) and allows to query and update it using SPARQL.
///
/// To use it, the `"rocksdb"` feature needs to be activated.
///
/// Usage example:
/// ```
/// use oxigraph::RocksDbStore;
/// use oxigraph::model::*;
/// use oxigraph::sparql::{QueryOptions, QueryResult};
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
/// if let QueryResult::Solutions(mut solutions) = store.query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())? {
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

const ID2STR_CF: &str = "id2str";
const SPOG_CF: &str = "spog";
const POSG_CF: &str = "posg";
const OSPG_CF: &str = "ospg";
const GSPO_CF: &str = "gspo";
const GPOS_CF: &str = "gpos";
const GOSP_CF: &str = "gosp";

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

const COLUMN_FAMILIES: [&str; 7] = [
    ID2STR_CF, SPOG_CF, POSG_CF, OSPG_CF, GSPO_CF, GPOS_CF, GOSP_CF,
];

const MAX_TRANSACTION_SIZE: usize = 1024;

impl RocksDbStore {
    /// Opens a `RocksDbStore`
    pub fn open(path: impl AsRef<Path>) -> Result<Self, io::Error> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        options.set_compaction_style(DBCompactionStyle::Universal);

        Ok(Self {
            db: Arc::new(DB::open_cf(&options, path, &COLUMN_FAMILIES).map_err(map_err)?),
        })
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/).
    ///
    /// See `MemoryStore` for a usage example.
    pub fn query(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
    ) -> Result<QueryResult, EvaluationError> {
        self.prepare_query(query, options)?.exec()
    }

    /// Prepares a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) and returns an object that could be used to execute it.
    /// It is useful if you want to execute multiple times the same SPARQL query.
    ///
    /// See `MemoryStore` for a usage example.
    pub fn prepare_query(
        &self,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
    ) -> Result<RocksDbPreparedQuery, EvaluationError> {
        Ok(RocksDbPreparedQuery(SimplePreparedQuery::new(
            (*self).clone(),
            query,
            options,
        )?))
    }

    /// Retrieves quads with a filter on each quad component
    ///
    /// See `MemoryStore` for a usage example.
    pub fn quads_for_pattern(
        &self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<&GraphName>,
    ) -> impl Iterator<Item = Result<Quad, io::Error>> {
        let subject = subject.map(|s| s.into());
        let predicate = predicate.map(|p| p.into());
        let object = object.map(|o| o.into());
        let graph_name = graph_name.map(|g| g.into());
        let store = self.clone();
        self.encoded_quads_for_pattern(subject, predicate, object, graph_name)
            .map(move |quad| Ok(store.decode_quad(&quad?)?))
    }

    /// Checks if this store contains a given quad
    pub fn contains(&self, quad: &Quad) -> Result<bool, io::Error> {
        let quad = quad.into();
        self.contains_encoded(&quad)
    }

    /// Returns the number of quads in the store
    pub fn len(&self) -> usize {
        self.db
            .full_iterator_cf(self.spog_cf(), IteratorMode::Start)
            .count()
    }

    /// Returns if the store is empty
    pub fn is_empty(&self) -> bool {
        self.db
            .full_iterator_cf(self.spog_cf(), IteratorMode::Start)
            .next()
            .is_none()
    }

    /// Executes a transaction.
    ///
    /// The transaction is executed if the given closure returns `Ok`.
    /// Nothing is done if the closure returns `Err`.
    ///
    /// See `MemoryStore` for a usage example.
    pub fn transaction<'a, E: From<io::Error>>(
        &'a self,
        f: impl FnOnce(&mut RocksDbTransaction<'a>) -> Result<(), E>,
    ) -> Result<(), E> {
        let mut transaction = RocksDbTransaction {
            inner: BatchWriter {
                store: self,
                batch: WriteBatch::default(),
                buffer: Vec::default(),
            },
        };
        f(&mut transaction)?;
        Ok(transaction.inner.apply()?)
    }

    /// Loads a graph file (i.e. triples) into the store
    ///
    /// Warning: This functions saves the triples in batch. If the parsing fails in the middle of the file,
    /// only a part of it may be written. Use a (memory greedy) transaction if you do not want that.
    ///
    /// See `MemoryStore` for a usage example.
    ///
    /// Errors related to parameter validation like the base IRI use the `INVALID_INPUT` error kind.
    /// Errors related to a bad syntax in the loaded file use the `INVALID_DATA` error kind.
    /// Errors related to data loading into the store use the other error kinds.
    pub fn load_graph(
        &self,
        reader: impl BufRead,
        format: GraphFormat,
        to_graph_name: &GraphName,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        let mut transaction = self.auto_batch_writer();
        load_graph(&mut transaction, reader, format, to_graph_name, base_iri)?;
        Ok(transaction.apply()?)
    }

    /// Loads a dataset file (i.e. quads) into the store.
    ///
    /// Warning: This functions saves the quads in batch. If the parsing fails in the middle of the file,
    /// only a part of it may be written. Use a (memory greedy) transaction if you do not want that.
    ///
    /// See `MemoryStore` for a usage example.
    ///
    /// Errors related to parameter validation like the base IRI use the `INVALID_INPUT` error kind.
    /// Errors related to a bad syntax in the loaded file use the `INVALID_DATA` error kind.
    /// Errors related to data loading into the store use the other error kinds.
    pub fn load_dataset(
        &self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        let mut transaction = self.auto_batch_writer();
        load_dataset(&mut transaction, reader, format, base_iri)?;
        Ok(transaction.apply()?)
    }

    /// Adds a quad to this store.
    pub fn insert(&self, quad: &Quad) -> Result<(), io::Error> {
        let mut transaction = self.auto_batch_writer();
        let quad = transaction.encode_quad(quad)?;
        transaction.insert_encoded(&quad)?;
        transaction.apply()
    }

    /// Removes a quad from this store.
    pub fn remove(&self, quad: &Quad) -> Result<(), io::Error> {
        let mut transaction = self.auto_batch_writer();
        let quad = quad.into();
        transaction.remove_encoded(&quad)?;
        transaction.apply()
    }

    /// Dumps a store graph into a file.
    ///    
    /// See `MemoryStore` for a usage example.
    pub fn dump_graph(
        &self,
        writer: impl Write,
        format: GraphFormat,
        from_graph_name: &GraphName,
    ) -> Result<(), io::Error> {
        dump_graph(
            self.quads_for_pattern(None, None, None, Some(from_graph_name))
                .map(|q| Ok(q?.into())),
            writer,
            format,
        )
    }

    /// Dumps the store dataset into a file.
    ///    
    /// See `MemoryStore` for a usage example.
    pub fn dump_dataset(&self, writer: impl Write, syntax: DatasetFormat) -> Result<(), io::Error> {
        dump_dataset(
            self.quads_for_pattern(None, None, None, None),
            writer,
            syntax,
        )
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

    fn auto_batch_writer(&self) -> AutoBatchWriter<'_> {
        AutoBatchWriter {
            inner: BatchWriter {
                store: self,
                batch: WriteBatch::default(),
                buffer: Vec::default(),
            },
        }
    }

    fn contains_encoded(&self, quad: &EncodedQuad) -> Result<bool, io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        write_spog_quad(&mut buffer, quad);
        Ok(self
            .db
            .get_pinned_cf(self.spog_cf(), &buffer)
            .map_err(map_err)?
            .is_some())
    }

    fn quads(&self) -> DecodingIndexIterator {
        self.spog_quads(Vec::default())
    }

    fn quads_for_subject(&self, subject: EncodedTerm) -> DecodingIndexIterator {
        self.spog_quads(encode_term(subject))
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> DecodingIndexIterator {
        self.spog_quads(encode_term_pair(subject, predicate))
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingIndexIterator {
        self.spog_quads(encode_term_triple(subject, predicate, object))
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingIndexIterator {
        self.ospg_quads(encode_term_pair(object, subject))
    }

    fn quads_for_predicate(&self, predicate: EncodedTerm) -> DecodingIndexIterator {
        self.posg_quads(encode_term(predicate))
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> DecodingIndexIterator {
        self.posg_quads(encode_term_pair(predicate, object))
    }

    fn quads_for_object(&self, object: EncodedTerm) -> DecodingIndexIterator {
        self.ospg_quads(encode_term(object))
    }

    fn quads_for_graph(&self, graph_name: EncodedTerm) -> DecodingIndexIterator {
        self.gspo_quads(encode_term(graph_name))
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator {
        self.gspo_quads(encode_term_pair(graph_name, subject))
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator {
        self.gspo_quads(encode_term_triple(graph_name, subject, predicate))
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator {
        self.gosp_quads(encode_term_triple(graph_name, object, subject))
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator {
        self.gpos_quads(encode_term_pair(graph_name, predicate))
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator {
        self.gpos_quads(encode_term_triple(graph_name, predicate, object))
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> DecodingIndexIterator {
        self.gosp_quads(encode_term_pair(graph_name, object))
    }

    fn spog_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.spog_cf(), prefix, QuadEncoding::SPOG)
    }

    fn posg_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.posg_cf(), prefix, QuadEncoding::POSG)
    }

    fn ospg_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.ospg_cf(), prefix, QuadEncoding::OSPG)
    }

    fn gspo_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.gspo_cf(), prefix, QuadEncoding::GSPO)
    }

    fn gpos_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.gpos_cf(), prefix, QuadEncoding::GPOS)
    }

    fn gosp_quads(&self, prefix: Vec<u8>) -> DecodingIndexIterator {
        self.inner_quads(self.gosp_cf(), prefix, QuadEncoding::GOSP)
    }

    #[allow(unsafe_code)]
    fn inner_quads(
        &self,
        cf: &ColumnFamily,
        prefix: Vec<u8>,
        encoding: QuadEncoding,
    ) -> DecodingIndexIterator {
        let mut iter = self.db.raw_iterator_cf(cf);
        iter.seek(&prefix);
        DecodingIndexIterator {
            iter: unsafe { StaticDBRowIterator::new(iter, self.db.clone()) }, // This is safe because the iterator belongs to DB
            prefix,
            encoding,
        }
    }
}

impl fmt::Display for RocksDbStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self.quads_for_pattern(None, None, None, None) {
            writeln!(f, "{}", t.map_err(|_| fmt::Error)?)?;
        }
        Ok(())
    }
}

impl WithStoreError for RocksDbStore {
    type Error = io::Error;
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
}

impl ReadableEncodedStore for RocksDbStore {
    type QuadsIter = DecodingIndexIterator;

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> DecodingIndexIterator {
        match subject {
            Some(subject) => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => self
                            .spog_quads(encode_term_quad(subject, predicate, object, graph_name)),
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

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/) for the `RocksDbStore`.
pub struct RocksDbPreparedQuery(SimplePreparedQuery<RocksDbStore>);

impl RocksDbPreparedQuery {
    /// Evaluates the query and returns its results
    pub fn exec(&self) -> Result<QueryResult, EvaluationError> {
        self.0.exec()
    }
}

/// Allows inserting and deleting quads during a transaction with the `RocksDbStore`.
pub struct RocksDbTransaction<'a> {
    inner: BatchWriter<'a>,
}

impl RocksDbTransaction<'_> {
    /// Loads a graph file (i.e. triples) into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content might be temporarily stored in main memory.
    /// Do not use for big files.
    ///
    /// See `MemoryTransaction` for a usage example.
    ///
    /// Errors related to parameter validation like the base IRI use the `INVALID_INPUT` error kind.
    /// Errors related to a bad syntax in the loaded file use the `INVALID_DATA` error kind.
    pub fn load_graph(
        &mut self,
        reader: impl BufRead,
        syntax: GraphFormat,
        to_graph_name: &GraphName,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        load_graph(&mut self.inner, reader, syntax, to_graph_name, base_iri)?;
        Ok(())
    }

    /// Loads a dataset file (i.e. quads) into the store. into the store during the transaction.
    ///
    /// Warning: Because the load happens during a transaction,
    /// the full file content might be temporarily stored in main memory.
    /// Do not use for big files.
    ///
    /// See `MemoryTransaction` for a usage example.
    ///
    /// Errors related to parameter validation like the base IRI use the `INVALID_INPUT` error kind.
    /// Errors related to a bad syntax in the loaded file use the `INVALID_DATA` error kind.
    pub fn load_dataset(
        &mut self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        load_dataset(&mut self.inner, reader, format, base_iri)?;
        Ok(())
    }

    /// Adds a quad to this store during the transaction.
    pub fn insert(&mut self, quad: &Quad) {
        let quad = self.inner.encode_quad(quad).unwrap_infallible();
        self.inner.insert_encoded(&quad).unwrap_infallible()
    }

    /// Removes a quad from this store during the transaction.
    pub fn remove(&mut self, quad: &Quad) {
        let quad = quad.into();
        self.inner.remove_encoded(&quad).unwrap_infallible()
    }
}

struct BatchWriter<'a> {
    store: &'a RocksDbStore,
    batch: WriteBatch,
    buffer: Vec<u8>,
}

impl WithStoreError for BatchWriter<'_> {
    type Error = Infallible;
}

impl StrContainer for BatchWriter<'_> {
    fn insert_str(&mut self, value: &str) -> Result<StrHash, Infallible> {
        let key = StrHash::new(value);
        self.batch
            .put_cf(self.store.id2str_cf(), &key.to_be_bytes(), value);
        Ok(key)
    }
}

impl WritableEncodedStore for BatchWriter<'_> {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<(), Infallible> {
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

        Ok(())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<(), Infallible> {
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
        self.buffer.clear();

        Ok(())
    }
}

impl BatchWriter<'_> {
    fn apply(self) -> Result<(), io::Error> {
        self.store.db.write(self.batch).map_err(map_err)
    }
}

struct AutoBatchWriter<'a> {
    inner: BatchWriter<'a>,
}

impl WithStoreError for AutoBatchWriter<'_> {
    type Error = io::Error;
}

impl StrContainer for AutoBatchWriter<'_> {
    fn insert_str(&mut self, value: &str) -> Result<StrHash, io::Error> {
        Ok(self.inner.insert_str(value).unwrap_infallible())
    }
}

impl WritableEncodedStore for AutoBatchWriter<'_> {
    fn insert_encoded(&mut self, quad: &EncodedQuad) -> Result<(), io::Error> {
        self.inner.insert_encoded(quad).unwrap_infallible();
        self.apply_if_big()
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<(), io::Error> {
        self.inner.remove_encoded(quad).unwrap_infallible();
        self.apply_if_big()
    }
}

impl AutoBatchWriter<'_> {
    fn apply(self) -> Result<(), io::Error> {
        self.inner.apply()
    }

    fn apply_if_big(&mut self) -> Result<(), io::Error> {
        if self.inner.batch.len() > MAX_TRANSACTION_SIZE {
            self.inner
                .store
                .db
                .write(take(&mut self.inner.batch))
                .map_err(map_err)?;
        }
        Ok(())
    }
}

#[allow(clippy::expect_used)]
fn get_cf<'a>(db: &'a DB, name: &str) -> &'a ColumnFamily {
    db.cf_handle(name)
        .expect("A column family that should exist in RocksDB does not exist")
}

fn encode_term(t: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t);
    vec
}

fn encode_term_pair(t1: EncodedTerm, t2: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(2 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    vec
}

fn encode_term_triple(t1: EncodedTerm, t2: EncodedTerm, t3: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(3 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    write_term(&mut vec, t3);
    vec
}

fn encode_term_quad(t1: EncodedTerm, t2: EncodedTerm, t3: EncodedTerm, t4: EncodedTerm) -> Vec<u8> {
    let mut vec = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
    write_term(&mut vec, t1);
    write_term(&mut vec, t2);
    write_term(&mut vec, t3);
    write_term(&mut vec, t4);
    vec
}

struct StaticDBRowIterator {
    iter: DBRawIterator<'static>,
    _db: Arc<DB>, // needed to ensure that DB still lives while iter is used
}

impl StaticDBRowIterator {
    /// Creates a static iterator from a non static one by keeping a ARC reference to the database
    /// Caller must unsure that the iterator belongs to the same database
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
}

pub(crate) struct DecodingIndexIterator {
    iter: StaticDBRowIterator,
    prefix: Vec<u8>,
    encoding: QuadEncoding,
}

impl Iterator for DecodingIndexIterator {
    type Item = Result<EncodedQuad, io::Error>;

    fn next(&mut self) -> Option<Result<EncodedQuad, io::Error>> {
        if let Some(key) = self.iter.iter.key() {
            if key.starts_with(&self.prefix) {
                let result = self.encoding.decode(key);
                self.iter.iter.next();
                Some(result)
            } else {
                None
            }
        } else {
            None
        }
    }
}

fn write_spog_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
    write_term(sink, quad.graph_name);
}

fn write_posg_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
    write_term(sink, quad.graph_name);
}

fn write_ospg_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
    write_term(sink, quad.graph_name);
}

fn write_gspo_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.graph_name);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
}

fn write_gpos_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.graph_name);
    write_term(sink, quad.predicate);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
}

fn write_gosp_quad(sink: &mut Vec<u8>, quad: &EncodedQuad) {
    write_term(sink, quad.graph_name);
    write_term(sink, quad.object);
    write_term(sink, quad.subject);
    write_term(sink, quad.predicate);
}

#[derive(Clone, Copy)]
enum QuadEncoding {
    SPOG,
    POSG,
    OSPG,
    GSPO,
    GPOS,
    GOSP,
}

impl QuadEncoding {
    fn decode(self, buffer: &[u8]) -> Result<EncodedQuad, io::Error> {
        let mut cursor = Cursor::new(&buffer);
        match self {
            QuadEncoding::SPOG => cursor.read_spog_quad(),
            QuadEncoding::POSG => cursor.read_posg_quad(),
            QuadEncoding::OSPG => cursor.read_ospg_quad(),
            QuadEncoding::GSPO => cursor.read_gspo_quad(),
            QuadEncoding::GPOS => cursor.read_gpos_quad(),
            QuadEncoding::GOSP => cursor.read_gosp_quad(),
        }
    }
}

fn map_err(e: Error) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
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

    let main_quad = Quad::new(main_s.clone(), main_p.clone(), main_o.clone(), None);
    let all_o = vec![
        Quad::new(main_s.clone(), main_p.clone(), Literal::from(0), None),
        Quad::new(main_s.clone(), main_p.clone(), main_o.clone(), None),
        Quad::new(main_s.clone(), main_p.clone(), Literal::from(2), None),
    ];

    let mut repo_path = temp_dir();
    repo_path.push(random::<u128>().to_string());

    {
        let store = RocksDbStore::open(&repo_path)?;
        store.insert(&main_quad)?;
        for t in &all_o {
            store.insert(t)?;
        }

        let target = vec![main_quad];
        assert_eq!(
            store
                .quads_for_pattern(None, None, None, None)
                .collect::<Result<Vec<_>, _>>()?,
            all_o
        );
        assert_eq!(
            store
                .quads_for_pattern(Some(&main_s), None, None, None)
                .collect::<Result<Vec<_>, _>>()?,
            all_o
        );
        assert_eq!(
            store
                .quads_for_pattern(Some(&main_s), Some(&main_p), None, None)
                .collect::<Result<Vec<_>, _>>()?,
            all_o
        );
        assert_eq!(
            store
                .quads_for_pattern(Some(&main_s), Some(&main_p), Some(&main_o), None)
                .collect::<Result<Vec<_>, _>>()?,
            target
        );
        assert_eq!(
            store
                .quads_for_pattern(
                    Some(&main_s),
                    Some(&main_p),
                    Some(&main_o),
                    Some(&GraphName::DefaultGraph)
                )
                .collect::<Result<Vec<_>, _>>()?,
            target
        );
        assert_eq!(
            store
                .quads_for_pattern(
                    Some(&main_s),
                    Some(&main_p),
                    None,
                    Some(&GraphName::DefaultGraph)
                )
                .collect::<Result<Vec<_>, _>>()?,
            all_o
        );
        assert_eq!(
            store
                .quads_for_pattern(Some(&main_s), None, Some(&main_o), None)
                .collect::<Result<Vec<_>, _>>()?,
            target
        );
        assert_eq!(
            store
                .quads_for_pattern(
                    Some(&main_s),
                    None,
                    Some(&main_o),
                    Some(&GraphName::DefaultGraph)
                )
                .collect::<Result<Vec<_>, _>>()?,
            target
        );
        assert_eq!(
            store
                .quads_for_pattern(Some(&main_s), None, None, Some(&GraphName::DefaultGraph))
                .collect::<Result<Vec<_>, _>>()?,
            all_o
        );
        assert_eq!(
            store
                .quads_for_pattern(None, Some(&main_p), None, None)
                .collect::<Result<Vec<_>, _>>()?,
            all_o
        );
        assert_eq!(
            store
                .quads_for_pattern(None, Some(&main_p), Some(&main_o), None)
                .collect::<Result<Vec<_>, _>>()?,
            target
        );
        assert_eq!(
            store
                .quads_for_pattern(None, None, Some(&main_o), None)
                .collect::<Result<Vec<_>, _>>()?,
            target
        );
        assert_eq!(
            store
                .quads_for_pattern(None, None, None, Some(&GraphName::DefaultGraph))
                .collect::<Result<Vec<_>, _>>()?,
            all_o
        );
        assert_eq!(
            store
                .quads_for_pattern(
                    None,
                    Some(&main_p),
                    Some(&main_o),
                    Some(&GraphName::DefaultGraph)
                )
                .collect::<Result<Vec<_>, _>>()?,
            target
        );
    }

    remove_dir_all(&repo_path)?;
    Ok(())
}
