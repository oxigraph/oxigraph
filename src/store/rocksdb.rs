use errors::*;
use model::*;
use rocksdb::ColumnFamily;
use rocksdb::DBRawIterator;
use rocksdb::DBVector;
use rocksdb::IteratorMode;
use rocksdb::Options;
use rocksdb::WriteBatch;
use rocksdb::DB;
use std::ops::Deref;
use std::path::Path;
use std::slice;
use std::str;
use store::numeric_encoder::BytesStore;
use store::numeric_encoder::EncodedQuad;
use store::numeric_encoder::EncodedTerm;
use store::numeric_encoder::Encoder;
use store::numeric_encoder::STRING_KEY_SIZE;
use store::numeric_encoder::TERM_ENCODING_SIZE;
use utils::to_bytes;

pub struct RocksDbDataset {
    store: RocksDbStore,
}

impl RocksDbDataset {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            store: RocksDbStore::open(path)?,
        })
    }

    fn graph(&self, name: &NamedOrBlankNode) -> RocksDbGraph {
        RocksDbGraph {
            store: &self.store,
            name: name.clone(),
        }
    }

    fn default_graph(&self) -> RocksDbDefaultGraph {
        RocksDbDefaultGraph { store: &self.store }
    }

    fn union_graph(&self) -> RocksDbUnionGraph {
        RocksDbUnionGraph { store: &self.store }
    }

    fn iter(&self) -> Result<QuadsIterator<SPOGIndexIterator>> {
        Ok(QuadsIterator {
            iter: self.store.quads()?,
            encoder: self.store.encoder(),
        })
    }

    fn quads_for_subject(
        &self,
        subject: &NamedOrBlankNode,
    ) -> Result<QuadsIterator<FilteringEncodedQuadsIterator<SPOGIndexIterator>>> {
        Ok(QuadsIterator {
            iter: self.store
                .quads_for_subject(self.store.encoder().encode_named_or_blank_node(subject)?)?,
            encoder: self.store.encoder(),
        })
    }

    fn contains(&self, quad: &Quad) -> Result<bool> {
        self.store
            .contains(&self.store.encoder().encode_quad(quad)?)
    }

    fn insert(&self, quad: &Quad) -> Result<()> {
        self.store.insert(&self.store.encoder().encode_quad(quad)?)
    }

    fn remove(&self, quad: &Quad) -> Result<()> {
        self.store.remove(&self.store.encoder().encode_quad(quad)?)
    }
}

struct RocksDbGraph<'a> {
    store: &'a RocksDbStore,
    name: NamedOrBlankNode, //TODO: better storage
}

struct RocksDbDefaultGraph<'a> {
    store: &'a RocksDbStore,
}

struct RocksDbUnionGraph<'a> {
    store: &'a RocksDbStore,
}

const ID2STR_CF: &'static str = "id2str";
const STR2ID_CF: &'static str = "id2str";
const SPOG_CF: &'static str = "spog";
const POSG_CF: &'static str = "posg";
const OSPG_CF: &'static str = "ospg";

const EMPTY_BUF: [u8; 0] = [0 as u8; 0];

//TODO: indexes for the default graph and indexes for the named graphs (no more Optional and space saving)

