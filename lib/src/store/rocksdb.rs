use crate::store::numeric_encoder::*;
use crate::store::{Store, StoreConnection, StoreRepositoryConnection};
use crate::{Repository, Result};
use byteorder::ByteOrder;
use byteorder::LittleEndian;
use failure::format_err;
use rocksdb::ColumnFamily;
use rocksdb::DBCompactionStyle;
use rocksdb::DBRawIterator;
use rocksdb::DBVector;
use rocksdb::Options;
use rocksdb::WriteBatch;
use rocksdb::DB;
use std::io::Cursor;
use std::iter::{empty, once};
use std::ops::Deref;
use std::path::Path;
use std::str;
use std::sync::Mutex;

/// `Repository` implementation based on the [RocksDB](https://rocksdb.org/) key-value store
///
/// To use it, the `"rocksdb"` feature need to be activated.
///
/// Usage example:
/// ```ignored
/// use rudf::model::*;
/// use rudf::{Repository, RepositoryConnection, RocksDbRepository, Result};
/// use crate::rudf::sparql::PreparedQuery;
/// use rudf::sparql::QueryResult;
///
/// let repository = RocksDbRepository::open("example.db").unwrap();
/// let mut connection = repository.connection().unwrap();
///
/// // insertion
/// let ex = NamedNode::parse("http://example.com").unwrap();
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
/// connection.insert(&quad);
///
/// // quad filter
/// let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
/// assert_eq!(vec![quad], results.unwrap());
///
/// // SPARQL query
/// let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", None).unwrap();
/// let results = prepared_query.exec().unwrap();
/// if let QueryResult::Bindings(results) = results {
///     assert_eq!(results.into_values_iter().next().unwrap().unwrap()[0], Some(ex.into()));
/// }
/// ```
pub struct RocksDbRepository {
    inner: RocksDbStore,
}

pub type RocksDbRepositoryConnection<'a> = StoreRepositoryConnection<RocksDbStoreConnection<'a>>;

const ID2STR_CF: &str = "id2str";
const STR2ID_CF: &str = "id2str";
const SPOG_CF: &str = "spog";
const POSG_CF: &str = "posg";
const OSPG_CF: &str = "ospg";
const GSPO_CF: &str = "gspo";
const GPOS_CF: &str = "gpos";
const GOSP_CF: &str = "gosp";

const EMPTY_BUF: [u8; 0] = [0 as u8; 0];

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

const COLUMN_FAMILIES: [&str; 8] = [
    ID2STR_CF, STR2ID_CF, SPOG_CF, POSG_CF, OSPG_CF, GSPO_CF, GPOS_CF, GOSP_CF,
];

struct RocksDbStore {
    db: DB,
    str_id_counter: Mutex<RocksDBCounter>,
}

#[derive(Clone)]
pub struct RocksDbStoreConnection<'a> {
    store: &'a RocksDbStore,
    buffer: Vec<u8>,
    id2str_cf: ColumnFamily<'a>,
    str2id_cf: ColumnFamily<'a>,
    spog_cf: ColumnFamily<'a>,
    posg_cf: ColumnFamily<'a>,
    ospg_cf: ColumnFamily<'a>,
    gspo_cf: ColumnFamily<'a>,
    gpos_cf: ColumnFamily<'a>,
    gosp_cf: ColumnFamily<'a>,
}

impl RocksDbRepository {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            inner: RocksDbStore::open(path)?,
        })
    }
}

impl<'a> Repository for &'a RocksDbRepository {
    type Connection = RocksDbRepositoryConnection<'a>;

    fn connection(self) -> Result<StoreRepositoryConnection<RocksDbStoreConnection<'a>>> {
        Ok(self.inner.connection()?.into())
    }
}

impl RocksDbStore {
    fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);
        options.set_compaction_style(DBCompactionStyle::Universal);

        let new = Self {
            db: DB::open_cf(&options, path, &COLUMN_FAMILIES)?,
            str_id_counter: Mutex::new(RocksDBCounter::new("bsc")),
        };
        (&new).connection()?.set_first_strings()?;
        Ok(new)
    }
}

