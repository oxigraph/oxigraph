mod storage;

use errors::*;
use model::*;
use std::path::Path;
use store::numeric_encoder::EncodedQuad;
use store::numeric_encoder::Encoder;
use store::rocksdb::storage::*;

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

    pub fn iter(&self) -> Result<QuadsIterator<SPOGIndexIterator>> {
        Ok(QuadsIterator {
            iter: self.store.quads()?,
            encoder: self.store.encoder(),
        })
    }

    pub fn quads_for_subject(
        &self,
        subject: &NamedOrBlankNode,
    ) -> Result<QuadsIterator<FilteringEncodedQuadsIterator<SPOGIndexIterator>>> {
        Ok(QuadsIterator {
            iter: self
                .store
                .quads_for_subject(self.store.encoder().encode_named_or_blank_node(subject)?)?,
            encoder: self.store.encoder(),
        })
    }

    pub fn contains(&self, quad: &Quad) -> Result<bool> {
        self.store
            .contains(&self.store.encoder().encode_quad(quad)?)
    }

    pub fn insert(&self, quad: &Quad) -> Result<()> {
        self.store.insert(&self.store.encoder().encode_quad(quad)?)
    }

    pub fn remove(&self, quad: &Quad) -> Result<()> {
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

pub struct QuadsIterator<'a, I: Iterator<Item = Result<EncodedQuad>>> {
    iter: I,
    encoder: Encoder<RocksDbBytesStore<'a>>,
}

impl<'a, I: Iterator<Item = Result<EncodedQuad>>> Iterator for QuadsIterator<'a, I> {
    type Item = Result<Quad>;

    fn next(&mut self) -> Option<Result<Quad>> {
        self.iter
            .next()
            .map(|k| k.and_then(|quad| self.encoder.decode_quad(&quad)))
    }
}
