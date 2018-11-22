use byteorder::ByteOrder;
use byteorder::LittleEndian;
use failure::Backtrace;
use rocksdb::ColumnFamily;
use rocksdb::DBRawIterator;
use rocksdb::DBVector;
use rocksdb::Options;
use rocksdb::WriteBatch;
use rocksdb::DB;
use std::io::Cursor;
use std::ops::Deref;
use std::path::Path;
use std::str;
use std::sync::Mutex;
use std::sync::PoisonError;
use store::encoded::EncodedQuadsStore;
use store::encoded::StoreDataset;
use store::numeric_encoder::*;
use Result;

/// `rudf::model::Dataset` trait implementation based on the [RocksDB](https://rocksdb.org/) key-value store
///
/// To use it, the `"rocksdb"` feature need to be activated.
///
/// Usage example:
/// ```
/// use rudf::store::RocksDbDataset;
/// let dataset = RocksDbDataset::open("example.db").unwrap();
/// ```
pub type RocksDbDataset = StoreDataset<RocksDbStore>;

impl RocksDbDataset {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self::new_from_store(RocksDbStore::open(path)?))
    }
}

const ID2STR_CF: &str = "id2str";
const STR2ID_CF: &str = "id2str";
const SPOG_CF: &str = "spog";
const POSG_CF: &str = "posg";
const OSPG_CF: &str = "ospg";

const EMPTY_BUF: [u8; 0] = [0 as u8; 0];

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

const COLUMN_FAMILIES: [&str; 5] = [ID2STR_CF, STR2ID_CF, SPOG_CF, POSG_CF, OSPG_CF];

pub struct RocksDbStore {
    db: DB,
    str_id_counter: Mutex<RocksDBCounter>,
    id2str_cf: SendColumnFamily,
    str2id_cf: SendColumnFamily,
    spog_cf: SendColumnFamily,
    posg_cf: SendColumnFamily,
    ospg_cf: SendColumnFamily,
}

impl RocksDbStore {
    fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);

        let db = DB::open_cf(&options, path, &COLUMN_FAMILIES)?;
        let id2str_cf = SendColumnFamily(get_cf(&db, STR2ID_CF)?);
        let str2id_cf = SendColumnFamily(get_cf(&db, ID2STR_CF)?);
        let spog_cf = SendColumnFamily(get_cf(&db, SPOG_CF)?);
        let posg_cf = SendColumnFamily(get_cf(&db, POSG_CF)?);
        let ospg_cf = SendColumnFamily(get_cf(&db, OSPG_CF)?);

        let new = Self {
            db,
            str_id_counter: Mutex::new(RocksDBCounter::new("bsc")),
            id2str_cf,
            str2id_cf,
            spog_cf,
            posg_cf,
            ospg_cf,
        };
        new.set_first_strings()?;
        Ok(new)
    }
}

impl BytesStore for RocksDbStore {
    type BytesOutput = DBVector;

    fn insert_bytes(&self, value: &[u8]) -> Result<u64> {
        Ok(if let Some(id) = self.db.get_cf(*self.str2id_cf, value)? {
            LittleEndian::read_u64(&id)
        } else {
            let id = self
                .str_id_counter
                .lock()
                .map_err(RocksDBCounterMutexPoisonError::from)?
                .get_and_increment(&self.db)? as u64;
            let id_bytes = to_bytes(id);
            let mut batch = WriteBatch::default();
            batch.put_cf(*self.id2str_cf, &id_bytes, value)?;
            batch.put_cf(*self.str2id_cf, value, &id_bytes)?;
            self.db.write(batch)?;
            id
        })
    }

    fn get_bytes(&self, id: u64) -> Result<Option<DBVector>> {
        Ok(self.db.get_cf(*self.id2str_cf, &to_bytes(id))?)
    }
}