impl<'a> Store for &'a RocksDbStore {
    type Connection = RocksDbStoreConnection<'a>;

    fn connection(self) -> Result<RocksDbStoreConnection<'a>> {
        Ok(RocksDbStoreConnection {
            store: self,
            buffer: Vec::default(),
            id2str_cf: get_cf(&self.db, ID2STR_CF)?,
            str2id_cf: get_cf(&self.db, STR2ID_CF)?,
            spog_cf: get_cf(&self.db, SPOG_CF)?,
            posg_cf: get_cf(&self.db, POSG_CF)?,
            ospg_cf: get_cf(&self.db, OSPG_CF)?,
            gspo_cf: get_cf(&self.db, GSPO_CF)?,
            gpos_cf: get_cf(&self.db, GPOS_CF)?,
            gosp_cf: get_cf(&self.db, GOSP_CF)?,
        })
    }
}

impl StringStore for RocksDbStoreConnection<'_> {
    type StringType = RocksString;

    fn get_str(&self, id: u64) -> Result<Option<RocksString>> {
        Ok(self
            .store
            .db
            .get_cf(self.id2str_cf, &to_bytes(id))?
            .map(|v| RocksString { vec: v }))
    }

    fn get_str_id(&self, value: &str) -> Result<Option<u64>> {
        Ok(self
            .store
            .db
            .get_cf(self.str2id_cf, value.as_bytes())?
            .map(|id| LittleEndian::read_u64(&id)))
    }

    fn insert_str(&self, value: &str) -> Result<u64> {
        Ok(if let Some(id) = self.get_str_id(value)? {
            id
        } else {
            let id = self
                .store
                .str_id_counter
                .lock()
                .map_err(MutexPoisonError::from)?
                .get_and_increment(&self.store.db)? as u64;
            let id_bytes = to_bytes(id);
            let mut batch = WriteBatch::default();
            batch.put_cf(self.id2str_cf, &id_bytes, value)?;
            batch.put_cf(self.str2id_cf, value, &id_bytes)?;
            self.store.db.write(batch)?;
            id
        })
    }
}

