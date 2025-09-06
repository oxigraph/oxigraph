#[cfg(feature = "rdf-12")]
use crate::model::vocab::rdf;
#[cfg(feature = "rdf-12")]
use crate::model::{BlankNode, GraphName, Term, Triple};
use crate::model::{GraphNameRef, NamedOrBlankNodeRef, Quad, QuadRef, TermRef};
use crate::storage::binary_encoder::{
    QuadEncoding, TYPE_STAR_TRIPLE, WRITTEN_TERM_MAX_SIZE, decode_term, encode_term,
    encode_term_pair, encode_term_quad, encode_term_triple, write_gosp_quad, write_gpos_quad,
    write_gspo_quad, write_osp_quad, write_ospg_quad, write_pos_quad, write_posg_quad,
    write_spo_quad, write_spog_quad, write_term,
};
pub use crate::storage::error::{CorruptionError, StorageError};
use crate::storage::numeric_encoder::{
    Decoder, EncodedQuad, EncodedTerm, StrHash, StrHashHasher, StrLookup, insert_term,
};
use crate::storage::rocksdb_wrapper::{
    ColumnFamily, ColumnFamilyDefinition, Db, Iter, ReadableTransaction, Reader, Transaction,
};
use crate::storage::{DEFAULT_BULK_LOAD_BATCH_SIZE, map_thread_result};
use rustc_hash::{FxBuildHasher, FxHashSet};
#[cfg(feature = "rdf-12")]
use siphasher::sip128::{Hasher128, SipHasher24};
use std::collections::{HashMap, VecDeque};
use std::fs::remove_file;
use std::hash::BuildHasherDefault;
#[cfg(feature = "rdf-12")]
use std::hash::Hash;
use std::mem::take;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::{io, thread};

const BATCH_SIZE: usize = 100_000;
const LATEST_STORAGE_VERSION: u64 = 2;
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
pub struct RocksDbStorage {
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

impl RocksDbStorage {
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        Self::setup(Db::open_read_write(path, Self::column_families())?)
    }

    pub fn open_read_only(path: &Path) -> Result<Self, StorageError> {
        Self::setup(Db::open_read_only(path, Self::column_families())?)
    }

    fn column_families() -> Vec<ColumnFamilyDefinition> {
        vec![
            ColumnFamilyDefinition {
                name: ID2STR_CF,
                use_iter: false,
                min_prefix_size: 0,
                unordered_writes: true,
            },
            ColumnFamilyDefinition {
                name: SPOG_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
                unordered_writes: false,
            },
            ColumnFamilyDefinition {
                name: POSG_CF,
                use_iter: true,
                min_prefix_size: 17, // named node start
                unordered_writes: false,
            },
            ColumnFamilyDefinition {
                name: OSPG_CF,
                use_iter: true,
                min_prefix_size: 0, // There are small literals...
                unordered_writes: false,
            },
            ColumnFamilyDefinition {
                name: GSPO_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
                unordered_writes: false,
            },
            ColumnFamilyDefinition {
                name: GPOS_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
                unordered_writes: false,
            },
            ColumnFamilyDefinition {
                name: GOSP_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
                unordered_writes: false,
            },
            ColumnFamilyDefinition {
                name: DSPO_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
                unordered_writes: false,
            },
            ColumnFamilyDefinition {
                name: DPOS_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
                unordered_writes: false,
            },
            ColumnFamilyDefinition {
                name: DOSP_CF,
                use_iter: true,
                min_prefix_size: 0, // There are small literals...
                unordered_writes: false,
            },
            ColumnFamilyDefinition {
                name: GRAPHS_CF,
                use_iter: true,
                min_prefix_size: 17, // named or blank node start
                unordered_writes: false,
            },
        ]
    }

    fn setup(db: Db) -> Result<Self, StorageError> {
        let this = Self {
            default_cf: db.column_family(DEFAULT_CF)?,
            id2str_cf: db.column_family(ID2STR_CF)?,
            spog_cf: db.column_family(SPOG_CF)?,
            posg_cf: db.column_family(POSG_CF)?,
            ospg_cf: db.column_family(OSPG_CF)?,
            gspo_cf: db.column_family(GSPO_CF)?,
            gpos_cf: db.column_family(GPOS_CF)?,
            gosp_cf: db.column_family(GOSP_CF)?,
            dspo_cf: db.column_family(DSPO_CF)?,
            dpos_cf: db.column_family(DPOS_CF)?,
            dosp_cf: db.column_family(DOSP_CF)?,
            graphs_cf: db.column_family(GRAPHS_CF)?,
            db,
        };
        this.migrate()?;
        Ok(this)
    }

