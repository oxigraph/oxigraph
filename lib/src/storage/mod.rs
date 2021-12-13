use crate::error::invalid_data_error;
use crate::model::{GraphNameRef, NamedOrBlankNodeRef, Quad, QuadRef, TermRef};
use crate::storage::backend::{Reader, Transaction};
#[cfg(not(target_arch = "wasm32"))]
use crate::storage::binary_encoder::LATEST_STORAGE_VERSION;
use crate::storage::binary_encoder::{
    decode_term, encode_term, encode_term_pair, encode_term_quad, encode_term_triple,
    write_gosp_quad, write_gpos_quad, write_gspo_quad, write_osp_quad, write_ospg_quad,
    write_pos_quad, write_posg_quad, write_spo_quad, write_spog_quad, write_term, QuadEncoding,
    WRITTEN_TERM_MAX_SIZE,
};
use crate::storage::numeric_encoder::{insert_term, EncodedQuad, EncodedTerm, StrHash, StrLookup};
use backend::{ColumnFamily, ColumnFamilyDefinition, Db, Iter};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::{HashMap, HashSet};
use std::io::Result;
#[cfg(not(target_arch = "wasm32"))]
use std::mem::take;
#[cfg(not(target_arch = "wasm32"))]
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::thread::spawn;

mod backend;
mod binary_encoder;
pub mod numeric_encoder;
pub mod small_string;

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
const DEFAULT_CF: &str = "default";
#[cfg(not(target_arch = "wasm32"))]
const BULK_LOAD_BATCH_SIZE: usize = 1024 * 1024;

/// Low level storage primitives
#[derive(Clone)]
pub struct Storage {
    db: Db,
    default_cf: ColumnFamily,
    id2str_cf: ColumnFamily,
    spog_cf: ColumnFamily,
    posg_cf: ColumnFamily,
    ospg_cf: ColumnFamily,
    gspo_cf: ColumnFamily,
    gpos_cf: ColumnFamily,
    gosp_cf: ColumnFamily,
    dspo_cf: ColumnFamily,
    dpos_cf: ColumnFamily,
    dosp_cf: ColumnFamily,
    graphs_cf: ColumnFamily,
}

impl Storage {
    pub fn new() -> Result<Self> {
        Self::setup(Db::new(Self::column_families())?)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: &Path) -> Result<Self> {
        Self::setup(Db::open(path, Self::column_families())?)
    }