impl<'a> StoreConnection for RocksDbStoreConnection<'a> {
    fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        buffer.write_spog_quad(quad)?;
        Ok(self.store.db.get_cf(self.spog_cf, &buffer)?.is_some())
    }

    fn insert(&mut self, quad: &EncodedQuad) -> Result<()> {
        let mut batch = WriteBatch::default();

        self.buffer.write_spog_quad(quad)?;
        batch.put_cf(self.spog_cf, &self.buffer, &EMPTY_BUF)?;
        self.buffer.clear();

        self.buffer.write_posg_quad(quad)?;
        batch.put_cf(self.posg_cf, &self.buffer, &EMPTY_BUF)?;
        self.buffer.clear();

        self.buffer.write_ospg_quad(quad)?;
        batch.put_cf(self.ospg_cf, &self.buffer, &EMPTY_BUF)?;
        self.buffer.clear();

        self.buffer.write_gspo_quad(quad)?;
        batch.put_cf(self.gspo_cf, &self.buffer, &EMPTY_BUF)?;
        self.buffer.clear();

        self.buffer.write_gpos_quad(quad)?;
        batch.put_cf(self.gpos_cf, &self.buffer, &EMPTY_BUF)?;
        self.buffer.clear();

        self.buffer.write_gosp_quad(quad)?;
        batch.put_cf(self.gosp_cf, &self.buffer, &EMPTY_BUF)?;
        self.buffer.clear();

        self.store.db.write(batch)?; //TODO: check what's going on if the key already exists
        Ok(())
    }

    fn remove(&mut self, quad: &EncodedQuad) -> Result<()> {
        let mut batch = WriteBatch::default();

        self.buffer.write_spog_quad(quad)?;
        batch.delete_cf(self.spog_cf, &self.buffer)?;
        self.buffer.clear();

        self.buffer.write_posg_quad(quad)?;
        batch.delete_cf(self.posg_cf, &self.buffer)?;
        self.buffer.clear();

        self.buffer.write_ospg_quad(quad)?;
        batch.delete_cf(self.ospg_cf, &self.buffer)?;
        self.buffer.clear();

        self.buffer.write_gspo_quad(quad)?;
        batch.delete_cf(self.gspo_cf, &self.buffer)?;
        self.buffer.clear();

        self.buffer.write_gpos_quad(quad)?;
        batch.delete_cf(self.gpos_cf, &self.buffer)?;
        self.buffer.clear();

        self.buffer.write_gosp_quad(quad)?;
        batch.delete_cf(self.gosp_cf, &self.buffer)?;
        self.buffer.clear();

        self.store.db.write(batch)?;
        Ok(())
    }

    fn quads_for_pattern<'b>(
        &'b self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Box<dyn Iterator<Item = Result<EncodedQuad>> + 'b> {
        match subject {
            Some(subject) => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            let quad = EncodedQuad::new(subject, predicate, object, graph_name);
                            match self.contains(&quad) {
                                Ok(true) => Box::new(once(Ok(quad))),
                                Ok(false) => Box::new(empty()),
                                Err(error) => Box::new(once(Err(error))),
                            }
                        }
                        None => wrap_error(
                            self.quads_for_subject_predicate_object(subject, predicate, object),
                        ),
                    },
                    None => match graph_name {
                        Some(graph_name) => wrap_error(
                            self.quads_for_subject_predicate_graph(subject, predicate, graph_name),
                        ),
                        None => wrap_error(self.quads_for_subject_predicate(subject, predicate)),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => wrap_error(
                            self.quads_for_subject_object_graph(subject, object, graph_name),
                        ),
                        None => wrap_error(self.quads_for_subject_object(subject, object)),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            wrap_error(self.quads_for_subject_graph(subject, graph_name))
                        }
                        None => wrap_error(self.quads_for_subject(subject)),
                    },
                },
            },
            None => match predicate {
                Some(predicate) => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => wrap_error(
                            self.quads_for_predicate_object_graph(predicate, object, graph_name),
                        ),
                        None => wrap_error(self.quads_for_predicate_object(predicate, object)),
                    },
                    None => match graph_name {
                        Some(graph_name) => {
                            wrap_error(self.quads_for_predicate_graph(predicate, graph_name))
                        }
                        None => wrap_error(self.quads_for_predicate(predicate)),
                    },
                },
                None => match object {
                    Some(object) => match graph_name {
                        Some(graph_name) => {
                            wrap_error(self.quads_for_object_graph(object, graph_name))
                        }
                        None => wrap_error(self.quads_for_object(object)),
                    },
                    None => match graph_name {
                        Some(graph_name) => wrap_error(self.quads_for_graph(graph_name)),
                        None => wrap_error(self.quads()),
                    },
                },
            },
        }
    }
}

impl<'a> RocksDbStoreConnection<'a> {
    fn quads(&self) -> Result<SPOGIndexIterator> {
        let mut iter = self.store.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek_to_first();
        Ok(SPOGIndexIterator { iter })
    }