    fn migrate(&self) -> Result<(), StorageError> {
        let mut version = self.ensure_version()?;
        if version == 0 {
            // We migrate to v1
            if !self.db.is_writable() {
                return Err(StorageError::Other(
                    "It is not possible to upgrade read-only Oxigraph instances to newer Oxigraph versions, please open in read-write regular mode to upgrade.".into(),
                ));
            }
            let mut graph_names = FxHashSet::default();
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
                .insert_stt_files(&[(self.graphs_cf.clone(), stt_file.finish()?)])?;
            version = 1;
            self.update_version(version)?;
        }
        if version == 1 {
            // We migrate to v2
            #[cfg(feature = "rdf-12")]
            fn to_rdf12_reified_triple(
                subject: &EncodedTerm,
                predicate: &EncodedTerm,
                object: &EncodedTerm,
                graph_name: &EncodedTerm,
                r: &RocksDbStorageReader<'_>,
                w: &mut RocksDbStorageTransaction<'_>,
            ) -> Result<EncodedTerm, StorageError> {
                let subject = if let EncodedTerm::Triple(t) = subject {
                    to_rdf12_reified_triple(&t.subject, &t.predicate, &t.object, graph_name, r, w)?
                } else {
                    subject.clone()
                };
                let object = if let EncodedTerm::Triple(t) = object {
                    to_rdf12_reified_triple(&t.subject, &t.predicate, &t.object, graph_name, r, w)?
                } else {
                    object.clone()
                };
                // We hash the triple
                let triple = Triple::new(
                    r.decode_named_or_blank_node(&subject)?,
                    r.decode_named_node(predicate)?,
                    r.decode_term(&object)?,
                );
                let mut hasher = SipHasher24::new();
                triple.hash(&mut hasher);
                let reifier = BlankNode::new_from_unique_id(hasher.finish128().as_u128());
                w.insert(QuadRef::new(
                    &reifier,
                    rdf::REIFIES,
                    &Term::from(triple),
                    &if *graph_name == EncodedTerm::DefaultGraph {
                        GraphName::DefaultGraph
                    } else {
                        r.decode_named_or_blank_node(graph_name)?.into()
                    },
                ));
                Ok(reifier.as_ref().into())
            }

            if !self.db.is_writable() {
                return Err(StorageError::Other(
                    "It is not possible to upgrade read-only Oxigraph instances to newer Oxigraph versions, please open in read-write regular mode to upgrade.".into(),
                ));
            }
            let snapshot = self.snapshot();
            #[cfg_attr(not(feature = "rdf-12"), expect(clippy::never_loop))]
            for quad in snapshot
                .dspo_quads(&[TYPE_STAR_TRIPLE])
                .chain(snapshot.spog_quads(&[TYPE_STAR_TRIPLE]))
                .chain(snapshot.dosp_quads(&[TYPE_STAR_TRIPLE]))
                .chain(snapshot.ospg_quads(&[TYPE_STAR_TRIPLE]))
            {
                #[cfg_attr(not(feature = "rdf-12"), expect(unused_variables))]
                let quad = quad?;
                #[cfg(not(feature = "rdf-12"))]
                return Err(CorruptionError::msg(
                    "You need to enable the rdf-12 Cargo feature to read a database with triple terms",
                ).into());

                #[cfg(feature = "rdf-12")]
                {
                    let mut w = self.start_transaction()?;
                    let mut new_quad = quad.clone();
                    if let EncodedTerm::Triple(t) = new_quad.subject {
                        new_quad.subject = to_rdf12_reified_triple(
                            &t.subject,
                            &t.predicate,
                            &t.object,
                            &quad.graph_name,
                            &snapshot,
                            &mut w,
                        )?;
                    }
                    if let EncodedTerm::Triple(t) = new_quad.object {
                        new_quad.object = to_rdf12_reified_triple(
                            &t.subject,
                            &t.predicate,
                            &t.object,
                            &quad.graph_name,
                            &snapshot,
                            &mut w,
                        )?;
                    }
                    w.insert(snapshot.decode_quad(&new_quad)?.as_ref());
                    w.remove_encoded(&quad);
                    w.commit()?;
                }
            }
            version = 2;
            self.update_version(version)?;
        }

        match version {
            _ if version < LATEST_STORAGE_VERSION => Err(CorruptionError::msg(format!(
                "The RocksDB database is using the outdated encoding version {version}. Automated migration is not supported, please dump the store dataset using a compatible Oxigraph version and load it again using the current version"
            )).into()),
            LATEST_STORAGE_VERSION => Ok(()),
            _ => Err(CorruptionError::msg(format!(
                "The RocksDB database is using the too recent version {version}. Upgrade to the latest Oxigraph version to load this database"

            )).into())
        }
    }

    fn ensure_version(&self) -> Result<u64, StorageError> {
        Ok(
            if let Some(version) = self.db.get(&self.default_cf, b"oxversion")? {
                u64::from_be_bytes(version.as_ref().try_into().map_err(|e| {
                    CorruptionError::new(format!("Error while parsing the version key: {e}"))
                })?)
            } else {
                self.update_version(LATEST_STORAGE_VERSION)?;
                LATEST_STORAGE_VERSION
            },
        )
    }

    fn update_version(&self, version: u64) -> Result<(), StorageError> {
        self.db
            .insert(&self.default_cf, b"oxversion", &version.to_be_bytes())?;
        self.db.flush()
    }

    pub fn snapshot(&self) -> RocksDbStorageReader<'static> {
        RocksDbStorageReader {
            reader: self.db.snapshot(),
            storage: self.clone(),
        }
    }

    pub fn start_transaction(&self) -> Result<RocksDbStorageTransaction<'_>, StorageError> {
        Ok(RocksDbStorageTransaction {
            buffer: Vec::new(),
            transaction: self.db.start_transaction()?,
            storage: self,
        })
    }

    pub fn start_readable_transaction(
        &self,
    ) -> Result<RocksDbStorageReadableTransaction<'_>, StorageError> {
        Ok(RocksDbStorageReadableTransaction {
            buffer: Vec::new(),
            transaction: self.db.start_readable_transaction()?,
            storage: self,
        })
    }

    pub fn flush(&self) -> Result<(), StorageError> {
        self.db.flush()
    }

    pub fn compact(&self) -> Result<(), StorageError> {
        self.db.compact(&self.default_cf)?;
        self.db.compact(&self.gspo_cf)?;
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

    pub fn backup(&self, target_directory: &Path) -> Result<(), StorageError> {
        self.db.backup(target_directory)
    }

    pub fn bulk_loader(&self) -> RocksDbStorageBulkLoader<'_> {
        RocksDbStorageBulkLoader {
            storage: self,
            hooks: Vec::new(),
            threads: VecDeque::new(),
            sst_files: Vec::new(),
            done_counter: Arc::new(Mutex::new(0)),
            done_and_displayed_counter: 0,
        }
    }
}

