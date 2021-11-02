use crate::error::invalid_data_error;
use crate::model::{GraphNameRef, NamedOrBlankNodeRef, QuadRef};
use crate::storage::binary_encoder::{
    decode_term, encode_term, encode_term_pair, encode_term_quad, encode_term_triple,
    write_gosp_quad, write_gpos_quad, write_gspo_quad, write_osp_quad, write_ospg_quad,
    write_pos_quad, write_posg_quad, write_spo_quad, write_spog_quad, write_term, QuadEncoding,
    LATEST_STORAGE_VERSION, WRITTEN_TERM_MAX_SIZE,
};
use crate::storage::numeric_encoder::{EncodedQuad, EncodedTerm, StrHash, StrLookup, TermEncoder};
#[cfg(target_arch = "wasm32")]
use fallback_backend::{
    ColumnFamily, ColumnFamilyDefinition, CompactionAction, CompactionFilter, Db, Iter,
    MergeOperator,
};
#[cfg(not(target_arch = "wasm32"))]
use rocksdb_backend::{
    ColumnFamily, ColumnFamilyDefinition, CompactionAction, CompactionFilter, Db, Iter,
    MergeOperator,
};
use std::ffi::CString;
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

mod binary_encoder;
#[cfg(target_arch = "wasm32")]
mod fallback_backend;
pub mod io;
pub mod numeric_encoder;
#[cfg(not(target_arch = "wasm32"))]
mod rocksdb_backend;
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
    pub fn new() -> std::io::Result<Self> {
        Self::setup(Db::new(Self::column_families())?)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: &Path) -> std::io::Result<Self> {
        Self::setup(Db::open(path, Self::column_families())?)
    }

    fn column_families() -> Vec<ColumnFamilyDefinition> {
        vec![
            ColumnFamilyDefinition {
                name: ID2STR_CF,
                merge_operator: Some(Self::str2id_merge()),
                compaction_filter: Some(Self::str2id_filter()),
            },
            ColumnFamilyDefinition {
                name: SPOG_CF,
                merge_operator: None,
                compaction_filter: None,
            },
            ColumnFamilyDefinition {
                name: POSG_CF,
                merge_operator: None,
                compaction_filter: None,
            },
            ColumnFamilyDefinition {
                name: OSPG_CF,
                merge_operator: None,
                compaction_filter: None,
            },
            ColumnFamilyDefinition {
                name: GSPO_CF,
                merge_operator: None,
                compaction_filter: None,
            },
            ColumnFamilyDefinition {
                name: GPOS_CF,
                merge_operator: None,
                compaction_filter: None,
            },
            ColumnFamilyDefinition {
                name: GOSP_CF,
                merge_operator: None,
                compaction_filter: None,
            },
            ColumnFamilyDefinition {
                name: DSPO_CF,
                merge_operator: None,
                compaction_filter: None,
            },
            ColumnFamilyDefinition {
                name: DPOS_CF,
                merge_operator: None,
                compaction_filter: None,
            },
            ColumnFamilyDefinition {
                name: DOSP_CF,
                merge_operator: None,
                compaction_filter: None,
            },
            ColumnFamilyDefinition {
                name: GRAPHS_CF,
                merge_operator: None,
                compaction_filter: None,
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

    fn setup(db: Db) -> std::io::Result<Self> {
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
            // We migrate to v1
            for quad in this.quads() {
                let quad = quad?;
                if !quad.graph_name.is_default_graph() {
                    this.db
                        .insert_empty(&this.graphs_cf, &encode_term(&quad.graph_name), false)?;
                }
            }
            this.db.flush(&this.graphs_cf)?;
            version = 1;
            this.set_version(version)?;
            this.db.flush(&this.default_cf)?;
        }
        if version == 1 {
            // We migrate to v2
            let mut iter = this.db.iter(&this.id2str_cf);
            while let (Some(key), Some(value)) = (iter.key(), iter.value()) {
                let mut new_value = Vec::with_capacity(value.len() + 4);
                new_value.extend_from_slice(&i32::MAX.to_be_bytes());
                new_value.extend_from_slice(value);
                this.db.insert(&this.id2str_cf, key, &new_value, false)?;
                iter.next();
            }
            iter.status()?;
            this.db.flush(&this.id2str_cf)?;
            version = 2;
            this.set_version(version)?;
            this.db.flush(&this.default_cf)?;
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

    fn ensure_version(&self) -> std::io::Result<u64> {
        Ok(
            if let Some(version) = self.db.get(&self.default_cf, b"oxversion")? {
                let mut buffer = [0; 8];
                buffer.copy_from_slice(&version);
                u64::from_be_bytes(buffer)
            } else {
                self.set_version(LATEST_STORAGE_VERSION)?;
                LATEST_STORAGE_VERSION
            },
        )
    }

    fn set_version(&self, version: u64) -> std::io::Result<()> {
        self.db.insert(
            &self.default_cf,
            b"oxversion",
            &version.to_be_bytes(),
            false,
        )?;
        Ok(())
    }

    pub fn len(&self) -> std::io::Result<usize> {
        Ok(self.db.len(&self.gspo_cf)? + self.db.len(&self.dspo_cf)?)
    }

    pub fn is_empty(&self) -> std::io::Result<bool> {
        Ok(self.db.is_empty(&self.gspo_cf)? && self.db.is_empty(&self.dspo_cf)?)
    }

    pub fn contains(&self, quad: &EncodedQuad) -> std::io::Result<bool> {
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

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> std::io::Result<bool> {
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

    pub fn insert(&self, quad: QuadRef<'_>) -> std::io::Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);
        let encoded = quad.into();

        Ok(if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, &encoded);
            if self.db.contains_key(&self.dspo_cf, buffer.as_slice())? {
                false
            } else {
                self.insert_quad_triple(quad, &encoded)?;

                self.db
                    .insert_empty(&self.dspo_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_pos_quad(&mut buffer, &encoded);
                self.db
                    .insert_empty(&self.dpos_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_osp_quad(&mut buffer, &encoded);
                self.db
                    .insert_empty(&self.dosp_cf, buffer.as_slice(), false)?;
                buffer.clear();

                true
            }
        } else {
            write_spog_quad(&mut buffer, &encoded);
            if self.db.contains_key(&self.spog_cf, buffer.as_slice())? {
                false
            } else {
                self.insert_quad_triple(quad, &encoded)?;

                self.db
                    .insert_empty(&self.spog_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_posg_quad(&mut buffer, &encoded);
                self.db
                    .insert_empty(&self.posg_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_ospg_quad(&mut buffer, &encoded);
                self.db
                    .insert_empty(&self.ospg_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_gspo_quad(&mut buffer, &encoded);
                self.db
                    .insert_empty(&self.gspo_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_gpos_quad(&mut buffer, &encoded);
                self.db
                    .insert_empty(&self.gpos_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_gosp_quad(&mut buffer, &encoded);
                self.db
                    .insert_empty(&self.gosp_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_term(&mut buffer, &encoded.graph_name);
                if !self.db.contains_key(&self.graphs_cf, &buffer)? {
                    self.db.insert_empty(&self.graphs_cf, &buffer, false)?;
                    self.insert_graph_name(quad.graph_name, &encoded.graph_name)?;
                }
                buffer.clear();

                true
            }
        })
    }

    pub fn remove(&self, quad: QuadRef<'_>) -> std::io::Result<bool> {
        self.remove_encoded(&quad.into())
    }

    fn remove_encoded(&self, quad: &EncodedQuad) -> std::io::Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        Ok(if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);

            if self.db.contains_key(&self.dspo_cf, buffer.as_slice())? {
                self.db.remove(&self.dspo_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_pos_quad(&mut buffer, quad);
                self.db.remove(&self.dpos_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_osp_quad(&mut buffer, quad);
                self.db.remove(&self.dosp_cf, buffer.as_slice(), false)?;
                buffer.clear();

                self.remove_quad_triple(quad)?;

                true
            } else {
                false
            }
        } else {
            write_spog_quad(&mut buffer, quad);

            if self.db.contains_key(&self.spog_cf, buffer.as_slice())? {
                self.db.remove(&self.spog_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_posg_quad(&mut buffer, quad);
                self.db.remove(&self.posg_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_ospg_quad(&mut buffer, quad);
                self.db.remove(&self.ospg_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_gspo_quad(&mut buffer, quad);
                self.db.remove(&self.gspo_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_gpos_quad(&mut buffer, quad);
                self.db.remove(&self.gpos_cf, buffer.as_slice(), false)?;
                buffer.clear();

                write_gosp_quad(&mut buffer, quad);
                self.db.remove(&self.gosp_cf, buffer.as_slice(), false)?;
                buffer.clear();

                self.remove_quad_triple(quad)?;

                true
            } else {
                false
            }
        })
    }

    pub fn insert_named_graph(&self, graph_name: NamedOrBlankNodeRef<'_>) -> std::io::Result<bool> {
        let encoded_graph_name = graph_name.into();
        let encoded = encode_term(&encoded_graph_name);
        Ok(if self.db.contains_key(&self.graphs_cf, &encoded)? {
            false
        } else {
            self.db.insert_empty(&self.graphs_cf, &encoded, false)?;
            self.insert_term(graph_name.into(), &encoded_graph_name)?;
            true
        })
    }

    pub fn clear_graph(&self, graph_name: GraphNameRef<'_>) -> std::io::Result<()> {
        for quad in self.quads_for_graph(&graph_name.into()) {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    pub fn clear_all_named_graphs(&self) -> std::io::Result<()> {
        for quad in self.quads_in_named_graph() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    pub fn clear_all_graphs(&self) -> std::io::Result<()> {
        for quad in self.quads() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    pub fn remove_named_graph(&self, graph_name: NamedOrBlankNodeRef<'_>) -> std::io::Result<bool> {
        self.remove_encoded_named_graph(&graph_name.into())
    }

    fn remove_encoded_named_graph(&self, graph_name: &EncodedTerm) -> std::io::Result<bool> {
        for quad in self.quads_for_graph(graph_name) {
            self.remove_encoded(&quad?)?;
        }
        let encoded_graph = encode_term(graph_name);
        Ok(if self.db.contains_key(&self.graphs_cf, &encoded_graph)? {
            self.db.remove(&self.graphs_cf, &encoded_graph, false)?;
            self.remove_term(graph_name)?;
            true
        } else {
            false
        })
    }

    pub fn remove_all_named_graphs(&self) -> std::io::Result<()> {
        for graph_name in self.named_graphs() {
            self.remove_encoded_named_graph(&graph_name?)?;
        }
        Ok(())
    }

    pub fn clear(&self) -> std::io::Result<()> {
        for graph_name in self.named_graphs() {
            self.remove_encoded_named_graph(&graph_name?)?;
        }
        for quad in self.quads() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn flush(&self) -> std::io::Result<()> {
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

    pub fn get_str(&self, key: &StrHash) -> std::io::Result<Option<String>> {
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

    pub fn contains_str(&self, key: &StrHash) -> std::io::Result<bool> {
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
    type Item = std::io::Result<EncodedQuad>;

    fn next(&mut self) -> Option<std::io::Result<EncodedQuad>> {
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
    type Item = std::io::Result<EncodedQuad>;

    fn next(&mut self) -> Option<std::io::Result<EncodedQuad>> {
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
    type Item = std::io::Result<EncodedTerm>;

    fn next(&mut self) -> Option<std::io::Result<EncodedTerm>> {
        if let Err(e) = self.iter.status() {
            return Some(Err(e));
        }
        let term = decode_term(self.iter.key()?);
        self.iter.next();
        Some(term)
    }
}

impl TermEncoder for Storage {
    type Error = std::io::Error;

    fn insert_str(&self, key: &StrHash, value: &str) -> std::io::Result<()> {
        let mut buffer = Vec::with_capacity(value.len() + 4);
        buffer.extend_from_slice(&1_i32.to_be_bytes());
        buffer.extend_from_slice(value.as_bytes());
        self.db
            .merge(&self.id2str_cf, &key.to_be_bytes(), &buffer, false)
    }

    fn remove_str(&self, key: &StrHash) -> std::io::Result<()> {
        self.db.merge(
            &self.id2str_cf,
            &key.to_be_bytes(),
            &(-1_i32).to_be_bytes(),
            true,
        )
    }
}

pub trait StorageLike: StrLookup {
    fn insert(&self, quad: QuadRef<'_>) -> Result<bool, Self::Error>;

    fn remove(&self, quad: QuadRef<'_>) -> Result<bool, Self::Error>;
}

impl StrLookup for Storage {
    type Error = std::io::Error;

    fn get_str(&self, key: &StrHash) -> std::io::Result<Option<String>> {
        self.get_str(key)
    }

    fn contains_str(&self, key: &StrHash) -> std::io::Result<bool> {
        self.contains_str(key)
    }
}

impl StorageLike for Storage {
    fn insert(&self, quad: QuadRef<'_>) -> std::io::Result<bool> {
        self.insert(quad)
    }

    fn remove(&self, quad: QuadRef<'_>) -> std::io::Result<bool> {
        self.remove(quad)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::NamedNodeRef;

    #[test]
    fn test_strings_removal() -> std::io::Result<()> {
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
        storage.insert(quad)?;
        storage.insert(quad2)?;
        storage.remove(quad2)?;
        storage.flush()?;
        storage.db.compact(&storage.id2str_cf)?;
        assert!(storage
            .get_str(&StrHash::new("http://example.com/s"))?
            .is_some());
        assert!(storage
            .get_str(&StrHash::new("http://example.com/p"))?
            .is_some());
        assert!(storage
            .get_str(&StrHash::new("http://example.com/o2"))?
            .is_none());
        storage.clear_graph(NamedNodeRef::new_unchecked("http://example.com/g").into())?;
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
        storage.remove_named_graph(NamedNodeRef::new_unchecked("http://example.com/g").into())?;
        assert!(storage
            .get_str(&StrHash::new("http://example.com/g"))?
            .is_none());
        Ok(())
    }
}