    fn column_families() -> Vec<ColumnFamilyDefinition> {
        vec![
            ColumnFamilyDefinition {
                name: ID2STR_CF,
                use_iter: false,
                min_prefix_size: 0,
            },
            ColumnFamilyDefinition {
                name: SPOG_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: POSG_CF,
                use_iter: true,
                min_prefix_size: 17, // named node start
            },
            ColumnFamilyDefinition {
                name: OSPG_CF,
                use_iter: true,
                min_prefix_size: 0, // There are small literals...
            },
            ColumnFamilyDefinition {
                name: GSPO_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: GPOS_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: GOSP_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: DSPO_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: DPOS_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: DOSP_CF,
                use_iter: true,
                min_prefix_size: 0, // There are small literals...
            },
            ColumnFamilyDefinition {
                name: GRAPHS_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
        ]
    }

    fn setup(db: Db) -> Result<Self> {
        let this = Self {
            default_cf: db.column_family(DEFAULT_CF).unwrap(),
            id2str_cf: db.column_family(ID2STR_CF).unwrap(),
            spog_cf: db.column_family(SPOG_CF).unwrap(),
            posg_cf: db.column_family(POSG_CF).unwrap(),
            ospg_cf: db.column_family(OSPG_CF).unwrap(),
            gspo_cf: db.column_family(GSPO_CF).unwrap(),
            gpos_cf: db.column_family(GPOS_CF).unwrap(),
            gosp_cf: db.column_family(GOSP_CF).unwrap(),
            dspo_cf: db.column_family(DSPO_CF).unwrap(),
            dpos_cf: db.column_family(DPOS_CF).unwrap(),
            dosp_cf: db.column_family(DOSP_CF).unwrap(),
            graphs_cf: db.column_family(GRAPHS_CF).unwrap(),
            db,
        };
        #[cfg(not(target_arch = "wasm32"))]
        this.migrate()?;
        Ok(this)
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn migrate(&self) -> Result<()> {
        let mut version = self.ensure_version()?;
        if version == 0 {
            // We migrate to v1
            let mut graph_names = HashSet::new();
            for quad in self.snapshot().quads() {
                let quad = quad?;
                if !quad.graph_name.is_default_graph() {
                    graph_names.insert(quad.graph_name);
                }
            }
            let mut graph_names = graph_names
                .into_iter()
                .map(|g| encode_term(&g))
                .collect::<Vec<_>>();
            graph_names.sort_unstable();
            let mut stt_file = self.db.new_sst_file()?;
            for k in graph_names {
                stt_file.insert_empty(&k)?;
            }
            self.db
                .insert_stt_files(&[(&self.graphs_cf, stt_file.finish()?)])?;
            version = 1;
            self.update_version(version)?;
        }

        match version {
            _ if version < LATEST_STORAGE_VERSION => Err(invalid_data_error(format!(
                "The RocksDB database is using the outdated encoding version {}. Automated migration is not supported, please dump the store dataset using a compatible Oxigraph version and load it again using the current version",
                version
            ))),
            LATEST_STORAGE_VERSION => Ok(()),
            _ => Err(invalid_data_error(format!(
                "The RocksDB database is using the too recent version {}. Upgrade to the latest Oxigraph version to load this database",
                version
            )))
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn ensure_version(&self) -> Result<u64> {
        Ok(
            if let Some(version) = self.db.snapshot().get(&self.default_cf, b"oxversion")? {
                let mut buffer = [0; 8];
                buffer.copy_from_slice(&version);
                u64::from_be_bytes(buffer)
            } else {
                self.update_version(LATEST_STORAGE_VERSION)?;
                LATEST_STORAGE_VERSION
            },
        )
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn update_version(&self, version: u64) -> Result<()> {
        self.db.transaction(|mut t| {
            t.insert(&self.default_cf, b"oxversion", &version.to_be_bytes())
        })?;
        self.db.flush(&self.default_cf)
    }

    pub fn snapshot(&self) -> StorageReader {
        StorageReader {
            reader: self.db.snapshot(),
            storage: self.clone(),
        }
    }

    pub fn transaction<'a, 'b: 'a, T>(
        &'b self,
        f: impl Fn(StorageWriter<'a>) -> Result<T>,
    ) -> Result<T> {
        self.db.transaction(|transaction| {
            f(StorageWriter {
                buffer: Vec::new(),
                transaction,
                storage: self,
            })
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn flush(&self) -> Result<()> {
        self.db.flush(&self.default_cf)?;
        self.db.flush(&self.gpos_cf)?;
        self.db.flush(&self.gpos_cf)?;
        self.db.flush(&self.gosp_cf)?;
        self.db.flush(&self.spog_cf)?;
        self.db.flush(&self.posg_cf)?;
        self.db.flush(&self.ospg_cf)?;
        self.db.flush(&self.dspo_cf)?;
        self.db.flush(&self.dpos_cf)?;
        self.db.flush(&self.dosp_cf)?;
        self.db.flush(&self.id2str_cf)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn compact(&self) -> Result<()> {
        self.db.compact(&self.default_cf)?;
        self.db.compact(&self.gpos_cf)?;
        self.db.compact(&self.gpos_cf)?;
        self.db.compact(&self.gosp_cf)?;
        self.db.compact(&self.spog_cf)?;
        self.db.compact(&self.posg_cf)?;
        self.db.compact(&self.ospg_cf)?;
        self.db.compact(&self.dspo_cf)?;
        self.db.compact(&self.dpos_cf)?;
        self.db.compact(&self.dosp_cf)?;
        self.db.compact(&self.id2str_cf)
    }
}

pub struct StorageReader {
    reader: Reader,
    storage: Storage,
}

impl StorageReader {
    pub fn len(&self) -> Result<usize> {
        Ok(self.reader.len(&self.storage.gspo_cf)? + self.reader.len(&self.storage.dspo_cf)?)
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.reader.is_empty(&self.storage.gspo_cf)?
            && self.reader.is_empty(&self.storage.dspo_cf)?)
    }

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            Ok(self.reader.contains_key(&self.storage.dspo_cf, &buffer)?)
        } else {
            write_gspo_quad(&mut buffer, quad);
            Ok(self.reader.contains_key(&self.storage.gspo_cf, &buffer)?)
        }
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> ChainedDecodingQuadIterator {
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

    pub fn quads(&self) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(self.dspo_quads(&[]), self.gspo_quads(&[]))
    }

    fn quads_in_named_graph(&self) -> DecodingQuadIterator {
        self.gspo_quads(&[])
    }

    fn quads_for_subject(&self, subject: &EncodedTerm) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dspo_quads(&encode_term(subject)),
            self.spog_quads(&encode_term(subject)),
        )
    }

    fn quads_for_subject_predicate(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dspo_quads(&encode_term_pair(subject, predicate)),
            self.spog_quads(&encode_term_pair(subject, predicate)),
        )
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dspo_quads(&encode_term_triple(subject, predicate, object)),
            self.spog_quads(&encode_term_triple(subject, predicate, object)),
        )
    }

    fn quads_for_subject_object(
        &self,
        subject: &EncodedTerm,
        object: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dosp_quads(&encode_term_pair(object, subject)),
            self.ospg_quads(&encode_term_pair(object, subject)),
        )
    }

    fn quads_for_predicate(&self, predicate: &EncodedTerm) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dpos_quads(&encode_term(predicate)),
            self.posg_quads(&encode_term(predicate)),
        )
    }