#[must_use]
pub struct RocksDbStorageReader<'a> {
    reader: Reader<'a>,
    storage: RocksDbStorage,
}

impl<'a> RocksDbStorageReader<'a> {
    pub fn len(&self) -> Result<usize, StorageError> {
        Ok(self.reader.len(&self.storage.gspo_cf)? + self.reader.len(&self.storage.dspo_cf)?)
    }

    pub fn is_empty(&self) -> Result<bool, StorageError> {
        Ok(self.reader.is_empty(&self.storage.gspo_cf)?
            && self.reader.is_empty(&self.storage.dspo_cf)?)
    }

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool, StorageError> {
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
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
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

    pub fn quads(&self) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::pair(self.dspo_quads(&[]), self.gspo_quads(&[]))
    }

    fn quads_for_subject(&self, subject: &EncodedTerm) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::pair(
            self.dspo_quads(&encode_term(subject)),
            self.spog_quads(&encode_term(subject)),
        )
    }

    fn quads_for_subject_predicate(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::pair(
            self.dspo_quads(&encode_term_pair(subject, predicate)),
            self.spog_quads(&encode_term_pair(subject, predicate)),
        )
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::pair(
            self.dspo_quads(&encode_term_triple(subject, predicate, object)),
            self.spog_quads(&encode_term_triple(subject, predicate, object)),
        )
    }

    fn quads_for_subject_object(
        &self,
        subject: &EncodedTerm,
        object: &EncodedTerm,
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::pair(
            self.dosp_quads(&encode_term_pair(object, subject)),
            self.ospg_quads(&encode_term_pair(object, subject)),
        )
    }

    fn quads_for_predicate(
        &self,
        predicate: &EncodedTerm,
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::pair(
            self.dpos_quads(&encode_term(predicate)),
            self.posg_quads(&encode_term(predicate)),
        )
    }

    fn quads_for_predicate_object(
        &self,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::pair(
            self.dpos_quads(&encode_term_pair(predicate, object)),
            self.posg_quads(&encode_term_pair(predicate, object)),
        )
    }

    fn quads_for_object(&self, object: &EncodedTerm) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::pair(
            self.dosp_quads(&encode_term(object)),
            self.ospg_quads(&encode_term(object)),
        )
    }

    fn quads_for_graph(&self, graph_name: &EncodedTerm) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(&Vec::default())
        } else {
            self.gspo_quads(&encode_term(graph_name))
        })
    }

    fn quads_for_subject_graph(
        &self,
        subject: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
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
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
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
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
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
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(&encode_term_pair(object, subject))
        } else {
            self.gosp_quads(&encode_term_triple(graph_name, object, subject))
        })
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
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
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(&encode_term_pair(predicate, object))
        } else {
            self.gpos_quads(&encode_term_triple(graph_name, predicate, object))
        })
    }

    fn quads_for_object_graph(
        &self,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> RocksDbChainedDecodingQuadIterator<'a> {
        RocksDbChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(&encode_term(object))
        } else {
            self.gosp_quads(&encode_term_pair(graph_name, object))
        })
    }

    pub fn named_graphs(&self) -> RocksDbDecodingGraphIterator<'a> {
        RocksDbDecodingGraphIterator {
            iter: self.reader.iter(&self.storage.graphs_cf),
        }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> Result<bool, StorageError> {
        self.reader
            .contains_key(&self.storage.graphs_cf, &encode_term(graph_name))
    }

    fn spog_quads(&self, prefix: &[u8]) -> RocksDbDecodingQuadIterator<'a> {
        self.inner_quads(&self.storage.spog_cf, prefix, QuadEncoding::Spog)
    }

    fn posg_quads(&self, prefix: &[u8]) -> RocksDbDecodingQuadIterator<'a> {
        self.inner_quads(&self.storage.posg_cf, prefix, QuadEncoding::Posg)
    }

    fn ospg_quads(&self, prefix: &[u8]) -> RocksDbDecodingQuadIterator<'a> {
        self.inner_quads(&self.storage.ospg_cf, prefix, QuadEncoding::Ospg)
    }

    fn gspo_quads(&self, prefix: &[u8]) -> RocksDbDecodingQuadIterator<'a> {
        self.inner_quads(&self.storage.gspo_cf, prefix, QuadEncoding::Gspo)
    }

    fn gpos_quads(&self, prefix: &[u8]) -> RocksDbDecodingQuadIterator<'a> {
        self.inner_quads(&self.storage.gpos_cf, prefix, QuadEncoding::Gpos)
    }

    fn gosp_quads(&self, prefix: &[u8]) -> RocksDbDecodingQuadIterator<'a> {
        self.inner_quads(&self.storage.gosp_cf, prefix, QuadEncoding::Gosp)
    }

    fn dspo_quads(&self, prefix: &[u8]) -> RocksDbDecodingQuadIterator<'a> {
        self.inner_quads(&self.storage.dspo_cf, prefix, QuadEncoding::Dspo)
    }

    fn dpos_quads(&self, prefix: &[u8]) -> RocksDbDecodingQuadIterator<'a> {
        self.inner_quads(&self.storage.dpos_cf, prefix, QuadEncoding::Dpos)
    }

    fn dosp_quads(&self, prefix: &[u8]) -> RocksDbDecodingQuadIterator<'a> {
        self.inner_quads(&self.storage.dosp_cf, prefix, QuadEncoding::Dosp)
    }

    fn inner_quads(
        &self,
        column_family: &ColumnFamily,
        prefix: &[u8],
        encoding: QuadEncoding,
    ) -> RocksDbDecodingQuadIterator<'a> {
        RocksDbDecodingQuadIterator {
            iter: self.reader.scan_prefix(column_family, prefix),
            encoding,
        }
    }

    pub fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
        self.storage
            .db
            .contains_key(&self.storage.id2str_cf, &key.to_be_bytes())
    }

    /// Validate that all the storage invariants held in the data
    pub fn validate(&self) -> Result<(), StorageError> {
        // triples
        let dspo_size = self.dspo_quads(&[]).count();
        if dspo_size != self.dpos_quads(&[]).count() || dspo_size != self.dosp_quads(&[]).count() {
            return Err(CorruptionError::new(
                "Not the same number of triples in dspo, dpos and dosp",
            )
            .into());
        }
        for spo in self.dspo_quads(&[]) {
            let spo = spo?;
            self.decode_quad(&spo)?; // We ensure that the quad is readable
            if !self.storage.db.contains_key(
                &self.storage.dpos_cf,
                &encode_term_triple(&spo.predicate, &spo.object, &spo.subject),
            )? {
                return Err(CorruptionError::new("Quad in dspo and not in dpos").into());
            }
            if !self.storage.db.contains_key(
                &self.storage.dosp_cf,
                &encode_term_triple(&spo.object, &spo.subject, &spo.predicate),
            )? {
                return Err(CorruptionError::new("Quad in dspo and not in dosp").into());
            }
        }

        // quads
        let gspo_size = self.gspo_quads(&[]).count();
        if gspo_size != self.gpos_quads(&[]).count()
            || gspo_size != self.gosp_quads(&[]).count()
            || gspo_size != self.spog_quads(&[]).count()
            || gspo_size != self.posg_quads(&[]).count()
            || gspo_size != self.ospg_quads(&[]).count()
        {
            return Err(CorruptionError::new(
                "Not the same number of triples in dspo, dpos and dosp",
            )
            .into());
        }
        for gspo in self.gspo_quads(&[]) {
            let gspo = gspo?;
            self.decode_quad(&gspo)?; // We ensure that the quad is readable
            if !self.storage.db.contains_key(
                &self.storage.gpos_cf,
                &encode_term_quad(
                    &gspo.graph_name,
                    &gspo.predicate,
                    &gspo.object,
                    &gspo.subject,
                ),
            )? {
                return Err(CorruptionError::new("Quad in gspo and not in gpos").into());
            }
            if !self.storage.db.contains_key(
                &self.storage.gosp_cf,
                &encode_term_quad(
                    &gspo.graph_name,
                    &gspo.object,
                    &gspo.subject,
                    &gspo.predicate,
                ),
            )? {
                return Err(CorruptionError::new("Quad in gspo and not in gosp").into());
            }
            if !self.storage.db.contains_key(
                &self.storage.spog_cf,
                &encode_term_quad(
                    &gspo.subject,
                    &gspo.predicate,
                    &gspo.object,
                    &gspo.graph_name,
                ),
            )? {
                return Err(CorruptionError::new("Quad in gspo and not in spog").into());
            }
            if !self.storage.db.contains_key(
                &self.storage.posg_cf,
                &encode_term_quad(
                    &gspo.predicate,
                    &gspo.object,
                    &gspo.subject,
                    &gspo.graph_name,
                ),
            )? {
                return Err(CorruptionError::new("Quad in gspo and not in posg").into());
            }
            if !self.storage.db.contains_key(
                &self.storage.ospg_cf,
                &encode_term_quad(
                    &gspo.object,
                    &gspo.subject,
                    &gspo.predicate,
                    &gspo.graph_name,
                ),
            )? {
                return Err(CorruptionError::new("Quad in gspo and not in ospg").into());
            }
            if !self
                .storage
                .db
                .contains_key(&self.storage.graphs_cf, &encode_term(&gspo.graph_name))?
            {
                return Err(
                    CorruptionError::new("Quad graph name in gspo and not in graphs").into(),
                );
            }
        }
        Ok(())
    }
}