impl EncodedQuadsStore for RocksDbStore {
    type QuadsIterator = SPOGIndexIterator;
    type QuadsForSubjectIterator = FilteringEncodedQuadsIterator<SPOGIndexIterator>;
    type QuadsForSubjectPredicateIterator = FilteringEncodedQuadsIterator<SPOGIndexIterator>;
    type QuadsForSubjectPredicateObjectIterator = FilteringEncodedQuadsIterator<SPOGIndexIterator>;
    type QuadsForSubjectObjectIterator = FilteringEncodedQuadsIterator<OSPGIndexIterator>;
    type QuadsForPredicateIterator = FilteringEncodedQuadsIterator<POSGIndexIterator>;
    type QuadsForPredicateObjectIterator = FilteringEncodedQuadsIterator<POSGIndexIterator>;
    type QuadsForObjectIterator = FilteringEncodedQuadsIterator<OSPGIndexIterator>;
    type QuadsForGraphIterator = InGraphQuadsIterator<SPOGIndexIterator>;
    type QuadsForSubjectGraphIterator =
        InGraphQuadsIterator<FilteringEncodedQuadsIterator<SPOGIndexIterator>>;
    type QuadsForSubjectPredicateGraphIterator =
        InGraphQuadsIterator<FilteringEncodedQuadsIterator<SPOGIndexIterator>>;
    type QuadsForSubjectObjectGraphIterator =
        InGraphQuadsIterator<FilteringEncodedQuadsIterator<OSPGIndexIterator>>;
    type QuadsForPredicateGraphIterator =
        InGraphQuadsIterator<FilteringEncodedQuadsIterator<POSGIndexIterator>>;
    type QuadsForPredicateObjectGraphIterator =
        InGraphQuadsIterator<FilteringEncodedQuadsIterator<POSGIndexIterator>>;
    type QuadsForObjectGraphIterator =
        InGraphQuadsIterator<FilteringEncodedQuadsIterator<OSPGIndexIterator>>;

    fn quads(&self) -> Result<SPOGIndexIterator> {
        let mut iter = self.db.raw_iterator_cf(*self.spog_cf)?;
        iter.seek_to_first();
        Ok(SPOGIndexIterator { iter })
    }

    fn quads_for_subject(
        &self,
        subject: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<SPOGIndexIterator>> {
        let mut iter = self.db.raw_iterator_cf(*self.spog_cf)?;
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
        let mut iter = self.db.raw_iterator_cf(*self.spog_cf)?;
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
        let mut iter = self.db.raw_iterator_cf(*self.spog_cf)?;
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
        let mut iter = self.db.raw_iterator_cf(*self.spog_cf)?;
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
        let mut iter = self.db.raw_iterator_cf(*self.posg_cf)?;
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
        let mut iter = self.db.raw_iterator_cf(*self.spog_cf)?;
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
        let mut iter = self.db.raw_iterator_cf(*self.ospg_cf)?;
        iter.seek(&encode_term(object)?);
        Ok(FilteringEncodedQuadsIterator {
            iter: OSPGIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, None, Some(object), None),
        })
    }