    fn quads_for_subject(
        &self,
        subject: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<SPOGIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek(&encode_term(subject)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: SPOGIndexIterator { iter },
            filter: EncodedQuadPattern::new(Some(subject), None, None, None),
        })
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<SPOGIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek(&encode_term_pair(subject, predicate)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: SPOGIndexIterator { iter },
            filter: EncodedQuadPattern::new(Some(subject), Some(predicate), None, None),
        })
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<SPOGIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek(&encode_term_triple(subject, predicate, object)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: SPOGIndexIterator { iter },
            filter: EncodedQuadPattern::new(Some(subject), Some(predicate), Some(object), None),
        })
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<OSPGIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek(&encode_term_pair(object, subject)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: OSPGIndexIterator { iter },
            filter: EncodedQuadPattern::new(Some(subject), None, Some(object), None),
        })
    }

    fn quads_for_predicate(
        &self,
        predicate: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<POSGIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.posg_cf)?;
        iter.seek(&encode_term(predicate)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: POSGIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, Some(predicate), None, None),
        })
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<POSGIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek(&encode_term_pair(predicate, object)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: POSGIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, Some(predicate), Some(object), None),
        })
    }

    fn quads_for_object(
        &self,
        object: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<OSPGIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.ospg_cf)?;
        iter.seek(&encode_term(object)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: OSPGIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, None, Some(object), None),
        })
    }

    fn quads_for_graph(
        &self,
        graph_name: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<GSPOIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.gspo_cf)?;
        iter.seek(&encode_term(graph_name)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: GSPOIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, None, None, Some(graph_name)),
        })
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<GSPOIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.gspo_cf)?;
        iter.seek(&encode_term_pair(graph_name, subject)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: GSPOIndexIterator { iter },
            filter: EncodedQuadPattern::new(Some(subject), None, None, Some(graph_name)),
        })
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<GSPOIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek(&encode_term_triple(graph_name, subject, predicate)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: GSPOIndexIterator { iter },
            filter: EncodedQuadPattern::new(Some(subject), Some(predicate), None, Some(graph_name)),
        })
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<GOSPIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.gosp_cf)?;
        iter.seek(&encode_term_triple(graph_name, object, subject)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: GOSPIndexIterator { iter },
            filter: EncodedQuadPattern::new(Some(subject), None, Some(object), Some(graph_name)),
        })
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<GPOSIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.gpos_cf)?;
        iter.seek(&encode_term_pair(graph_name, predicate)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: GPOSIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, Some(predicate), None, Some(graph_name)),
        })
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<GPOSIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.gosp_cf)?;
        iter.seek(&encode_term_triple(graph_name, predicate, object)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: GPOSIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, Some(predicate), Some(object), Some(graph_name)),
        })
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<GOSPIndexIterator>> {
        let mut iter = self.store.db.raw_iterator_cf(self.gosp_cf)?;
        iter.seek(&encode_term_pair(graph_name, object)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: GOSPIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, None, Some(object), Some(graph_name)),
        })
    }
}

fn get_cf<'a>(db: &'a DB, name: &str) -> Result<ColumnFamily<'a>> {
    db.cf_handle(name)
        .ok_or_else(|| format_err!("column family {} not found", name))
}

fn wrap_error<'a, E: 'a, I: Iterator<Item = Result<E>> + 'a>(
    iter: Result<I>,
) -> Box<dyn Iterator<Item = Result<E>> + 'a> {
    match iter {
        Ok(iter) => Box::new(iter),
        Err(error) => Box::new(once(Err(error))),
    }
}

struct RocksDBCounter {
    name: &'static str,
}

impl RocksDBCounter {
    fn new(name: &'static str) -> Self {
        Self { name }
    }

    fn get_and_increment(&self, db: &DB) -> Result<u64> {
        let value = db
            .get(self.name.as_bytes())?
            .map_or(0, |b| LittleEndian::read_u64(&b));
        db.put(self.name.as_bytes(), &to_bytes(value + 1))?;
        Ok(value)
    }
}

struct EncodedQuadPattern {
    subject: Option<EncodedTerm>,
    predicate: Option<EncodedTerm>,
    object: Option<EncodedTerm>,
    graph_name: Option<EncodedTerm>,
}

impl EncodedQuadPattern {
    fn new(
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Self {
        Self {
            subject,
            predicate,
            object,
            graph_name,
        }
    }

    fn filter(&self, quad: &EncodedQuad) -> bool {
        if let Some(ref subject) = self.subject {
            if &quad.subject != subject {
                return false;
            }
        }
        if let Some(ref predicate) = self.predicate {
            if &quad.predicate != predicate {
                return false;
            }
        }
        if let Some(ref object) = self.object {
            if &quad.object != object {
                return false;
            }
        }
        if let Some(ref graph_name) = self.graph_name {
            if &quad.graph_name != graph_name {
                return false;
            }
        }
        true
    }
}

fn encode_term(t: EncodedTerm) -> Result<Vec<u8>> {
    let mut vec = Vec::with_capacity(WRITTEN_TERM_MAX_SIZE);
    vec.write_term(t)?;
    Ok(vec)
}

fn encode_term_pair(t1: EncodedTerm, t2: EncodedTerm) -> Result<Vec<u8>> {
    let mut vec = Vec::with_capacity(2 * WRITTEN_TERM_MAX_SIZE);
    vec.write_term(t1)?;
    vec.write_term(t2)?;
    Ok(vec)
}

fn encode_term_triple(t1: EncodedTerm, t2: EncodedTerm, t3: EncodedTerm) -> Result<Vec<u8>> {
    let mut vec = Vec::with_capacity(3 * WRITTEN_TERM_MAX_SIZE);
    vec.write_term(t1)?;
    vec.write_term(t2)?;
    vec.write_term(t3)?;
    Ok(vec)
}

struct GSPOIndexIterator<'a> {
    iter: DBRawIterator<'a>,
}