#[must_use]
pub struct RocksDbChainedDecodingQuadIterator<'a> {
    first: RocksDbDecodingQuadIterator<'a>,
    second: Option<RocksDbDecodingQuadIterator<'a>>,
}

impl<'a> RocksDbChainedDecodingQuadIterator<'a> {
    fn new(first: RocksDbDecodingQuadIterator<'a>) -> Self {
        Self {
            first,
            second: None,
        }
    }

    fn pair(
        first: RocksDbDecodingQuadIterator<'a>,
        second: RocksDbDecodingQuadIterator<'a>,
    ) -> Self {
        Self {
            first,
            second: Some(second),
        }
    }
}

impl Iterator for RocksDbChainedDecodingQuadIterator<'_> {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(result) = self.first.next() {
            Some(result)
        } else if let Some(second) = &mut self.second {
            second.next()
        } else {
            None
        }
    }
}

struct RocksDbDecodingQuadIterator<'a> {
    iter: Iter<'a>,
    encoding: QuadEncoding,
}

impl Iterator for RocksDbDecodingQuadIterator<'_> {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Err(e) = self.iter.status() {
            return Some(Err(e));
        }
        let term = self.encoding.decode(self.iter.key()?);
        self.iter.next();
        Some(term)
    }
}

#[must_use]
pub struct RocksDbDecodingGraphIterator<'a> {
    iter: Iter<'a>,
}

impl Iterator for RocksDbDecodingGraphIterator<'_> {
    type Item = Result<EncodedTerm, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Err(e) = self.iter.status() {
            return Some(Err(e));
        }
        let term = decode_term(self.iter.key()?);
        self.iter.next();
        Some(term)
    }
}

impl StrLookup for RocksDbStorageReader<'_> {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(self
            .storage
            .db
            .get(&self.storage.id2str_cf, &key.to_be_bytes())?
            .map(|v| String::from_utf8(v.into()))
            .transpose()
            .map_err(CorruptionError::new)?)
    }
}