    fn quads_for_predicate_object(
        &self,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dpos_quads(&encode_term_pair(predicate, object)),
            self.posg_quads(&encode_term_pair(predicate, object)),
        )
    }

    fn quads_for_object(&self, object: &EncodedTerm) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dosp_quads(&encode_term(object)),
            self.ospg_quads(&encode_term(object)),
        )
    }

    fn quads_for_graph(&self, graph_name: &EncodedTerm) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(&Vec::default())
        } else {
            self.gspo_quads(&encode_term(graph_name))
        })
    }

    fn quads_for_subject_graph(
        &self,
        subject: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(&encode_term(subject))
        } else {
            self.gspo_quads(&encode_term_pair(graph_name, subject))
        })
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(&encode_term_pair(subject, predicate))
        } else {
            self.gspo_quads(&encode_term_triple(graph_name, subject, predicate))
        })
    }

    fn quads_for_subject_predicate_object_graph(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(&encode_term_triple(subject, predicate, object))
        } else {
            self.gspo_quads(&encode_term_quad(graph_name, subject, predicate, object))
        })
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(&encode_term_pair(object, subject))
        } else {
            self.gosp_quads(&encode_term_triple(graph_name, object, subject))
        })
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(&encode_term(predicate))
        } else {
            self.gpos_quads(&encode_term_pair(graph_name, predicate))
        })
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(&encode_term_pair(predicate, object))
        } else {
            self.gpos_quads(&encode_term_triple(graph_name, predicate, object))
        })
    }

    fn quads_for_object_graph(
        &self,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(&encode_term(object))
        } else {
            self.gosp_quads(&encode_term_pair(graph_name, object))
        })
    }

    pub fn named_graphs(&self) -> DecodingGraphIterator {
        DecodingGraphIterator {
            iter: self.reader.iter(&self.storage.graphs_cf).unwrap(), //TODO: propagate error?
        }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> Result<bool> {
        self.reader
            .contains_key(&self.storage.graphs_cf, &encode_term(graph_name))
    }

    fn spog_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.storage.spog_cf, prefix, QuadEncoding::Spog)
    }

    fn posg_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.storage.posg_cf, prefix, QuadEncoding::Posg)
    }

    fn ospg_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.storage.ospg_cf, prefix, QuadEncoding::Ospg)
    }

    fn gspo_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.storage.gspo_cf, prefix, QuadEncoding::Gspo)
    }

    fn gpos_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.storage.gpos_cf, prefix, QuadEncoding::Gpos)
    }

    fn gosp_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.storage.gosp_cf, prefix, QuadEncoding::Gosp)
    }

    fn dspo_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.storage.dspo_cf, prefix, QuadEncoding::Dspo)
    }

    fn dpos_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.storage.dpos_cf, prefix, QuadEncoding::Dpos)
    }

    fn dosp_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.storage.dosp_cf, prefix, QuadEncoding::Dosp)
    }

    fn inner_quads(
        &self,
        column_family: &ColumnFamily,
        prefix: &[u8],
        encoding: QuadEncoding,
    ) -> DecodingQuadIterator {
        DecodingQuadIterator {
            iter: self.reader.scan_prefix(column_family, prefix).unwrap(), // TODO: propagate error?
            encoding,
        }
    }

    pub fn get_str(&self, key: &StrHash) -> Result<Option<String>> {
        self.reader
            .get(&self.storage.id2str_cf, &key.to_be_bytes())?
            .map(|v| String::from_utf8(v.into()))
            .transpose()
            .map_err(invalid_data_error)
    }

    pub fn contains_str(&self, key: &StrHash) -> Result<bool> {
        self.reader
            .contains_key(&self.storage.id2str_cf, &key.to_be_bytes())
    }
}