const COLUMN_FAMILIES: [&'static str; 5] = [ID2STR_CF, STR2ID_CF, SPOG_CF, POSG_CF, OSPG_CF];

struct RocksDbStore {
    db: DB,
    id2str_cf: ColumnFamily,
    str2id_cf: ColumnFamily,
    spog_cf: ColumnFamily,
    posg_cf: ColumnFamily,
    ospg_cf: ColumnFamily,
}

impl RocksDbStore {
    fn open(path: impl AsRef<Path>) -> Result<Self> {
        let options = Options::default();

        let db = DB::open_cf(&options, path, &COLUMN_FAMILIES)?;
        let id2str_cf = get_cf(&db, STR2ID_CF)?;
        let str2id_cf = get_cf(&db, ID2STR_CF)?;
        let spog_cf = get_cf(&db, SPOG_CF)?;
        let posg_cf = get_cf(&db, POSG_CF)?;
        let ospg_cf = get_cf(&db, OSPG_CF)?;

        Ok(Self {
            db,
            id2str_cf,
            str2id_cf,
            spog_cf,
            posg_cf,
            ospg_cf,
        })
    }

    fn encoder(&self) -> Encoder<RocksDbBytesStore> {
        Encoder::new(RocksDbBytesStore(&self))
    }

    fn quads(&self) -> Result<SPOGIndexIterator> {
        let mut iter = self.db.raw_iterator_cf(self.spog_cf)?;
        iter.seek_to_first();
        Ok(SPOGIndexIterator { iter })
    }

    fn quads_for_subject(
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

    fn quads_for_subject_predicate(
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

    fn quads_for_subject_predicate_object(
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

    fn quads_for_predicate(
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

    fn quads_for_predicate_object(
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

    fn quads_for_object(
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

    fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        Ok(self.db.get_cf(self.spog_cf, &quad.spog())?.is_some())
    }

    fn insert(&self, quad: &EncodedQuad) -> Result<()> {
        let mut batch = WriteBatch::default();
        batch.put_cf(self.spog_cf, &quad.spog(), &EMPTY_BUF)?;
        batch.put_cf(self.posg_cf, &quad.posg(), &EMPTY_BUF)?;
        batch.put_cf(self.ospg_cf, &quad.ospg(), &EMPTY_BUF)?;
        Ok(self.db.write(batch)?) //TODO: check what's going on if the key already exists
    }

    fn remove(&self, quad: &EncodedQuad) -> Result<()> {
        let mut batch = WriteBatch::default();
        batch.delete_cf(self.spog_cf, &quad.spog())?;
        batch.delete_cf(self.posg_cf, &quad.posg())?;
        batch.delete_cf(self.ospg_cf, &quad.ospg())?;
        Ok(self.db.write(batch)?)
    }
}

fn get_cf(db: &DB, name: &str) -> Result<ColumnFamily> {
    db.cf_handle(name)
        .ok_or_else(|| Error::from("column family not found"))
}

struct RocksDbBytesStore<'a>(&'a RocksDbStore);

impl<'a> BytesStore for RocksDbBytesStore<'a> {
    type BytesOutput = DBVector;

    fn put(&self, value: &[u8], id_buffer: &mut [u8]) -> Result<()> {
        match self.0.db.get_cf(self.0.str2id_cf, value)? {
            Some(id) => id_buffer.copy_from_slice(&id),
            None => {
                let mut batch = WriteBatch::default();
                // TODO: id allocation
                let id = [0 as u8; STRING_KEY_SIZE];
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
    bytes[2 * TERM_ENCODING_SIZE..3 * TERM_ENCODING_SIZE].copy_from_slice(t2.as_ref());
    bytes
}

struct SPOGIndexIterator {
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

struct POSGIndexIterator {
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

struct OSPGIndexIterator {
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

struct FilteringEncodedQuadsIterator<I: Iterator<Item = Result<EncodedQuad>>> {
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

struct QuadsIterator<'a, I: Iterator<Item = Result<EncodedQuad>>> {
    iter: I,
    encoder: Encoder<RocksDbBytesStore<'a>>,
}

impl<'a, I: Iterator<Item = Result<EncodedQuad>>> Iterator for QuadsIterator<'a, I> {
    type Item = Result<Quad>;

    fn next(&mut self) -> Option<Result<Quad>> {
        self.iter
            .next()
            .map(|k| k.and_then(|quad| self.encoder.decode_quad(quad)))
    }
}

/*fn encode_sp(
    encoder: &Encoder<RocksDbBytesStore>,
    subject: &NamedOrBlankNode,
    predicate: &NamedNode,
) -> Result<[u8; 2 * TERM_ENCODING_SIZE]> {
    let mut sp = [0 as u8; 2 * TERM_ENCODING_SIZE];
    encoder.encode_named_or_blank_node(subject, &mut sp)?;
    encoder.encode_named_node(predicate, &mut sp)?;
    Ok(sp)
}

fn encode_po(
    encoder: &Encoder<RocksDbBytesStore>,
    predicate: &NamedNode,
    object: &Term,
) -> Result<[u8; 2 * TERM_ENCODING_SIZE]> {
    let mut po = [0 as u8; 2 * TERM_ENCODING_SIZE];
    encoder.encode_named_node(predicate, &mut po)?;
    encoder.encode_term(object, &mut po)?;
    Ok(po)
}

fn encode_os(
    encoder: &Encoder<RocksDbBytesStore>,
    object: &Term,
    subject: &NamedOrBlankNode,
) -> Result<[u8; 2 * TERM_ENCODING_SIZE]> {
    let mut po = [0 as u8; 2 * TERM_ENCODING_SIZE];
    encoder.encode_term(object, &mut po)?;
    encoder.encode_named_or_blank_node(subject, &mut po)?;
    Ok(po)
}*/