#[must_use]
pub struct RocksDbStorageTransaction<'a> {
    buffer: Vec<u8>,
    transaction: Transaction,
    storage: &'a RocksDbStorage,
}

impl RocksDbStorageTransaction<'_> {
    pub fn insert(&mut self, quad: QuadRef<'_>) {
        let encoded = quad.into();
        self.buffer.clear();
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.dspo_cf, &self.buffer);

            self.buffer.clear();
            write_pos_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.dpos_cf, &self.buffer);

            self.buffer.clear();
            write_osp_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.dosp_cf, &self.buffer);

            self.insert_term(quad.subject.into(), &encoded.subject);
            self.insert_term(quad.predicate.into(), &encoded.predicate);
            self.insert_term(quad.object, &encoded.object);
        } else {
            write_spog_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.spog_cf, &self.buffer);

            self.buffer.clear();
            write_posg_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.posg_cf, &self.buffer);

            self.buffer.clear();
            write_ospg_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.ospg_cf, &self.buffer);

            self.buffer.clear();
            write_gspo_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.gspo_cf, &self.buffer);

            self.buffer.clear();
            write_gpos_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.gpos_cf, &self.buffer);

            self.buffer.clear();
            write_gosp_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.gosp_cf, &self.buffer);

            self.insert_term(quad.subject.into(), &encoded.subject);
            self.insert_term(quad.predicate.into(), &encoded.predicate);
            self.insert_term(quad.object, &encoded.object);

            self.buffer.clear();
            write_term(&mut self.buffer, &encoded.graph_name);
            self.transaction
                .insert_empty(&self.storage.graphs_cf, &self.buffer);
            self.insert_graph_name(quad.graph_name, &encoded.graph_name);
        }
    }

    pub fn insert_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) {
        let encoded_graph_name = graph_name.into();

        self.buffer.clear();
        write_term(&mut self.buffer, &encoded_graph_name);
        self.transaction
            .insert_empty(&self.storage.graphs_cf, &self.buffer);
        self.insert_term(graph_name.into(), &encoded_graph_name);
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
        self.transaction.insert(
            &self.storage.id2str_cf,
            &key.to_be_bytes(),
            value.as_bytes(),
        )
    }

    pub fn remove(&mut self, quad: QuadRef<'_>) {
        self.remove_encoded(&quad.into())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) {
        self.buffer.clear();
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.dspo_cf, &self.buffer);

            self.buffer.clear();
            write_pos_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.dpos_cf, &self.buffer);

            self.buffer.clear();
            write_osp_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.dosp_cf, &self.buffer);
        } else {
            write_spog_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.spog_cf, &self.buffer);

            self.buffer.clear();
            write_posg_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.posg_cf, &self.buffer);

            self.buffer.clear();
            write_ospg_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.ospg_cf, &self.buffer);

            self.buffer.clear();
            write_gspo_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.gspo_cf, &self.buffer);

            self.buffer.clear();
            write_gpos_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.gpos_cf, &self.buffer);

            self.buffer.clear();
            write_gosp_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.gosp_cf, &self.buffer);
        }
    }

    pub fn clear_default_graph(&mut self) {
        self.transaction
            .remove_range(&self.storage.dspo_cf, &[], &[u8::MAX]);
        self.transaction
            .remove_range(&self.storage.dpos_cf, &[], &[u8::MAX]);
        self.transaction
            .remove_range(&self.storage.dosp_cf, &[], &[u8::MAX]);
    }

    pub fn clear_all_named_graphs(&mut self) {
        self.transaction
            .remove_range(&self.storage.gspo_cf, &[], &[u8::MAX]);
        self.transaction
            .remove_range(&self.storage.gpos_cf, &[], &[u8::MAX]);
        self.transaction
            .remove_range(&self.storage.gosp_cf, &[], &[u8::MAX]);
        self.transaction
            .remove_range(&self.storage.spog_cf, &[], &[u8::MAX]);
        self.transaction
            .remove_range(&self.storage.posg_cf, &[], &[u8::MAX]);
        self.transaction
            .remove_range(&self.storage.ospg_cf, &[], &[u8::MAX]);
    }

    pub fn clear_all_graphs(&mut self) {
        self.clear_default_graph();
        self.remove_all_named_graphs();
    }

    pub fn remove_all_named_graphs(&mut self) {
        self.clear_all_named_graphs();
        self.transaction
            .remove_range(&self.storage.graphs_cf, &[], &[u8::MAX]);
    }

    pub fn clear(&mut self) {
        self.clear_default_graph();
        self.remove_all_named_graphs();
        // TODO: clear id2str?
    }

    pub fn commit(self) -> Result<(), StorageError> {
        self.transaction.commit()
    }
}

#[must_use]
pub struct RocksDbStorageReadableTransaction<'a> {
    buffer: Vec<u8>,
    transaction: ReadableTransaction<'a>,
    storage: &'a RocksDbStorage,
}

