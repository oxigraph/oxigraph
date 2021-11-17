use crate::error::invalid_data_error;
use crate::model::{GraphNameRef, NamedOrBlankNodeRef, Quad, QuadRef, TermRef};
use crate::storage::binary_encoder::{
    decode_term, encode_term, encode_term_pair, encode_term_quad, encode_term_triple,
    write_gosp_quad, write_gpos_quad, write_gspo_quad, write_osp_quad, write_ospg_quad,
    write_pos_quad, write_posg_quad, write_spo_quad, write_spog_quad, write_term, QuadEncoding,
    LATEST_STORAGE_VERSION, WRITTEN_TERM_MAX_SIZE,
};
use crate::storage::numeric_encoder::{
    insert_term, remove_term, EncodedQuad, EncodedTerm, StrHash, StrLookup,
};
use backend::{
    ColumnFamily, ColumnFamilyDefinition, CompactionAction, CompactionFilter, Db, Iter,
    MergeOperator, WriteBatchWithIndex,
};
#[cfg(not(target_arch = "wasm32"))]
use std::collections::{hash_map, HashMap, HashSet};
use std::ffi::CString;
use std::io::Result;
#[cfg(not(target_arch = "wasm32"))]
use std::mem::take;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;
use std::path::PathBuf;

mod backend;
mod binary_encoder;
pub mod io;
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
const AUTO_WRITE_BATCH_THRESHOLD: usize = 1024;

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
    pub fn open(path: &Path, for_bulk_load: bool) -> Result<Self> {
        Self::setup(Db::open(path, Self::column_families(), for_bulk_load)?)
    }

    fn column_families() -> Vec<ColumnFamilyDefinition> {
        vec![
            ColumnFamilyDefinition {
                name: ID2STR_CF,
                merge_operator: Some(Self::str2id_merge()),
                compaction_filter: Some(Self::str2id_filter()),
                use_iter: false,
                min_prefix_size: 0,
            },
            ColumnFamilyDefinition {
                name: SPOG_CF,
                merge_operator: None,
                compaction_filter: None,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: POSG_CF,
                merge_operator: None,
                compaction_filter: None,
                use_iter: true,
                min_prefix_size: 17, // named node start
            },
            ColumnFamilyDefinition {
                name: OSPG_CF,
                merge_operator: None,
                compaction_filter: None,
                use_iter: true,
                min_prefix_size: 0, // There are small literals...
            },
            ColumnFamilyDefinition {
                name: GSPO_CF,
                merge_operator: None,
                compaction_filter: None,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: GPOS_CF,
                merge_operator: None,
                compaction_filter: None,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: GOSP_CF,
                merge_operator: None,
                compaction_filter: None,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: DSPO_CF,
                merge_operator: None,
                compaction_filter: None,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: DPOS_CF,
                merge_operator: None,
                compaction_filter: None,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
            ColumnFamilyDefinition {
                name: DOSP_CF,
                merge_operator: None,
                compaction_filter: None,
                use_iter: true,
                min_prefix_size: 0, // There are small literals...
            },
            ColumnFamilyDefinition {
                name: GRAPHS_CF,
                merge_operator: None,
                compaction_filter: None,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
            },
        ]
    }

    fn str2id_merge() -> MergeOperator {
        fn merge_counted_values<'a>(values: impl Iterator<Item = &'a [u8]>) -> Vec<u8> {
            let (counter, str) =
                values.fold((0_i32, [].as_ref()), |(prev_counter, prev_str), current| {
                    let new_counter = i32::from_be_bytes(current[..4].try_into().unwrap());
                    (
                        if prev_counter == i32::MAX {
                            i32::MAX // We keep to max, no counting
                        } else {
                            prev_counter.saturating_add(new_counter)
                        },
                        if prev_str.is_empty() {
                            &current[4..]
                        } else {
                            prev_str
                        },
                    )
                });
            let mut buffer = Vec::with_capacity(str.len() + 4);
            buffer.extend_from_slice(&counter.to_be_bytes());
            buffer.extend_from_slice(str);
            buffer
        }

        MergeOperator {
            full: |_, previous, values| merge_counted_values(previous.into_iter().chain(values)),
            partial: |_, values| merge_counted_values(values),
            name: CString::new("id2str_merge").unwrap(),
        }
    }

    fn str2id_filter() -> CompactionFilter {
        CompactionFilter {
            filter: |_, value| {
                let counter = i32::from_be_bytes(value[..4].try_into().unwrap());
                if counter > 0 {
                    CompactionAction::Keep
                } else {
                    CompactionAction::Remove
                }
            },
            name: CString::new("id2str_compaction_filter").unwrap(),
        }
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

        let mut version = this.ensure_version()?;
        if version == 0 {
            let mut batch = this.db.new_batch();
            // We migrate to v1
            for quad in this.quads() {
                let quad = quad?;
                if !quad.graph_name.is_default_graph() {
                    batch.insert_empty(&this.graphs_cf, &encode_term(&quad.graph_name));
                    if batch.len() >= AUTO_WRITE_BATCH_THRESHOLD {
                        this.db.write(&mut batch)?;
                    }
                }
            }
            this.db.write(&mut batch)?;
            this.db.flush(&this.graphs_cf)?;
            version = 1;
            this.update_version(version)?;
        }
        if version == 1 {
            // We migrate to v2
            let mut batch = this.db.new_batch();
            let mut iter = this.db.iter(&this.id2str_cf);
            while let (Some(key), Some(value)) = (iter.key(), iter.value()) {
                let mut new_value = Vec::with_capacity(value.len() + 4);
                new_value.extend_from_slice(&i32::MAX.to_be_bytes());
                new_value.extend_from_slice(value);
                batch.insert(&this.id2str_cf, key, &new_value);
                iter.next();
                if batch.len() >= AUTO_WRITE_BATCH_THRESHOLD {
                    this.db.write(&mut batch)?;
                    batch.clear();
                }
            }
            this.db.write(&mut batch)?;
            iter.status()?;
            this.db.flush(&this.id2str_cf)?;
            version = 2;
            this.update_version(version)?;
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

    fn ensure_version(&self) -> Result<u64> {
        Ok(
            if let Some(version) = self.db.get(&self.default_cf, b"oxversion")? {
                let mut buffer = [0; 8];
                buffer.copy_from_slice(&version);
                u64::from_be_bytes(buffer)
            } else {
                self.update_version(LATEST_STORAGE_VERSION)?;
                LATEST_STORAGE_VERSION
            },
        )
    }

    fn update_version(&self, version: u64) -> Result<()> {
        let mut batch = self.db.new_batch();
        batch.insert(&self.default_cf, b"oxversion", &version.to_be_bytes());
        self.db.write(&mut batch)?;
        self.db.flush(&self.default_cf)
    }

    pub fn len(&self) -> Result<usize> {
        Ok(self.db.len(&self.gspo_cf)? + self.db.len(&self.dspo_cf)?)
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.db.is_empty(&self.gspo_cf)? && self.db.is_empty(&self.dspo_cf)?)
    }

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            Ok(self.db.contains_key(&self.dspo_cf, &buffer)?)
        } else {
            write_gspo_quad(&mut buffer, quad);
            Ok(self.db.contains_key(&self.gspo_cf, &buffer)?)
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
            iter: self.db.iter(&self.graphs_cf),
        }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> Result<bool> {
        self.db
            .contains_key(&self.graphs_cf, &encode_term(graph_name))
    }

    fn spog_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.spog_cf, prefix, QuadEncoding::Spog)
    }

    fn posg_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.posg_cf, prefix, QuadEncoding::Posg)
    }

    fn ospg_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.ospg_cf, prefix, QuadEncoding::Ospg)
    }

    fn gspo_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.gspo_cf, prefix, QuadEncoding::Gspo)
    }

    fn gpos_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.gpos_cf, prefix, QuadEncoding::Gpos)
    }

    fn gosp_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.gosp_cf, prefix, QuadEncoding::Gosp)
    }

    fn dspo_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.dspo_cf, prefix, QuadEncoding::Dspo)
    }

    fn dpos_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.dpos_cf, prefix, QuadEncoding::Dpos)
    }

    fn dosp_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        self.inner_quads(&self.dosp_cf, prefix, QuadEncoding::Dosp)
    }

    fn inner_quads(
        &self,
        column_family: &ColumnFamily,
        prefix: &[u8],
        encoding: QuadEncoding,
    ) -> DecodingQuadIterator {
        DecodingQuadIterator {
            iter: self.db.scan_prefix(column_family, prefix),
            encoding,
        }
    }

    pub fn atomic_writer(&self) -> StorageWriter {
        StorageWriter {
            buffer: Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE),
            batch: self.db.new_batch(),
            storage: self.clone(),
            auto_commit: false,
        }
    }

    pub fn simple_writer(&self) -> StorageWriter {
        StorageWriter {
            buffer: Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE),
            batch: self.db.new_batch(),
            storage: self.clone(),
            auto_commit: true,
        }
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

    pub fn get_str(&self, key: &StrHash) -> Result<Option<String>> {
        self.db
            .get(&self.id2str_cf, &key.to_be_bytes())?
            .and_then(|v| {
                let count = i32::from_be_bytes(v[..4].try_into().unwrap());
                if count > 0 {
                    Some(String::from_utf8(v[4..].to_vec()))
                } else {
                    None
                }
            })
            .transpose()
            .map_err(invalid_data_error)
    }

    pub fn contains_str(&self, key: &StrHash) -> Result<bool> {
        Ok(self
            .db
            .get(&self.id2str_cf, &key.to_be_bytes())?
            .map_or(false, |v| {
                i32::from_be_bytes(v[..4].try_into().unwrap()) > 0
            }))
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

impl StrLookup for Storage {
    type Error = std::io::Error;

    fn get_str(&self, key: &StrHash) -> Result<Option<String>> {
        self.get_str(key)
    }

    fn contains_str(&self, key: &StrHash) -> Result<bool> {
        self.contains_str(key)
    }
}

pub struct StorageWriter {
    buffer: Vec<u8>,
    batch: WriteBatchWithIndex,
    storage: Storage,
    auto_commit: bool,
}

impl StorageWriter {
    pub fn insert(&mut self, quad: QuadRef<'_>) -> Result<bool> {
        let encoded = quad.into();
        self.buffer.clear();
        let result = if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, &encoded);
            if self
                .batch
                .contains_key(&self.storage.dspo_cf, &self.buffer)?
            {
                false
            } else {
                self.batch.insert_empty(&self.storage.dspo_cf, &self.buffer);

                self.buffer.clear();
                write_pos_quad(&mut self.buffer, &encoded);
                self.batch.insert_empty(&self.storage.dpos_cf, &self.buffer);

                self.buffer.clear();
                write_osp_quad(&mut self.buffer, &encoded);
                self.batch.insert_empty(&self.storage.dosp_cf, &self.buffer);

                self.insert_term(quad.subject.into(), &encoded.subject);
                self.insert_term(quad.predicate.into(), &encoded.predicate);
                self.insert_term(quad.object, &encoded.object);
                true
            }
        } else {
            write_spog_quad(&mut self.buffer, &encoded);
            if self
                .batch
                .contains_key(&self.storage.spog_cf, &self.buffer)?
            {
                false
            } else {
                self.batch.insert_empty(&self.storage.spog_cf, &self.buffer);

                self.buffer.clear();
                write_posg_quad(&mut self.buffer, &encoded);
                self.batch.insert_empty(&self.storage.posg_cf, &self.buffer);

                self.buffer.clear();
                write_ospg_quad(&mut self.buffer, &encoded);
                self.batch.insert_empty(&self.storage.ospg_cf, &self.buffer);

                self.buffer.clear();
                write_gspo_quad(&mut self.buffer, &encoded);
                self.batch.insert_empty(&self.storage.gspo_cf, &self.buffer);

                self.buffer.clear();
                write_gpos_quad(&mut self.buffer, &encoded);
                self.batch.insert_empty(&self.storage.gpos_cf, &self.buffer);

                self.buffer.clear();
                write_gosp_quad(&mut self.buffer, &encoded);
                self.batch.insert_empty(&self.storage.gosp_cf, &self.buffer);

                self.insert_term(quad.subject.into(), &encoded.subject);
                self.insert_term(quad.predicate.into(), &encoded.predicate);
                self.insert_term(quad.object, &encoded.object);

                self.buffer.clear();
                write_term(&mut self.buffer, &encoded.graph_name);
                if !self
                    .batch
                    .contains_key(&self.storage.graphs_cf, &self.buffer)?
                {
                    self.batch
                        .insert_empty(&self.storage.graphs_cf, &self.buffer);
                    self.insert_graph_name(quad.graph_name, &encoded.graph_name);
                }
                true
            }
        };
        self.write_if_needed()?;
        Ok(result)
    }

    pub fn insert_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) -> Result<bool> {
        let encoded_graph_name = graph_name.into();

        self.buffer.clear();
        write_term(&mut self.buffer, &encoded_graph_name);
        let result = if self
            .batch
            .contains_key(&self.storage.graphs_cf, &self.buffer)?
        {
            false
        } else {
            self.batch
                .insert_empty(&self.storage.graphs_cf, &self.buffer);
            self.insert_term(graph_name.into(), &encoded_graph_name);
            true
        };
        self.write_if_needed()?;
        Ok(result)
    }

    fn insert_term(&mut self, term: TermRef<'_>, encoded: &EncodedTerm) {
        insert_term(term, encoded, &mut |key, value| self.insert_str(key, value))
    }

    fn insert_graph_name(&mut self, graph_name: GraphNameRef<'_>, encoded: &EncodedTerm) {
        match graph_name {
            GraphNameRef::NamedNode(graph_name) => self.insert_term(graph_name.into(), encoded),
            GraphNameRef::BlankNode(graph_name) => self.insert_term(graph_name.into(), encoded),
            GraphNameRef::DefaultGraph => (),
        }
    }

    fn insert_str(&mut self, key: &StrHash, value: &str) {
        self.buffer.clear();
        self.buffer.extend_from_slice(&1_i32.to_be_bytes());
        self.buffer.extend_from_slice(value.as_bytes());
        self.batch
            .merge(&self.storage.id2str_cf, &key.to_be_bytes(), &self.buffer);
    }

    pub fn remove(&mut self, quad: QuadRef<'_>) -> Result<bool> {
        self.remove_encoded(&quad.into())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<bool> {
        self.buffer.clear();
        let result = if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, quad);

            if self
                .batch
                .contains_key(&self.storage.dspo_cf, &self.buffer)?
            {
                self.batch.remove(&self.storage.dspo_cf, &self.buffer);

                self.buffer.clear();
                write_pos_quad(&mut self.buffer, quad);
                self.batch.remove(&self.storage.dpos_cf, &self.buffer);

                self.buffer.clear();
                write_osp_quad(&mut self.buffer, quad);
                self.batch.remove(&self.storage.dosp_cf, &self.buffer);

                self.remove_term(&quad.subject);
                self.remove_term(&quad.predicate);
                self.remove_term(&quad.object);
                true
            } else {
                false
            }
        } else {
            write_spog_quad(&mut self.buffer, quad);

            if self
                .batch
                .contains_key(&self.storage.spog_cf, &self.buffer)?
            {
                self.batch.remove(&self.storage.spog_cf, &self.buffer);

                self.buffer.clear();
                write_posg_quad(&mut self.buffer, quad);
                self.batch.remove(&self.storage.posg_cf, &self.buffer);

                self.buffer.clear();
                write_ospg_quad(&mut self.buffer, quad);
                self.batch.remove(&self.storage.ospg_cf, &self.buffer);

                self.buffer.clear();
                write_gspo_quad(&mut self.buffer, quad);
                self.batch.remove(&self.storage.gspo_cf, &self.buffer);

                self.buffer.clear();
                write_gpos_quad(&mut self.buffer, quad);
                self.batch.remove(&self.storage.gpos_cf, &self.buffer);

                self.buffer.clear();
                write_gosp_quad(&mut self.buffer, quad);
                self.batch.remove(&self.storage.gosp_cf, &self.buffer);

                self.remove_term(&quad.subject);
                self.remove_term(&quad.predicate);
                self.remove_term(&quad.object);
                true
            } else {
                false
            }
        };
        self.write_if_needed()?;
        Ok(result)
    }

    pub fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<()> {
        for quad in self.storage.quads_for_graph(&graph_name.into()) {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    pub fn clear_all_named_graphs(&mut self) -> Result<()> {
        for quad in self.storage.quads_in_named_graph() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    pub fn clear_all_graphs(&mut self) -> Result<()> {
        for quad in self.storage.quads() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    pub fn remove_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) -> Result<bool> {
        self.remove_encoded_named_graph(&graph_name.into())
    }

    fn remove_encoded_named_graph(&mut self, graph_name: &EncodedTerm) -> Result<bool> {
        for quad in self.storage.quads_for_graph(graph_name) {
            self.remove_encoded(&quad?)?;
        }
        self.buffer.clear();
        write_term(&mut self.buffer, graph_name);
        let result = if self
            .batch
            .contains_key(&self.storage.graphs_cf, &self.buffer)?
        {
            self.batch.remove(&self.storage.graphs_cf, &self.buffer);
            self.remove_term(graph_name);
            true
        } else {
            false
        };
        self.write_if_needed()?;
        Ok(result)
    }

    pub fn remove_all_named_graphs(&mut self) -> Result<()> {
        for graph_name in self.storage.named_graphs() {
            self.remove_encoded_named_graph(&graph_name?)?;
        }
        Ok(())
    }

    pub fn clear(&mut self) -> Result<()> {
        for graph_name in self.storage.named_graphs() {
            self.remove_encoded_named_graph(&graph_name?)?;
        }
        for quad in self.storage.quads() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    fn remove_term(&mut self, encoded: &EncodedTerm) {
        remove_term(encoded, &mut |key| self.remove_str(key));
    }

    fn remove_str(&mut self, key: &StrHash) {
        self.batch.merge(
            &self.storage.id2str_cf,
            &key.to_be_bytes(),
            &(-1_i32).to_be_bytes(),
        )
    }

    fn write_if_needed(&mut self) -> Result<()> {
        if self.auto_commit && self.batch.len() > AUTO_WRITE_BATCH_THRESHOLD {
            self.commit()?;
        }
        Ok(())
    }

    pub fn commit(&mut self) -> Result<()> {
        self.storage.db.write(&mut self.batch)?;
        Ok(())
    }
}

/// Creates a database from a dataset files.
#[cfg(not(target_arch = "wasm32"))]
pub struct BulkLoader {
    storage: Storage,
    id2str: HashMap<StrHash, (i32, Box<str>)>,
    quads: HashSet<EncodedQuad>,
    triples: HashSet<EncodedQuad>,
    graphs: HashSet<EncodedTerm>,
}

#[cfg(not(target_arch = "wasm32"))]
impl BulkLoader {
    pub fn new(path: &Path) -> Result<Self> {
        Ok(Self {
            storage: Storage::open(path, true)?, //TODO: remove bulk option
            id2str: HashMap::default(),
            quads: HashSet::default(),
            triples: HashSet::default(),
            graphs: HashSet::default(),
        })
    }

    pub fn load(&mut self, quads: impl IntoIterator<Item = Result<Quad>>) -> Result<()> {
        let mut count = 0;
        for quad in quads {
            let quad = quad?;
            let encoded = EncodedQuad::from(quad.as_ref());
            if quad.graph_name.is_default_graph() {
                if self.triples.insert(encoded.clone()) {
                    self.insert_term(quad.subject.as_ref().into(), &encoded.subject);
                    self.insert_term(quad.predicate.as_ref().into(), &encoded.predicate);
                    self.insert_term(quad.object.as_ref(), &encoded.object);
                }
            } else if self.quads.insert(encoded.clone()) {
                self.insert_term(quad.subject.as_ref().into(), &encoded.subject);
                self.insert_term(quad.predicate.as_ref().into(), &encoded.predicate);
                self.insert_term(quad.object.as_ref(), &encoded.object);
                if self.graphs.insert(encoded.graph_name.clone()) {
                    self.insert_term(
                        match quad.graph_name.as_ref() {
                            GraphNameRef::NamedNode(n) => n.into(),
                            GraphNameRef::BlankNode(n) => n.into(),
                            GraphNameRef::DefaultGraph => unreachable!(),
                        },
                        &encoded.graph_name,
                    );
                }
            }
            count += 1;
            if count % (1024 * 1024) == 0 {
                self.save()?;
            }
        }
        self.save()?;
        self.storage.compact()
    }

    fn save(&mut self) -> Result<()> {
        let mut to_load = Vec::new();

        // id2str
        if !self.id2str.is_empty() {
            let mut id2str = take(&mut self.id2str)
                .into_iter()
                .map(|(k, v)| (k.to_be_bytes(), v))
                .collect::<Vec<_>>();
            id2str.sort();
            let mut id2str_sst = self.storage.db.new_sst_file()?;
            let mut buffer = Vec::new();
            for (k, (count, v)) in id2str {
                buffer.extend_from_slice(&count.to_be_bytes());
                buffer.extend_from_slice(v.as_bytes());
                id2str_sst.merge(&k, &buffer)?;
                buffer.clear();
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

        self.storage.db.write_stt_files(to_load)
    }

    fn insert_term(&mut self, term: TermRef<'_>, encoded: &EncodedTerm) {
        insert_term(
            term,
            encoded,
            &mut |key, value| match self.id2str.entry(*key) {
                hash_map::Entry::Occupied(mut e) => {
                    let e = e.get_mut();
                    e.0 = e.0.wrapping_add(1);
                }
                hash_map::Entry::Vacant(e) => {
                    e.insert((1, value.into()));
                }
            },
        )
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NamedNodeRef;

    #[test]
    fn test_strings_removal() -> Result<()> {
        let quad = QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            NamedNodeRef::new_unchecked("http://example.com/o"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        );
        let quad2 = QuadRef::new(
            NamedNodeRef::new_unchecked("http://example.com/s"),
            NamedNodeRef::new_unchecked("http://example.com/p"),
            NamedNodeRef::new_unchecked("http://example.com/o2"),
            NamedNodeRef::new_unchecked("http://example.com/g"),
        );

        let storage = Storage::new()?;
        let mut writer = storage.atomic_writer();
        writer.insert(quad)?;
        writer.insert(quad2)?;
        writer.remove(quad2)?;
        writer.commit()?;
        assert!(storage
            .get_str(&StrHash::new("http://example.com/s"))?
            .is_some());
        assert!(storage
            .get_str(&StrHash::new("http://example.com/p"))?
            .is_some());
        assert!(storage
            .get_str(&StrHash::new("http://example.com/o2"))?
            .is_none());
        writer.clear_graph(NamedNodeRef::new_unchecked("http://example.com/g").into())?;
        writer.commit()?;
        assert!(storage
            .get_str(&StrHash::new("http://example.com/s"))?
            .is_none());
        assert!(storage
            .get_str(&StrHash::new("http://example.com/p"))?
            .is_none());
        assert!(storage
            .get_str(&StrHash::new("http://example.com/o"))?
            .is_none());
        assert!(storage
            .get_str(&StrHash::new("http://example.com/g"))?
            .is_some());
        writer.remove_named_graph(NamedNodeRef::new_unchecked("http://example.com/g").into())?;
        writer.commit()?;
        assert!(storage
            .get_str(&StrHash::new("http://example.com/g"))?
            .is_none());
        Ok(())
    }
}
