use errors::*;
use rocksdb::ColumnFamily;
use rocksdb::DBRawIterator;
use rocksdb::DBVector;
use rocksdb::Options;
use rocksdb::WriteBatch;
use rocksdb::DB;
use std::mem::size_of;
use std::path::Path;
use std::str;
use std::sync::Mutex;
use store::numeric_encoder::*;
use utils::from_bytes;
use utils::to_bytes;

const ID2STR_CF: &'static str = "id2str";
const STR2ID_CF: &'static str = "id2str";
const SPOG_CF: &'static str = "spog";
const POSG_CF: &'static str = "posg";
const OSPG_CF: &'static str = "ospg";

const EMPTY_BUF: [u8; 0] = [0 as u8; 0];

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

const COLUMN_FAMILIES: [&'static str; 5] = [ID2STR_CF, STR2ID_CF, SPOG_CF, POSG_CF, OSPG_CF];

pub struct RocksDbStore {
    db: DB,
    str_id_counter: Mutex<RocksDBCounter>,
    id2str_cf: ColumnFamily,
    str2id_cf: ColumnFamily,
    spog_cf: ColumnFamily,
    posg_cf: ColumnFamily,
    ospg_cf: ColumnFamily,
}

impl RocksDbStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut options = Options::default();
        options.create_if_missing(true);
        options.create_missing_column_families(true);

        let db = DB::open_cf(&options, path, &COLUMN_FAMILIES)?;
        let id2str_cf = get_cf(&db, STR2ID_CF)?;
        let str2id_cf = get_cf(&db, ID2STR_CF)?;
        let spog_cf = get_cf(&db, SPOG_CF)?;
        let posg_cf = get_cf(&db, POSG_CF)?;
        let ospg_cf = get_cf(&db, OSPG_CF)?;

        Ok(Self {
            db,
            str_id_counter: Mutex::new(RocksDBCounter::new("bsc")),
            id2str_cf,
            str2id_cf,
            spog_cf,
            posg_cf,
            ospg_cf,
        })
    }

    pub fn encoder(&self) -> Encoder<RocksDbBytesStore> {
        Encoder::new(RocksDbBytesStore(&self))
    }

    pub fn quads(&self) -> Result<SPOGIndexIterator> {
        let mut iter = self.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek_to_first();
        Ok(SPOGIndexIterator { iter })
    }

    pub fn quads_for_subject(
        &self,
        subject: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<SPOGIndexIterator>> {
        let mut iter = self.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek(subject.as_ref());
        Ok(FilteringEncodedQuadsIterator {
            iter: SPOGIndexIterator { iter },
            filter: EncodedQuadPattern::new(Some(subject), None, None, None),
        })
    }

    pub fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<SPOGIndexIterator>> {
        let mut iter = self.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek(&encode_term_pair(&subject, &predicate));
        Ok(FilteringEncodedQuadsIterator {
            iter: SPOGIndexIterator { iter },
            filter: EncodedQuadPattern::new(Some(subject), Some(predicate), None, None),
        })
    }

    pub fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<SPOGIndexIterator>> {
        let mut iter = self.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek(&encode_term_triple(&subject, &predicate, &object));
        Ok(FilteringEncodedQuadsIterator {
            iter: SPOGIndexIterator { iter },
            filter: EncodedQuadPattern::new(Some(subject), Some(predicate), Some(object), None),
        })
    }

    pub fn quads_for_predicate(
        &self,
        predicate: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<POSGIndexIterator>> {
        let mut iter = self.db.raw_iterator_cf(self.posg_cf)?;
        iter.seek(predicate.as_ref());
        Ok(FilteringEncodedQuadsIterator {
            iter: POSGIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, Some(predicate), None, None),
        })
    }

    pub fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<POSGIndexIterator>> {
        let mut iter = self.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek(&encode_term_pair(&predicate, &object));
        Ok(FilteringEncodedQuadsIterator {
            iter: POSGIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, Some(predicate), Some(object), None),
        })
    }

    pub fn quads_for_object(
        &self,
        object: EncodedTerm,
    ) -> Result<FilteringEncodedQuadsIterator<OSPGIndexIterator>> {
        let mut iter = self.db.raw_iterator_cf(self.ospg_cf)?;
        iter.seek(object.as_ref());
        Ok(FilteringEncodedQuadsIterator {
            iter: OSPGIndexIterator { iter },
            filter: EncodedQuadPattern::new(None, None, Some(object), None),
        })
    }

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        Ok(self.db.get_cf(self.spog_cf, &quad.spog())?.is_some())
    }

    pub fn insert(&self, quad: &EncodedQuad) -> Result<()> {
        let mut batch = WriteBatch::default();
        batch.put_cf(self.spog_cf, &quad.spog(), &EMPTY_BUF)?;
        batch.put_cf(self.posg_cf, &quad.posg(), &EMPTY_BUF)?;
        batch.put_cf(self.ospg_cf, &quad.ospg(), &EMPTY_BUF)?;
        Ok(self.db.write(batch)?) //TODO: check what's going on if the key already exists
    }

    pub fn remove(&self, quad: &EncodedQuad) -> Result<()> {
        let mut batch = WriteBatch::default();
        batch.delete_cf(self.spog_cf, &quad.spog())?;
        batch.delete_cf(self.posg_cf, &quad.posg())?;
        batch.delete_cf(self.ospg_cf, &quad.ospg())?;
        Ok(self.db.write(batch)?)
    }
}