impl RocksDbStorageReadableTransaction<'_> {
    pub fn reader(&self) -> RocksDbStorageReader<'_> {
        RocksDbStorageReader {
            reader: self.transaction.reader(),
            storage: self.storage.clone(),
        }
    }

    pub fn insert(&mut self, quad: QuadRef<'_>) {
        let encoded = quad.into();
        self.buffer.clear();
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.dspo_cf, &self.buffer);

            self.buffer.clear();
            write_pos_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.dpos_cf, &self.buffer);

            self.buffer.clear();
            write_osp_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.dosp_cf, &self.buffer);

            self.insert_term(quad.subject.into(), &encoded.subject);
            self.insert_term(quad.predicate.into(), &encoded.predicate);
            self.insert_term(quad.object, &encoded.object)
        } else {
            write_spog_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.spog_cf, &self.buffer);

            self.buffer.clear();
            write_posg_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.posg_cf, &self.buffer);

            self.buffer.clear();
            write_ospg_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.ospg_cf, &self.buffer);

            self.buffer.clear();
            write_gspo_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.gspo_cf, &self.buffer);

            self.buffer.clear();
            write_gpos_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.gpos_cf, &self.buffer);

            self.buffer.clear();
            write_gosp_quad(&mut self.buffer, &encoded);
            self.transaction
                .insert_empty(&self.storage.gosp_cf, &self.buffer);

            self.insert_term(quad.subject.into(), &encoded.subject);
            self.insert_term(quad.predicate.into(), &encoded.predicate);
            self.insert_term(quad.object, &encoded.object);

            self.buffer.clear();
            write_term(&mut self.buffer, &encoded.graph_name);
            self.transaction
                .insert_empty(&self.storage.graphs_cf, &self.buffer);
            self.insert_graph_name(quad.graph_name, &encoded.graph_name)
        }
    }

    pub fn insert_named_graph(&mut self, graph_name: NamedOrBlankNodeRef<'_>) {
        let encoded_graph_name = graph_name.into();

        self.buffer.clear();
        write_term(&mut self.buffer, &encoded_graph_name);
        self.transaction
            .insert_empty(&self.storage.graphs_cf, &self.buffer);
        self.insert_term(graph_name.into(), &encoded_graph_name)
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
        self.transaction.insert(
            &self.storage.id2str_cf,
            &key.to_be_bytes(),
            value.as_bytes(),
        );
    }

    pub fn remove(&mut self, quad: QuadRef<'_>) {
        self.remove_encoded(&quad.into())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) {
        self.buffer.clear();
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.dspo_cf, &self.buffer);

            self.buffer.clear();
            write_pos_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.dpos_cf, &self.buffer);

            self.buffer.clear();
            write_osp_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.dosp_cf, &self.buffer);
        } else {
            write_spog_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.spog_cf, &self.buffer);

            self.buffer.clear();
            write_posg_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.posg_cf, &self.buffer);

            self.buffer.clear();
            write_ospg_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.ospg_cf, &self.buffer);

            self.buffer.clear();
            write_gspo_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.gspo_cf, &self.buffer);

            self.buffer.clear();
            write_gpos_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.gpos_cf, &self.buffer);

            self.buffer.clear();
            write_gosp_quad(&mut self.buffer, quad);
            self.transaction.remove(&self.storage.gosp_cf, &self.buffer);
        }
    }

    pub fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<(), StorageError> {
        self.clear_encoded_graph(&graph_name.into())
    }

    fn clear_encoded_graph(&mut self, graph_name: &EncodedTerm) -> Result<(), StorageError> {
        loop {
            let quads = self
                .reader()
                .quads_for_graph(graph_name)
                .take(BATCH_SIZE)
                .collect::<Result<Vec<_>, _>>()?;
            for quad in &quads {
                self.remove_encoded(quad);
            }
            if quads.len() < BATCH_SIZE {
                return Ok(());
            }
        }
    }

    pub fn clear_all_named_graphs(&mut self) -> Result<(), StorageError> {
        loop {
            let graph_names = self
                .reader()
                .named_graphs()
                .take(BATCH_SIZE)
                .collect::<Result<Vec<_>, _>>()?;
            for graph_name in &graph_names {
                self.clear_encoded_graph(graph_name)?;
            }
            if graph_names.len() < BATCH_SIZE {
                return Ok(());
            }
        }
    }

    pub fn clear_all_graphs(&mut self) -> Result<(), StorageError> {
        self.clear_all_named_graphs()?;
        self.clear_graph(GraphNameRef::DefaultGraph)
    }

    pub fn remove_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<(), StorageError> {
        self.remove_encoded_named_graph(&graph_name.into())
    }

    fn remove_encoded_named_graph(&mut self, graph_name: &EncodedTerm) -> Result<(), StorageError> {
        self.clear_encoded_graph(graph_name)?;
        self.buffer.clear();
        write_term(&mut self.buffer, graph_name);
        self.transaction
            .remove(&self.storage.graphs_cf, &self.buffer);
        Ok(())
    }

    pub fn remove_all_named_graphs(&mut self) -> Result<(), StorageError> {
        loop {
            let graph_names = self
                .reader()
                .named_graphs()
                .take(BATCH_SIZE)
                .collect::<Result<Vec<_>, _>>()?;
            for graph_name in &graph_names {
                self.remove_encoded_named_graph(graph_name)?;
            }
            if graph_names.len() < BATCH_SIZE {
                return Ok(());
            }
        }
    }

    pub fn clear(&mut self) -> Result<(), StorageError> {
        self.remove_all_named_graphs()?;
        self.clear_graph(GraphNameRef::DefaultGraph)
    }

    pub fn commit(self) -> Result<(), StorageError> {
        self.transaction.commit()
    }
}

#[must_use]
pub struct RocksDbStorageBulkLoader<'a> {
    storage: &'a RocksDbStorage,
    hooks: Vec<Box<dyn Fn(u64) + Send + Sync>>,
    threads: VecDeque<JoinHandle<Result<Vec<(ColumnFamily, PathBuf)>, StorageError>>>,
    sst_files: Vec<(ColumnFamily, PathBuf)>,
    done_counter: Arc<Mutex<u64>>,
    done_and_displayed_counter: u64,
}

impl Drop for RocksDbStorageBulkLoader<'_> {
    fn drop(&mut self) {
        // We clean the created files
        for (_, file) in &self.sst_files {
            #[expect(unused_must_use)] // We already have an error to report...
            remove_file(file);
        }
    }
}