pub struct ChainedDecodingQuadIterator {
    first: DecodingQuadIterator,
    second: Option<DecodingQuadIterator>,
}

impl ChainedDecodingQuadIterator {
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

impl Iterator for ChainedDecodingQuadIterator {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        if let Some(result) = self.first.next() {
            Some(result)
        } else if let Some(second) = self.second.as_mut() {
            second.next()
        } else {
            None
        }
    }
}

pub struct DecodingQuadIterator {
    iter: Iter,
    encoding: QuadEncoding,
}

impl Iterator for DecodingQuadIterator {
    type Item = Result<EncodedQuad>;

    fn next(&mut self) -> Option<Result<EncodedQuad>> {
        if let Err(e) = self.iter.status() {
            return Some(Err(e));
        }
        let term = self.encoding.decode(self.iter.key()?);
        self.iter.next();
        Some(term)
    }
}

pub struct DecodingGraphIterator {
    iter: Iter,
}

impl Iterator for DecodingGraphIterator {
    type Item = Result<EncodedTerm>;

    fn next(&mut self) -> Option<Result<EncodedTerm>> {
        if let Err(e) = self.iter.status() {
            return Some(Err(e));
        }
        let term = decode_term(self.iter.key()?);
        self.iter.next();
        Some(term)
    }
}

impl StrLookup for StorageReader {
    type Error = std::io::Error;

    fn get_str(&self, key: &StrHash) -> Result<Option<String>> {
        self.get_str(key)
    }

    fn contains_str(&self, key: &StrHash) -> Result<bool> {
        self.contains_str(key)
    }
}

pub struct StorageWriter<'a> {
    buffer: Vec<u8>,
    transaction: Transaction<'a>,
    storage: &'a Storage,
}

impl<'a> StorageWriter<'a> {
    pub fn reader(&self) -> StorageReader {
        StorageReader {
            reader: self.transaction.reader(),
            storage: self.storage.clone(),
        }
    }

