use crate::model::{GraphNameRef, NamedOrBlankNodeRef, QuadRef, TermRef};
use crate::storage::backend::memory::{
    ColumnFamily, ColumnFamilyDefinition, Db, Iter, Reader, Transaction,
};
use crate::storage::binary_encoder::{
    decode_term, encode_term, encode_term_pair, encode_term_quad, encode_term_triple,
    write_gosp_quad, write_gpos_quad, write_gspo_quad, write_osp_quad, write_ospg_quad,
    write_pos_quad, write_posg_quad, write_spo_quad, write_spog_quad, write_term, QuadEncoding,
    WRITTEN_TERM_MAX_SIZE,
};
pub use crate::storage::error::{CorruptionError, StorageError};
use crate::storage::numeric_encoder::{insert_term, EncodedQuad, EncodedTerm, StrHash, StrLookup};
use oxrdf::Quad;
use std::error::Error;

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
const BULK_LOAD_BATCH_SIZE: u64 = 100_000;

/// Low level storage primitives
#[derive(Clone)]
pub struct MemoryStorage {
    db: Db,
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

impl MemoryStorage {
    pub fn new() -> Result<Self, StorageError> {
        Self::setup(Db::new(Self::column_families())?)
    }

    fn column_families() -> Vec<ColumnFamilyDefinition> {
        vec![
            ColumnFamilyDefinition { name: ID2STR_CF },
            ColumnFamilyDefinition { name: SPOG_CF },
            ColumnFamilyDefinition { name: POSG_CF },
            ColumnFamilyDefinition { name: OSPG_CF },
            ColumnFamilyDefinition { name: GSPO_CF },
            ColumnFamilyDefinition { name: GPOS_CF },
            ColumnFamilyDefinition { name: GOSP_CF },
            ColumnFamilyDefinition { name: DSPO_CF },
            ColumnFamilyDefinition { name: DPOS_CF },
            ColumnFamilyDefinition { name: DOSP_CF },
            ColumnFamilyDefinition { name: GRAPHS_CF },
        ]
    }

    fn setup(db: Db) -> Result<Self, StorageError> {
        let this = Self {
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
        Ok(this)
    }

    pub fn snapshot(&self) -> MemoryStorageReader {
        MemoryStorageReader {
            reader: self.db.snapshot(),
            storage: self.clone(),
        }
    }

    pub fn transaction<'a, 'b: 'a, T, E: Error + 'static + From<StorageError>>(
        &'b self,
        f: impl Fn(MemoryStorageWriter<'a>) -> Result<T, E>,
    ) -> Result<T, E> {
        self.db.transaction(|transaction| {
            f(MemoryStorageWriter {
                buffer: Vec::new(),
                transaction,
                storage: self,
            })
        })
    }

    pub fn bulk_loader(&self) -> MemoryStorageBulkLoader {
        MemoryStorageBulkLoader {
            storage: self.clone(),
            hooks: Vec::new(),
        }
    }
}

pub struct MemoryStorageReader {
    reader: Reader,
    storage: MemoryStorage,
}

impl MemoryStorageReader {
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

    pub fn quads(&self) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(self.dspo_quads(&[]), self.gspo_quads(&[]))
    }

    fn quads_in_named_graph(&self) -> MemoryDecodingQuadIterator {
        self.gspo_quads(&[])
    }

    pub fn quads_for_subject(&self, subject: &EncodedTerm) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dspo_quads(&encode_term(subject)),
            self.spog_quads(&encode_term(subject)),
        )
    }