impl<'a> Iterator for GSPOIndexIterator<'a> {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        self.iter.next();
        unsafe {
            //This is safe because we are not keeping the buffer
            self.iter
                .key_inner()
                .map(|buffer| Cursor::new(buffer).read_gspo_quad())
        }
    }
}

struct GPOSIndexIterator<'a> {
    iter: DBRawIterator<'a>,
}

impl<'a> Iterator for GPOSIndexIterator<'a> {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        self.iter.next();
        unsafe {
            //This is safe because we are not keeping the buffer
            self.iter
                .key_inner()
                .map(|buffer| Cursor::new(buffer).read_gpos_quad())
        }
    }
}

struct GOSPIndexIterator<'a> {
    iter: DBRawIterator<'a>,
}

impl<'a> Iterator for GOSPIndexIterator<'a> {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        self.iter.next();
        unsafe {
            //This is safe because we are not keeping the buffer
            self.iter
                .key_inner()
                .map(|buffer| Cursor::new(buffer).read_gosp_quad())
        }
    }
}

struct SPOGIndexIterator<'a> {
    iter: DBRawIterator<'a>,
}

impl<'a> Iterator for SPOGIndexIterator<'a> {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        self.iter.next();
        unsafe {
            //This is safe because we are not keeping the buffer
            self.iter
                .key_inner()
                .map(|buffer| Cursor::new(buffer).read_spog_quad())
        }
    }
}

struct POSGIndexIterator<'a> {
    iter: DBRawIterator<'a>,
}

impl<'a> Iterator for POSGIndexIterator<'a> {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        self.iter.next();
        unsafe {
            //This is safe because we are not keeping the buffer
            self.iter
                .key_inner()
                .map(|buffer| Cursor::new(buffer).read_posg_quad())
        }
    }
}

struct OSPGIndexIterator<'a> {
    iter: DBRawIterator<'a>,
}

impl<'a> Iterator for OSPGIndexIterator<'a> {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        self.iter.next();
        unsafe {
            //This is safe because we are not keeping the buffer
            self.iter
                .key_inner()
                .map(|buffer| Cursor::new(buffer).read_ospg_quad())
        }
    }
}

struct FilteringEncodedQuadsIterator<I: Iterator<Item = Result<EncodedQuad>>> {
    iter: I,
    filter: EncodedQuadPattern,
}

impl<I: Iterator<Item = Result<EncodedQuad>>> Iterator for FilteringEncodedQuadsIterator<I> {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        self.iter.next().filter(|quad| match quad {
            Ok(quad) => self.filter.filter(quad),
            Err(_) => true,
        })
    }
}

fn to_bytes(int: u64) -> [u8; 8] {
    let mut buf = [0 as u8; 8];
    LittleEndian::write_u64(&mut buf, int);
    buf
}

pub struct RocksString {
    vec: DBVector,
}

impl Deref for RocksString {
    type Target = str;

    fn deref(&self) -> &str {
        unsafe { str::from_utf8_unchecked(&self.vec) }
    }
}

impl ToString for RocksString {
    fn to_string(&self) -> String {
        self.deref().to_owned()
    }
}

impl From<RocksString> for String {
    fn from(val: RocksString) -> String {
        val.deref().to_owned()
    }
}