    pub fn insert(&mut self, quad: QuadRef<'_>) -> Result<bool> {
        let encoded = quad.into();
        self.buffer.clear();
        let result = if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, &encoded);
            if self
                .transaction
                .contains_key_for_update(&self.storage.dspo_cf, &self.buffer)?
            {
                false
            } else {
                self.transaction
                    .insert_empty(&self.storage.dspo_cf, &self.buffer)?;

                self.buffer.clear();
                write_pos_quad(&mut self.buffer, &encoded);
                self.transaction
                    .insert_empty(&self.storage.dpos_cf, &self.buffer)?;

                self.buffer.clear();
                write_osp_quad(&mut self.buffer, &encoded);
                self.transaction
                    .insert_empty(&self.storage.dosp_cf, &self.buffer)?;

                self.insert_term(quad.subject.into(), &encoded.subject)?;
                self.insert_term(quad.predicate.into(), &encoded.predicate)?;
                self.insert_term(quad.object, &encoded.object)?;
                true
            }
        } else {
            write_spog_quad(&mut self.buffer, &encoded);
            if self
                .transaction
                .contains_key_for_update(&self.storage.spog_cf, &self.buffer)?
            {
                false
            } else {
                self.transaction
                    .insert_empty(&self.storage.spog_cf, &self.buffer)?;

                self.buffer.clear();
                write_posg_quad(&mut self.buffer, &encoded);
                self.transaction
                    .insert_empty(&self.storage.posg_cf, &self.buffer)?;

                self.buffer.clear();
                write_ospg_quad(&mut self.buffer, &encoded);
                self.transaction
                    .insert_empty(&self.storage.ospg_cf, &self.buffer)?;

                self.buffer.clear();
                write_gspo_quad(&mut self.buffer, &encoded);
                self.transaction
                    .insert_empty(&self.storage.gspo_cf, &self.buffer)?;

                self.buffer.clear();
                write_gpos_quad(&mut self.buffer, &encoded);
                self.transaction
                    .insert_empty(&self.storage.gpos_cf, &self.buffer)?;

                self.buffer.clear();
                write_gosp_quad(&mut self.buffer, &encoded);
                self.transaction
                    .insert_empty(&self.storage.gosp_cf, &self.buffer)?;

                self.insert_term(quad.subject.into(), &encoded.subject)?;
                self.insert_term(quad.predicate.into(), &encoded.predicate)?;
                self.insert_term(quad.object, &encoded.object)?;

                self.buffer.clear();
                write_term(&mut self.buffer, &encoded.graph_name);
                if !self
                    .transaction
                    .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
                {
                    self.transaction
                        .insert_empty(&self.storage.graphs_cf, &self.buffer)?;
                    self.insert_graph_name(quad.graph_name, &encoded.graph_name)?;
                }
                true
            }
        };
        Ok(result)
    }

    pub fn insert_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) -> Result<bool> {
        let encoded_graph_name = graph_name.into();

        self.buffer.clear();
        write_term(&mut self.buffer, &encoded_graph_name);
        let result = if self
            .transaction
            .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
        {
            false
        } else {
            self.transaction
                .insert_empty(&self.storage.graphs_cf, &self.buffer)?;
            self.insert_term(graph_name.into(), &encoded_graph_name)?;
            true
        };
        Ok(result)
    }

    fn insert_term(&mut self, term: TermRef<'_>, encoded: &EncodedTerm) -> Result<()> {
        insert_term(term, encoded, &mut |key, value| self.insert_str(key, value))
    }

    fn insert_graph_name(
        &mut self,
        graph_name: GraphNameRef<'_>,
        encoded: &EncodedTerm,
    ) -> Result<()> {
        match graph_name {
            GraphNameRef::NamedNode(graph_name) => self.insert_term(graph_name.into(), encoded),
            GraphNameRef::BlankNode(graph_name) => self.insert_term(graph_name.into(), encoded),
            GraphNameRef::DefaultGraph => Ok(()),
        }
    }

    fn insert_str(&mut self, key: &StrHash, value: &str) -> Result<()> {
        self.transaction.insert(
            &self.storage.id2str_cf,
            &key.to_be_bytes(),
            value.as_bytes(),
        )
    }

    pub fn remove(&mut self, quad: QuadRef<'_>) -> Result<bool> {
        self.remove_encoded(&quad.into())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<bool> {
        self.buffer.clear();
        let result = if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, quad);

            if self
                .transaction
                .contains_key_for_update(&self.storage.dspo_cf, &self.buffer)?
            {
                self.transaction
                    .remove(&self.storage.dspo_cf, &self.buffer)?;

                self.buffer.clear();
                write_pos_quad(&mut self.buffer, quad);
                self.transaction
                    .remove(&self.storage.dpos_cf, &self.buffer)?;

                self.buffer.clear();
                write_osp_quad(&mut self.buffer, quad);
                self.transaction
                    .remove(&self.storage.dosp_cf, &self.buffer)?;
                true
            } else {
                false
            }
        } else {
            write_spog_quad(&mut self.buffer, quad);

            if self
                .transaction
                .contains_key_for_update(&self.storage.spog_cf, &self.buffer)?
            {
                self.transaction
                    .remove(&self.storage.spog_cf, &self.buffer)?;

                self.buffer.clear();
                write_posg_quad(&mut self.buffer, quad);
                self.transaction
                    .remove(&self.storage.posg_cf, &self.buffer)?;

                self.buffer.clear();
                write_ospg_quad(&mut self.buffer, quad);
                self.transaction
                    .remove(&self.storage.ospg_cf, &self.buffer)?;

                self.buffer.clear();
                write_gspo_quad(&mut self.buffer, quad);
                self.transaction
                    .remove(&self.storage.gspo_cf, &self.buffer)?;

                self.buffer.clear();
                write_gpos_quad(&mut self.buffer, quad);
                self.transaction
                    .remove(&self.storage.gpos_cf, &self.buffer)?;

                self.buffer.clear();
                write_gosp_quad(&mut self.buffer, quad);
                self.transaction
                    .remove(&self.storage.gosp_cf, &self.buffer)?;
                true
            } else {
                false
            }
        };
        Ok(result)
    }

    pub fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<()> {
        if graph_name.is_default_graph() {
            for quad in self.reader().quads_for_graph(&EncodedTerm::DefaultGraph) {
                self.remove_encoded(&quad?)?;
            }
        } else {
            self.buffer.clear();
            write_term(&mut self.buffer, &graph_name.into());
            if self
                .transaction
                .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
            {
                // The condition is useful to lock the graph itself and ensure no quad is inserted at the same time
                for quad in self.reader().quads_for_graph(&graph_name.into()) {
                    self.remove_encoded(&quad?)?;
                }
            }
        }
        Ok(())
    }

    pub fn clear_all_named_graphs(&mut self) -> Result<()> {
        for quad in self.reader().quads_in_named_graph() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    pub fn clear_all_graphs(&mut self) -> Result<()> {
        for quad in self.reader().quads() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    pub fn remove_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) -> Result<bool> {
        self.remove_encoded_named_graph(&graph_name.into())
    }

    fn remove_encoded_named_graph(&mut self, graph_name: &EncodedTerm) -> Result<bool> {
        self.buffer.clear();
        write_term(&mut self.buffer, graph_name);
        let result = if self
            .transaction
            .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
        {
            // The condition is done ASAP to lock the graph itself
            for quad in self.reader().quads_for_graph(graph_name) {
                self.remove_encoded(&quad?)?;
            }
            self.buffer.clear();
            write_term(&mut self.buffer, graph_name);
            self.transaction
                .remove(&self.storage.graphs_cf, &self.buffer)?;
            true
        } else {
            false
        };
        Ok(result)
    }

    pub fn remove_all_named_graphs(&mut self) -> Result<()> {
        for graph_name in self.reader().named_graphs() {
            self.remove_encoded_named_graph(&graph_name?)?;
        }
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        for graph_name in self.reader().named_graphs() {
            self.remove_encoded_named_graph(&graph_name?)?;
        }
        for quad in self.reader().quads() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }
}