    pub fn quads_for_subject_predicate(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dspo_quads(&encode_term_pair(subject, predicate)),
            self.spog_quads(&encode_term_pair(subject, predicate)),
        )
    }

    pub fn quads_for_subject_predicate_object(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dspo_quads(&encode_term_triple(subject, predicate, object)),
            self.spog_quads(&encode_term_triple(subject, predicate, object)),
        )
    }

    pub fn quads_for_subject_object(
        &self,
        subject: &EncodedTerm,
        object: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dosp_quads(&encode_term_pair(object, subject)),
            self.ospg_quads(&encode_term_pair(object, subject)),
        )
    }

    pub fn quads_for_predicate(
        &self,
        predicate: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dpos_quads(&encode_term(predicate)),
            self.posg_quads(&encode_term(predicate)),
        )
    }

    pub fn quads_for_predicate_object(
        &self,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dpos_quads(&encode_term_pair(predicate, object)),
            self.posg_quads(&encode_term_pair(predicate, object)),
        )
    }

    pub fn quads_for_object(&self, object: &EncodedTerm) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::pair(
            self.dosp_quads(&encode_term(object)),
            self.ospg_quads(&encode_term(object)),
        )
    }

    pub fn quads_for_graph(&self, graph_name: &EncodedTerm) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(&Vec::default())
        } else {
            self.gspo_quads(&encode_term(graph_name))
        })
    }

    pub fn quads_for_subject_graph(
        &self,
        subject: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(&encode_term(subject))
        } else {
            self.gspo_quads(&encode_term_pair(graph_name, subject))
        })
    }

    pub fn quads_for_subject_predicate_graph(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(&encode_term_pair(subject, predicate))
        } else {
            self.gspo_quads(&encode_term_triple(graph_name, subject, predicate))
        })
    }

    pub fn quads_for_subject_predicate_object_graph(
        &self,
        subject: &EncodedTerm,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(&encode_term_triple(subject, predicate, object))
        } else {
            self.gspo_quads(&encode_term_quad(graph_name, subject, predicate, object))
        })
    }

    pub fn quads_for_subject_object_graph(
        &self,
        subject: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(&encode_term_pair(object, subject))
        } else {
            self.gosp_quads(&encode_term_triple(graph_name, object, subject))
        })
    }

    pub fn quads_for_predicate_graph(
        &self,
        predicate: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(&encode_term(predicate))
        } else {
            self.gpos_quads(&encode_term_pair(graph_name, predicate))
        })
    }

    pub fn quads_for_predicate_object_graph(
        &self,
        predicate: &EncodedTerm,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(&encode_term_pair(predicate, object))
        } else {
            self.gpos_quads(&encode_term_triple(graph_name, predicate, object))
        })
    }

    pub fn quads_for_object_graph(
        &self,
        object: &EncodedTerm,
        graph_name: &EncodedTerm,
    ) -> MemoryChainedDecodingQuadIterator {
        MemoryChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(&encode_term(object))
        } else {
            self.gosp_quads(&encode_term_pair(graph_name, object))
        })
    }

    pub fn named_graphs(&self) -> MemoryDecodingGraphIterator {
        MemoryDecodingGraphIterator {
            iter: self.reader.iter(&self.storage.graphs_cf).unwrap(), // TODO: propagate error?
        }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> Result<bool, StorageError> {
        self.reader
            .contains_key(&self.storage.graphs_cf, &encode_term(graph_name))
    }

    fn spog_quads(&self, prefix: &[u8]) -> MemoryDecodingQuadIterator {
        self.inner_quads(&self.storage.spog_cf, prefix, QuadEncoding::Spog)
    }

    fn posg_quads(&self, prefix: &[u8]) -> MemoryDecodingQuadIterator {
        self.inner_quads(&self.storage.posg_cf, prefix, QuadEncoding::Posg)
    }

    fn ospg_quads(&self, prefix: &[u8]) -> MemoryDecodingQuadIterator {
        self.inner_quads(&self.storage.ospg_cf, prefix, QuadEncoding::Ospg)
    }

    fn gspo_quads(&self, prefix: &[u8]) -> MemoryDecodingQuadIterator {
        self.inner_quads(&self.storage.gspo_cf, prefix, QuadEncoding::Gspo)
    }

    fn gpos_quads(&self, prefix: &[u8]) -> MemoryDecodingQuadIterator {
        self.inner_quads(&self.storage.gpos_cf, prefix, QuadEncoding::Gpos)
    }

    fn gosp_quads(&self, prefix: &[u8]) -> MemoryDecodingQuadIterator {
        self.inner_quads(&self.storage.gosp_cf, prefix, QuadEncoding::Gosp)
    }

    fn dspo_quads(&self, prefix: &[u8]) -> MemoryDecodingQuadIterator {
        self.inner_quads(&self.storage.dspo_cf, prefix, QuadEncoding::Dspo)
    }

    fn dpos_quads(&self, prefix: &[u8]) -> MemoryDecodingQuadIterator {
        self.inner_quads(&self.storage.dpos_cf, prefix, QuadEncoding::Dpos)
    }

    fn dosp_quads(&self, prefix: &[u8]) -> MemoryDecodingQuadIterator {
        self.inner_quads(&self.storage.dosp_cf, prefix, QuadEncoding::Dosp)
    }

    fn inner_quads(
        &self,
        column_family: &ColumnFamily,
        prefix: &[u8],
        encoding: QuadEncoding,
    ) -> MemoryDecodingQuadIterator {
        MemoryDecodingQuadIterator {
            iter: self.reader.scan_prefix(column_family, prefix).unwrap(), // TODO: propagate error?
            encoding,
        }
    }

    pub fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
        self.reader
            .contains_key(&self.storage.id2str_cf, &key.to_be_bytes())
    }

    /// Validates that all the storage invariants held in the data
    #[allow(clippy::unused_self, clippy::unnecessary_wraps)]
    pub fn validate(&self) -> Result<(), StorageError> {
        Ok(()) // TODO
    }
}

pub struct MemoryChainedDecodingQuadIterator {
    first: MemoryDecodingQuadIterator,
    second: Option<MemoryDecodingQuadIterator>,
}