    fn quads_for_graph(
        &self,
        graph_name: EncodedTerm,
    ) -> Result<InGraphQuadsIterator<SPOGIndexIterator>> {
        Ok(InGraphQuadsIterator {
            iter: self.quads()?,
            graph_name,
        })
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<InGraphQuadsIterator<FilteringEncodedQuadsIterator<SPOGIndexIterator>>> {
        Ok(InGraphQuadsIterator {
            iter: self.quads_for_subject(subject)?,
            graph_name,
        })
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<InGraphQuadsIterator<FilteringEncodedQuadsIterator<SPOGIndexIterator>>> {
        Ok(InGraphQuadsIterator {
            iter: self.quads_for_subject_predicate(subject, predicate)?,
            graph_name,
        })
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<InGraphQuadsIterator<FilteringEncodedQuadsIterator<OSPGIndexIterator>>> {
        Ok(InGraphQuadsIterator {
            iter: self.quads_for_subject_object(subject, object)?,
            graph_name,
        })
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<InGraphQuadsIterator<FilteringEncodedQuadsIterator<POSGIndexIterator>>> {
        Ok(InGraphQuadsIterator {
            iter: self.quads_for_predicate(predicate)?,
            graph_name,
        })
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<InGraphQuadsIterator<FilteringEncodedQuadsIterator<POSGIndexIterator>>> {
        Ok(InGraphQuadsIterator {
            iter: self.quads_for_predicate_object(predicate, object)?,
            graph_name,
        })
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> Result<InGraphQuadsIterator<FilteringEncodedQuadsIterator<OSPGIndexIterator>>> {
        Ok(InGraphQuadsIterator {
            iter: self.quads_for_object(object)?,
            graph_name,
        })
    }

    fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        Ok(self
            .db
            .get_cf(*self.spog_cf, &encode_spog_quad(quad)?)?
            .is_some())
    }

    fn insert(&self, quad: &EncodedQuad) -> Result<()> {
        let mut batch = WriteBatch::default();
        batch.put_cf(*self.spog_cf, &encode_spog_quad(quad)?, &EMPTY_BUF)?;
        batch.put_cf(*self.posg_cf, &encode_posg_quad(quad)?, &EMPTY_BUF)?;
        batch.put_cf(*self.ospg_cf, &encode_ospg_quad(quad)?, &EMPTY_BUF)?;
        self.db.write(batch)?; //TODO: check what's going on if the key already exists
        Ok(())
    }

    fn remove(&self, quad: &EncodedQuad) -> Result<()> {
        let mut batch = WriteBatch::default();
        batch.delete_cf(*self.spog_cf, &encode_spog_quad(quad)?)?;
        batch.delete_cf(*self.posg_cf, &encode_posg_quad(quad)?)?;
        batch.delete_cf(*self.ospg_cf, &encode_ospg_quad(quad)?)?;
        self.db.write(batch)?;
        Ok(())
    }
}

pub fn get_cf(db: &DB, name: &str) -> Result<ColumnFamily> {
    db.cf_handle(name)
        .ok_or_else(|| format_err!("column family not found"))
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
    let mut vec = Vec::default();
    vec.write_term(t)?;
    Ok(vec)
}

fn encode_term_pair(t1: EncodedTerm, t2: EncodedTerm) -> Result<Vec<u8>> {
    let mut vec = Vec::default();
    vec.write_term(t1)?;
    vec.write_term(t2)?;
    Ok(vec)
}

fn encode_term_triple(t1: EncodedTerm, t2: EncodedTerm, t3: EncodedTerm) -> Result<Vec<u8>> {
    let mut vec = Vec::default();
    vec.write_term(t1)?;
    vec.write_term(t2)?;
    vec.write_term(t3)?;
    Ok(vec)
}

fn encode_spog_quad(quad: &EncodedQuad) -> Result<Vec<u8>> {
    let mut vec = Vec::default();
    vec.write_spog_quad(quad)?;
    Ok(vec)
}

fn encode_posg_quad(quad: &EncodedQuad) -> Result<Vec<u8>> {
    let mut vec = Vec::default();
    vec.write_posg_quad(quad)?;
    Ok(vec)
}

fn encode_ospg_quad(quad: &EncodedQuad) -> Result<Vec<u8>> {
    let mut vec = Vec::default();
    vec.write_ospg_quad(quad)?;
    Ok(vec)
}

pub struct SPOGIndexIterator {
    iter: DBRawIterator,
}

impl Iterator for SPOGIndexIterator {
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

pub struct POSGIndexIterator {
    iter: DBRawIterator,
}

impl Iterator for POSGIndexIterator {
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

pub struct OSPGIndexIterator {
    iter: DBRawIterator,
}

impl Iterator for OSPGIndexIterator {
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

pub struct FilteringEncodedQuadsIterator<I: Iterator<Item = Result<EncodedQuad>>> {
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

pub struct InGraphQuadsIterator<I: Iterator<Item = Result<EncodedQuad>>> {
    iter: I,
    graph_name: EncodedTerm,
}

impl<I: Iterator<Item = Result<EncodedQuad>>> Iterator for InGraphQuadsIterator<I> {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        let graph_name = &self.graph_name;
        self.iter.find(|quad| match quad {
            Ok(quad) => graph_name == &quad.graph_name,
            Err(_) => true,
        })
    }
}

fn to_bytes(int: u64) -> [u8; 8] {
    let mut buf = [0 as u8; 8];
    LittleEndian::write_u64(&mut buf, int);
    buf
}

#[derive(Debug, Fail)]
#[fail(display = "RocksDBStore Mutex was poisoned")]
pub struct RocksDBCounterMutexPoisonError {
    backtrace: Backtrace,
}

impl<T> From<PoisonError<T>> for RocksDBCounterMutexPoisonError {
    fn from(_: PoisonError<T>) -> Self {
        Self {
            backtrace: Backtrace::new(),
        }
    }
}

// TODO: very bad but I believe it is fine
#[derive(Clone, Copy)]
struct SendColumnFamily(ColumnFamily);
unsafe impl Send for SendColumnFamily {}
unsafe impl Sync for SendColumnFamily {}

impl Deref for SendColumnFamily {
    type Target = ColumnFamily;

    fn deref(&self) -> &ColumnFamily {
        &self.0
    }
}