/// Creates a database from a dataset files.
#[cfg(not(target_arch = "wasm32"))]
pub fn bulk_load(storage: &Storage, quads: impl IntoIterator<Item = Result<Quad>>) -> Result<()> {
    let mut threads = Vec::new();
    let mut buffer = Vec::with_capacity(BULK_LOAD_BATCH_SIZE);
    for quad in quads {
        let quad = quad?;
        buffer.push(quad);
        if buffer.len() >= BULK_LOAD_BATCH_SIZE {
            let buffer = take(&mut buffer);
            let storage = storage.clone();
            threads.push(spawn(move || BulkLoader::new(storage).load(buffer)));
        }
    }
    BulkLoader::new(storage.clone()).load(buffer)?; // Last buffer
    for thread in threads {
        thread.join().unwrap()?;
    }
    Ok(())
}

/// Creates a database from a dataset files.
#[cfg(not(target_arch = "wasm32"))]
struct BulkLoader {
    storage: Storage,
    id2str: HashMap<StrHash, Box<str>>,
    quads: HashSet<EncodedQuad>,
    triples: HashSet<EncodedQuad>,
    graphs: HashSet<EncodedTerm>,
    buffer: Vec<u8>,
}

#[cfg(not(target_arch = "wasm32"))]
impl BulkLoader {
    fn new(storage: Storage) -> Self {
        Self {
            storage,
            id2str: HashMap::default(),
            quads: HashSet::default(),
            triples: HashSet::default(),
            graphs: HashSet::default(),
            buffer: Vec::new(),
        }
    }