impl MemoryChainedDecodingQuadIterator {
    fn new(first: MemoryDecodingQuadIterator) -> Self {
        Self {
            first,
            second: None,
        }
    }

    fn pair(first: MemoryDecodingQuadIterator, second: MemoryDecodingQuadIterator) -> Self {
        Self {
            first,
            second: Some(second),
        }
    }
}

impl Iterator for MemoryChainedDecodingQuadIterator {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(result) = self.first.next() {
            Some(result)
        } else if let Some(second) = self.second.as_mut() {
            second.next()
        } else {
            None
        }
    }
}

struct MemoryDecodingQuadIterator {
    iter: Iter,
    encoding: QuadEncoding,
}

impl Iterator for MemoryDecodingQuadIterator {
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

pub struct MemoryDecodingGraphIterator {
    iter: Iter,
}

impl Iterator for MemoryDecodingGraphIterator {
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

impl StrLookup for MemoryStorageReader {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(self
            .reader
            .get(&self.storage.id2str_cf, &key.to_be_bytes())?
            .map(String::from_utf8)
            .transpose()
            .map_err(CorruptionError::new)?)
    }
}

pub struct MemoryStorageWriter<'a> {
    buffer: Vec<u8>,
    transaction: Transaction<'a>,
    storage: &'a MemoryStorage,
}

impl<'a> MemoryStorageWriter<'a> {
    pub fn reader(&self) -> MemoryStorageReader {
        MemoryStorageReader {
            reader: self.transaction.reader(),
            storage: self.storage.clone(),
        }
    }

    pub fn insert(&mut self, quad: QuadRef<'_>) -> Result<bool, StorageError> {
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

    pub fn insert_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, StorageError> {
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

    fn insert_term(
        &mut self,
        term: TermRef<'_>,
        encoded: &EncodedTerm,
    ) -> Result<(), StorageError> {
        insert_term(term, encoded, &mut |key, value| self.insert_str(key, value))
    }

    fn insert_graph_name(
        &mut self,
        graph_name: GraphNameRef<'_>,
        encoded: &EncodedTerm,
    ) -> Result<(), StorageError> {
        match graph_name {
            GraphNameRef::NamedNode(graph_name) => self.insert_term(graph_name.into(), encoded),
            GraphNameRef::BlankNode(graph_name) => self.insert_term(graph_name.into(), encoded),
            GraphNameRef::DefaultGraph => Ok(()),
        }
    }

    fn insert_str(&mut self, key: &StrHash, value: &str) -> Result<(), StorageError> {
        self.transaction.insert(
            &self.storage.id2str_cf,
            &key.to_be_bytes(),
            value.as_bytes(),
        )
    }

    pub fn remove(&mut self, quad: QuadRef<'_>) -> Result<bool, StorageError> {
        self.remove_encoded(&quad.into())
    }

    fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<bool, StorageError> {
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

    pub fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<(), StorageError> {
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

    pub fn clear_all_named_graphs(&mut self) -> Result<(), StorageError> {
        for quad in self.reader().quads_in_named_graph() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    pub fn clear_all_graphs(&mut self) -> Result<(), StorageError> {
        for quad in self.reader().quads() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }

    pub fn remove_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, StorageError> {
        self.remove_encoded_named_graph(&graph_name.into())
    }

    fn remove_encoded_named_graph(
        &mut self,
        graph_name: &EncodedTerm,
    ) -> Result<bool, StorageError> {
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

    pub fn remove_all_named_graphs(&mut self) -> Result<(), StorageError> {
        for graph_name in self.reader().named_graphs() {
            self.remove_encoded_named_graph(&graph_name?)?;
        }
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), StorageError> {
        for graph_name in self.reader().named_graphs() {
            self.remove_encoded_named_graph(&graph_name?)?;
        }
        for quad in self.reader().quads() {
            self.remove_encoded(&quad?)?;
        }
        Ok(())
    }
}

#[must_use]
pub struct MemoryStorageBulkLoader {
    storage: MemoryStorage,
    hooks: Vec<Box<dyn Fn(u64)>>,
}

impl MemoryStorageBulkLoader {
    pub fn on_progress(mut self, callback: impl Fn(u64) + 'static) -> Self {
        self.hooks.push(Box::new(callback));
        self
    }

    pub fn load<EI, EO: From<StorageError> + From<EI>>(
        &self,
        quads: impl IntoIterator<Item = Result<Quad, EI>>,
    ) -> Result<(), EO> {
        // TODO: very naïve
        let mut done_counter = 0;
        for quad in quads {
            let quad = quad?;
            self.storage
                .transaction(|mut writer| writer.insert(quad.as_ref()))?;
            done_counter += 1;
            if done_counter % BULK_LOAD_BATCH_SIZE == 0 {
                for hook in &self.hooks {
                    hook(done_counter);
                }
            }
        }
        Ok(())
    }
}