impl RocksDbStorageBulkLoader<'_> {
    pub fn on_progress(mut self, callback: impl Fn(u64) + Send + Sync + 'static) -> Self {
        self.hooks.push(Box::new(callback));
        self
    }

    pub fn load_batch(
        &mut self,
        batch: Vec<Quad>,
        max_num_threads: usize,
    ) -> Result<(), StorageError> {
        self.on_possible_progress()?;
        while self.threads.len() >= max_num_threads {
            if let Some(thread) = self.threads.pop_front() {
                self.sst_files
                    .extend(map_thread_result(thread.join()).map_err(StorageError::Io)??);
                self.on_possible_progress()?;
            }
        }
        // TODO: better spawn
        let storage = self.storage.clone();
        let counter = Arc::clone(&self.done_counter);
        self.threads.push_back(thread::spawn(move || {
            FileBulkLoader::new(&storage, batch.len()).load(batch, &counter)
        }));
        Ok(())
    }

    fn on_possible_progress(&mut self) -> Result<(), StorageError> {
        let new_counter = *self
            .done_counter
            .lock()
            .map_err(|_| io::Error::other("Mutex poisoned"))?;
        let display_step = DEFAULT_BULK_LOAD_BATCH_SIZE as u64;
        if new_counter / display_step > self.done_and_displayed_counter / display_step {
            for hook in &self.hooks {
                hook(new_counter);
            }
        }
        self.done_and_displayed_counter = new_counter;
        Ok(())
    }

    pub fn commit(mut self) -> Result<(), StorageError> {
        while let Some(thread) = self.threads.pop_front() {
            self.sst_files
                .extend(map_thread_result(thread.join()).map_err(StorageError::Io)??);
            self.on_possible_progress()?;
        }
        self.storage.db.insert_stt_files(&self.sst_files)?;
        self.sst_files.clear(); // We clear the Vec to not remove them on Drop
        Ok(())
    }
}

struct FileBulkLoader<'a> {
    storage: &'a RocksDbStorage,
    id2str: HashMap<StrHash, Box<str>, BuildHasherDefault<StrHashHasher>>,
    quads: FxHashSet<EncodedQuad>,
    triples: FxHashSet<EncodedQuad>,
    graphs: FxHashSet<EncodedTerm>,
}

impl<'a> FileBulkLoader<'a> {
    fn new(storage: &'a RocksDbStorage, batch_size: usize) -> Self {
        Self {
            storage,
            id2str: HashMap::with_capacity_and_hasher(
                3 * batch_size,
                BuildHasherDefault::default(),
            ),
            quads: FxHashSet::with_capacity_and_hasher(batch_size, FxBuildHasher),
            triples: FxHashSet::with_capacity_and_hasher(batch_size, FxBuildHasher),
            graphs: FxHashSet::default(),
        }
    }

    fn load(
        &mut self,
        quads: Vec<Quad>,
        counter: &Mutex<u64>,
    ) -> Result<Vec<(ColumnFamily, PathBuf)>, StorageError> {
        self.encode(quads)?;
        let size = self.triples.len() + self.quads.len();
        let files = self.build_sst_files()?;
        *counter
            .lock()
            .map_err(|_| io::Error::other("Mutex poisoned"))? +=
            size.try_into().unwrap_or(u64::MAX);
        Ok(files)
    }