    fn load(&mut self, quads: impl IntoIterator<Item = Quad>) -> Result<()> {
        for quad in quads {
            let encoded = EncodedQuad::from(quad.as_ref());
            self.buffer.clear();
            if quad.graph_name.is_default_graph() {
                write_spo_quad(&mut self.buffer, &encoded);
                if self.triples.insert(encoded.clone()) {
                    self.insert_term(quad.subject.as_ref().into(), &encoded.subject)?;
                    self.insert_term(quad.predicate.as_ref().into(), &encoded.predicate)?;
                    self.insert_term(quad.object.as_ref(), &encoded.object)?;
                }
            } else {
                write_spog_quad(&mut self.buffer, &encoded);
                if self.quads.insert(encoded.clone()) {
                    self.insert_term(quad.subject.as_ref().into(), &encoded.subject)?;
                    self.insert_term(quad.predicate.as_ref().into(), &encoded.predicate)?;
                    self.insert_term(quad.object.as_ref(), &encoded.object)?;

                    self.buffer.clear();
                    write_term(&mut self.buffer, &encoded.graph_name);
                    if self.graphs.insert(encoded.graph_name.clone()) {
                        self.insert_term(
                            match quad.graph_name.as_ref() {
                                GraphNameRef::NamedNode(n) => n.into(),
                                GraphNameRef::BlankNode(n) => n.into(),
                                GraphNameRef::DefaultGraph => unreachable!(),
                            },
                            &encoded.graph_name,
                        )?;
                    }
                }
            }
        }
        self.save()
    }

    fn save(&mut self) -> Result<()> {
        let mut to_load = Vec::new();

        // id2str
        if !self.id2str.is_empty() {
            let mut id2str = take(&mut self.id2str)
                .into_iter()
                .map(|(k, v)| (k.to_be_bytes(), v))
                .collect::<Vec<_>>();
            id2str.sort_unstable();
            let mut id2str_sst = self.storage.db.new_sst_file()?;
            for (k, v) in id2str {
                id2str_sst.insert(&k, v.as_bytes())?;
            }
            to_load.push((&self.storage.id2str_cf, id2str_sst.finish()?));
        }

        if !self.triples.is_empty() {
            to_load.push((
                &self.storage.dspo_cf,
                self.build_sst_for_keys(
                    self.triples.iter().map(|quad| {
                        encode_term_triple(&quad.subject, &quad.predicate, &quad.object)
                    }),
                )?,
            ));
            to_load.push((
                &self.storage.dpos_cf,
                self.build_sst_for_keys(
                    self.triples.iter().map(|quad| {
                        encode_term_triple(&quad.predicate, &quad.object, &quad.subject)
                    }),
                )?,
            ));
            to_load.push((
                &self.storage.dosp_cf,
                self.build_sst_for_keys(
                    self.triples.iter().map(|quad| {
                        encode_term_triple(&quad.object, &quad.subject, &quad.predicate)
                    }),
                )?,
            ));
            self.triples.clear();
        }

        if !self.quads.is_empty() {
            let quads = take(&mut self.graphs);
            to_load.push((
                &self.storage.graphs_cf,
                self.build_sst_for_keys(quads.into_iter().map(|g| encode_term(&g)))?,
            ));

            to_load.push((
                &self.storage.gspo_cf,
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.graph_name,
                        &quad.subject,
                        &quad.predicate,
                        &quad.object,
                    )
                }))?,
            ));
            to_load.push((
                &self.storage.gpos_cf,
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.graph_name,
                        &quad.object,
                        &quad.subject,
                        &quad.predicate,
                    )
                }))?,
            ));
            to_load.push((
                &self.storage.gosp_cf,
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.graph_name,
                        &quad.object,
                        &quad.subject,
                        &quad.predicate,
                    )
                }))?,
            ));
            to_load.push((
                &self.storage.spog_cf,
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.subject,
                        &quad.predicate,
                        &quad.object,
                        &quad.graph_name,
                    )
                }))?,
            ));
            to_load.push((
                &self.storage.posg_cf,
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.object,
                        &quad.subject,
                        &quad.predicate,
                        &quad.graph_name,
                    )
                }))?,
            ));
            to_load.push((
                &self.storage.ospg_cf,
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.object,
                        &quad.subject,
                        &quad.predicate,
                        &quad.graph_name,
                    )
                }))?,
            ));
            self.quads.clear();
        }

        self.storage.db.insert_stt_files(&to_load)
    }

    fn insert_term(&mut self, term: TermRef<'_>, encoded: &EncodedTerm) -> Result<()> {
        insert_term(term, encoded, &mut |key, value| {
            self.id2str.entry(*key).or_insert_with(|| value.into());
            Ok(())
        })
    }

    fn build_sst_for_keys(&self, values: impl Iterator<Item = Vec<u8>>) -> Result<PathBuf> {
        let mut values = values.collect::<Vec<_>>();
        values.sort_unstable();
        let mut sst = self.storage.db.new_sst_file()?;
        for t in values {
            sst.insert_empty(&t)?;
        }
        sst.finish()
    }
}