pub fn get_cf(db: &DB, name: &str) -> Result<ColumnFamily> {
    db.cf_handle(name)
        .ok_or_else(|| Error::from("column family not found"))
}

pub struct RocksDbBytesStore<'a>(&'a RocksDbStore);

impl<'a> BytesStore for RocksDbBytesStore<'a> {
    type BytesOutput = DBVector;

    fn put(&self, value: &[u8], id_buffer: &mut [u8]) -> Result<()> {
        match self.0.db.get_cf(self.0.str2id_cf, value)? {
            Some(id) => id_buffer.copy_from_slice(&id),
            None => {
                let id = to_bytes(
                    self.0
                        .str_id_counter
                        .lock()
                        .unwrap()
                        .get_and_increment(&self.0.db)?,
                );
                let mut batch = WriteBatch::default();
                batch.put_cf(self.0.id2str_cf, &id, value)?;
                batch.put_cf(self.0.str2id_cf, value, &id)?;
                self.0.db.write(batch)?;
                id_buffer.copy_from_slice(&id)
            }
        }
        Ok(())
    }

    fn get(&self, id: &[u8]) -> Result<Option<DBVector>> {
        Ok(self.0.db.get_cf(self.0.id2str_cf, id)?)
    }
}

struct RocksDBCounter {
    name: &'static str,
}

impl RocksDBCounter {
    fn new(name: &'static str) -> Self {
        Self { name }
    }

    fn get_and_increment(&self, db: &DB) -> Result<usize> {
        let value = db
            .get(self.name.as_bytes())?
            .map(|b| {
                let mut buf = [0 as u8; size_of::<usize>()];
                buf.copy_from_slice(&b);
                from_bytes(buf)
            })
            .unwrap_or(0);
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

fn encode_term_pair(t1: &EncodedTerm, t2: &EncodedTerm) -> [u8; 2 * TERM_ENCODING_SIZE] {
    let mut bytes = [0 as u8; 2 * TERM_ENCODING_SIZE];
    bytes[0..TERM_ENCODING_SIZE].copy_from_slice(t1.as_ref());
    bytes[TERM_ENCODING_SIZE..2 * TERM_ENCODING_SIZE].copy_from_slice(t2.as_ref());
    bytes
}

fn encode_term_triple(
    t1: &EncodedTerm,
    t2: &EncodedTerm,
    t3: &EncodedTerm,
) -> [u8; 3 * TERM_ENCODING_SIZE] {
    let mut bytes = [0 as u8; 3 * TERM_ENCODING_SIZE];
    bytes[0..TERM_ENCODING_SIZE].copy_from_slice(t1.as_ref());
    bytes[TERM_ENCODING_SIZE..2 * TERM_ENCODING_SIZE].copy_from_slice(t2.as_ref());
    bytes[2 * TERM_ENCODING_SIZE..3 * TERM_ENCODING_SIZE].copy_from_slice(t3.as_ref());
    bytes
}

pub struct SPOGIndexIterator {
    iter: DBRawIterator,
}

impl Iterator for SPOGIndexIterator {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        self.iter.next();
        self.iter
            .key()
            .map(|buffer| EncodedQuad::new_from_spog_buffer(&buffer))
    }
}

pub struct POSGIndexIterator {
    iter: DBRawIterator,
}

impl Iterator for POSGIndexIterator {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        self.iter.next();
        self.iter
            .key()
            .map(|buffer| EncodedQuad::new_from_posg_buffer(&buffer))
    }
}

pub struct OSPGIndexIterator {
    iter: DBRawIterator,
}

impl Iterator for OSPGIndexIterator {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        self.iter.next();
        self.iter
            .key()
            .map(|buffer| EncodedQuad::new_from_ospg_buffer(&buffer))
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
            Err(e) => true,
        })
    }
}