    fn encode(&mut self, quads: Vec<Quad>) -> Result<(), StorageError> {
        for quad in quads {
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
                            GraphNameRef::DefaultGraph => {
                                return Err(CorruptionError::new(
                                    "Default graph this not the default graph",
                                )
                                .into());
                            }
                        },
                        &encoded.graph_name,
                    );
                }
            }
        }
        Ok(())
    }

    fn build_sst_files(&mut self) -> Result<Vec<(ColumnFamily, PathBuf)>, StorageError> {
        let mut sst_files = Vec::new();

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
            sst_files.push((self.storage.id2str_cf.clone(), id2str_sst.finish()?));
        }

        if !self.triples.is_empty() {
            sst_files.push((
                self.storage.dspo_cf.clone(),
                self.build_sst_for_keys(
                    self.triples.iter().map(|quad| {
                        encode_term_triple(&quad.subject, &quad.predicate, &quad.object)
                    }),
                )?,
            ));
            sst_files.push((
                self.storage.dpos_cf.clone(),
                self.build_sst_for_keys(
                    self.triples.iter().map(|quad| {
                        encode_term_triple(&quad.predicate, &quad.object, &quad.subject)
                    }),
                )?,
            ));
            sst_files.push((
                self.storage.dosp_cf.clone(),
                self.build_sst_for_keys(
                    self.triples.iter().map(|quad| {
                        encode_term_triple(&quad.object, &quad.subject, &quad.predicate)
                    }),
                )?,
            ));
            self.triples.clear();
        }

        if !self.quads.is_empty() {
            sst_files.push((
                self.storage.graphs_cf.clone(),
                self.build_sst_for_keys(self.graphs.iter().map(encode_term))?,
            ));
            self.graphs.clear();

            sst_files.push((
                self.storage.gspo_cf.clone(),
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.graph_name,
                        &quad.subject,
                        &quad.predicate,
                        &quad.object,
                    )
                }))?,
            ));
            sst_files.push((
                self.storage.gpos_cf.clone(),
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.graph_name,
                        &quad.predicate,
                        &quad.object,
                        &quad.subject,
                    )
                }))?,
            ));
            sst_files.push((
                self.storage.gosp_cf.clone(),
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.graph_name,
                        &quad.object,
                        &quad.subject,
                        &quad.predicate,
                    )
                }))?,
            ));
            sst_files.push((
                self.storage.spog_cf.clone(),
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.subject,
                        &quad.predicate,
                        &quad.object,
                        &quad.graph_name,
                    )
                }))?,
            ));
            sst_files.push((
                self.storage.posg_cf.clone(),
                self.build_sst_for_keys(self.quads.iter().map(|quad| {
                    encode_term_quad(
                        &quad.predicate,
                        &quad.object,
                        &quad.subject,
                        &quad.graph_name,
                    )
                }))?,
            ));
            sst_files.push((
                self.storage.ospg_cf.clone(),
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
        Ok(sst_files)
    }

    fn insert_term(&mut self, term: TermRef<'_>, encoded: &EncodedTerm) {
        insert_term(term, encoded, &mut |key, value| {
            self.id2str.entry(*key).or_insert_with(|| value.into());
        })
    }

    fn build_sst_for_keys(
        &self,
        values: impl Iterator<Item = Vec<u8>>,
    ) -> Result<PathBuf, StorageError> {
        let mut values = values.collect::<Vec<_>>();
        values.sort_unstable();
        let mut sst = self.storage.db.new_sst_file()?;
        for value in values {
            sst.insert_empty(&value)?;
        }
        sst.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::NamedNodeRef;
    use tempfile::TempDir;

    #[test]
    fn test_send_sync() {
        fn is_send_sync<T: Send + Sync>() {}
        is_send_sync::<RocksDbStorage>();
        is_send_sync::<RocksDbStorageReader<'static>>();
        is_send_sync::<RocksDbStorageReadableTransaction<'_>>();
        is_send_sync::<RocksDbStorageBulkLoader<'_>>();
    }

    #[test]
    #[expect(clippy::panic_in_result_fn)]
    fn test_transaction() -> Result<(), StorageError> {
        let example = NamedNodeRef::new_unchecked("http://example.com/1");
        let example2 = NamedNodeRef::new_unchecked("http://example.com/2");
        let encoded_example = EncodedTerm::from(example);
        let encoded_example2 = EncodedTerm::from(example2);
        let default_quad = QuadRef::new(example, example, example, GraphNameRef::DefaultGraph);
        let encoded_default_quad = EncodedQuad::from(default_quad);
        let named_graph_quad = QuadRef::new(example, example, example, example);
        let encoded_named_graph_quad = EncodedQuad::from(named_graph_quad);

        let path = TempDir::new()?;
        let storage = RocksDbStorage::open(path.as_ref())?;

        // We start with a graph
        let snapshot = storage.snapshot();
        let mut transaction = storage.start_transaction()?;
        transaction.insert_named_graph(example.into());
        transaction.commit()?;
        assert!(!snapshot.contains_named_graph(&encoded_example)?);
        assert!(storage.snapshot().contains_named_graph(&encoded_example)?);
        storage.snapshot().validate()?;

        // We add two quads
        let snapshot = storage.snapshot();
        let mut transaction = storage.start_transaction()?;
        transaction.insert(default_quad);
        transaction.insert(named_graph_quad);
        transaction.commit()?;
        assert!(!snapshot.contains(&encoded_default_quad)?);
        assert!(!snapshot.contains(&encoded_named_graph_quad)?);
        assert!(storage.snapshot().contains(&encoded_default_quad)?);
        assert!(storage.snapshot().contains(&encoded_named_graph_quad)?);
        storage.snapshot().validate()?;

        // We remove the quads
        let snapshot = storage.snapshot();
        let mut transaction = storage.start_readable_transaction()?;
        transaction.remove(default_quad);
        transaction.remove_named_graph(example.into())?;
        transaction.commit()?;
        assert!(snapshot.contains(&encoded_default_quad)?);
        assert!(snapshot.contains(&encoded_named_graph_quad)?);
        assert!(snapshot.contains_named_graph(&encoded_example)?);
        assert!(!storage.snapshot().contains(&encoded_default_quad)?);
        assert!(!storage.snapshot().contains(&encoded_named_graph_quad)?);
        assert!(!storage.snapshot().contains_named_graph(&encoded_example)?);
        storage.snapshot().validate()?;

        // We add the quads again but rollback
        let snapshot = storage.snapshot();
        let mut transaction = storage.start_transaction()?;
        transaction.insert(default_quad);
        transaction.insert(named_graph_quad);
        transaction.insert_named_graph(example2.into());
        drop(transaction);
        assert!(!snapshot.contains(&encoded_default_quad)?);
        assert!(!snapshot.contains(&encoded_named_graph_quad)?);
        assert!(!snapshot.contains_named_graph(&encoded_example)?);
        assert!(!snapshot.contains_named_graph(&encoded_example2)?);
        assert!(!storage.snapshot().contains(&encoded_default_quad)?);
        assert!(!storage.snapshot().contains(&encoded_named_graph_quad)?);
        assert!(!storage.snapshot().contains_named_graph(&encoded_example)?);
        assert!(!storage.snapshot().contains_named_graph(&encoded_example2)?);
        storage.snapshot().validate()?;

        // We add quads and graph, then clear
        let mut loader = storage.bulk_loader();
        loader.load_batch(
            vec![default_quad.into_owned(), named_graph_quad.into_owned()],
            1,
        )?;
        loader.commit()?;
        let mut transaction = storage.start_transaction()?;
        transaction.insert_named_graph(example2.into());
        transaction.commit()?;
        let mut transaction = storage.start_transaction()?;
        transaction.clear();
        transaction.commit()?;
        assert!(!storage.snapshot().contains(&encoded_default_quad)?);
        assert!(!storage.snapshot().contains(&encoded_named_graph_quad)?);
        assert!(!storage.snapshot().contains_named_graph(&encoded_example)?);
        assert!(!storage.snapshot().contains_named_graph(&encoded_example2)?);
        assert!(storage.snapshot().is_empty()?);
        storage.snapshot().validate()?;

        Ok(())
    }
}
