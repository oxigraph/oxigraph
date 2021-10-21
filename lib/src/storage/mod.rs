use std::error::Error;
use std::fmt;
use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
use sled::transaction::{
    ConflictableTransactionError as Sled2ConflictableTransactionError,
    TransactionError as Sled2TransactionError, TransactionalTree,
    UnabortableTransactionError as Sled2UnabortableTransactionError,
};

use crate::error::invalid_data_error;
use crate::model::{GraphNameRef, NamedOrBlankNodeRef, QuadRef};
use crate::sparql::EvaluationError;
use crate::storage::binary_encoder::{
    decode_term, encode_term, encode_term_pair, encode_term_quad, encode_term_triple,
    write_gosp_quad, write_gpos_quad, write_gspo_quad, write_osp_quad, write_ospg_quad,
    write_pos_quad, write_posg_quad, write_spo_quad, write_spog_quad, write_term, QuadEncoding,
    LATEST_STORAGE_VERSION, WRITTEN_TERM_MAX_SIZE,
};
use crate::storage::io::StoreOrParseError;
use crate::storage::numeric_encoder::{EncodedQuad, EncodedTerm, StrHash, StrLookup, TermEncoder};
#[cfg(target_arch = "wasm32")]
use fallback_backend::{Db, Iter, Tree};
#[cfg(not(target_arch = "wasm32"))]
use sled_backend::{Db, Iter, Tree};

mod binary_encoder;
#[cfg(target_arch = "wasm32")]
mod fallback_backend;
pub mod io;
pub mod numeric_encoder;
#[cfg(not(target_arch = "wasm32"))]
mod sled_backend;
pub mod small_string;

/// Low level storage primitives
#[derive(Clone)]
pub struct Storage {
    default: Db,
    id2str: Tree,
    spog: Tree,
    posg: Tree,
    ospg: Tree,
    gspo: Tree,
    gpos: Tree,
    gosp: Tree,
    dspo: Tree,
    dpos: Tree,
    dosp: Tree,
    graphs: Tree,
}

impl Storage {
    pub fn new() -> std::io::Result<Self> {
        Self::setup(Db::new()?)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: &Path) -> std::io::Result<Self> {
        Self::setup(Db::open(path)?)
    }

    fn setup(db: Db) -> std::io::Result<Self> {
        let mut id2str = db.open_tree("id2str")?;
        id2str.set_merge_operator(id2str_merge);

        let this = Self {
            id2str,
            spog: db.open_tree("spog")?,
            posg: db.open_tree("posg")?,
            ospg: db.open_tree("ospg")?,
            gspo: db.open_tree("gspo")?,
            gpos: db.open_tree("gpos")?,
            gosp: db.open_tree("gosp")?,
            dspo: db.open_tree("dspo")?,
            dpos: db.open_tree("dpos")?,
            dosp: db.open_tree("dosp")?,
            graphs: db.open_tree("graphs")?,
            default: db,
        };

        let mut version = this.ensure_version()?;
        if version == 0 {
            // We migrate to v1
            for quad in this.quads() {
                let quad = quad?;
                if !quad.graph_name.is_default_graph() {
                    this.insert_encoded_named_graph(&quad.graph_name)?;
                }
            }
            version = 1;
            this.set_version(version)?;
            this.default.flush()?;
        }
        if version == 1 {
            // We migrate to v2
            for entry in this.id2str.iter() {
                let (key, value) = entry?;
                let mut new_value = Vec::with_capacity(value.len() + 4);
                new_value.extend_from_slice(&u32::MAX.to_be_bytes());
                new_value.extend_from_slice(&value);
                this.id2str.insert(&key, new_value)?;
            }
            version = 2;
            this.set_version(version)?;
            this.id2str.flush()?;
        }

        match version {
            _ if version < LATEST_STORAGE_VERSION => Err(invalid_data_error(format!(
                "The Sled database is using the outdated encoding version {}. Automated migration is not supported, please dump the store dataset using a compatible Oxigraph version and load it again using the current version",
                version
            ))),
            LATEST_STORAGE_VERSION => Ok(this),
            _ => Err(invalid_data_error(format!(
                "The Sled database is using the too recent version {}. Upgrade to the latest Oxigraph version to load this database",
                version
            )))
        }
    }

    fn ensure_version(&self) -> std::io::Result<u64> {
        Ok(if let Some(version) = self.default.get(b"oxversion")? {
            let mut buffer = [0; 8];
            buffer.copy_from_slice(&version);
            u64::from_be_bytes(buffer)
        } else {
            self.set_version(LATEST_STORAGE_VERSION)?;
            LATEST_STORAGE_VERSION
        })
    }

    fn set_version(&self, version: u64) -> std::io::Result<()> {
        self.default
            .insert(b"oxversion", version.to_be_bytes().to_vec())?;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn transaction<T, E>(
        &self,
        f: impl Fn(StorageTransaction<'_>) -> Result<T, ConflictableTransactionError<E>>,
    ) -> Result<T, TransactionError<E>> {
        use sled::Transactional;

        Ok((
            self.id2str.as_sled(),
            self.spog.as_sled(),
            self.posg.as_sled(),
            self.ospg.as_sled(),
            self.gspo.as_sled(),
            self.gpos.as_sled(),
            self.gosp.as_sled(),
            self.dspo.as_sled(),
            self.dpos.as_sled(),
            self.dosp.as_sled(),
            self.graphs.as_sled(),
        )
            .transaction(
                move |(id2str, spog, posg, ospg, gspo, gpos, gosp, dspo, dpos, dosp, graphs)| {
                    Ok(f(StorageTransaction {
                        id2str,
                        spog,
                        posg,
                        ospg,
                        gspo,
                        gpos,
                        gosp,
                        dspo,
                        dpos,
                        dosp,
                        graphs,
                    })?)
                },
            )?)
    }

    pub fn len(&self) -> usize {
        self.gspo.len() + self.dspo.len()
    }

    pub fn is_empty(&self) -> bool {
        self.gspo.is_empty() && self.dspo.is_empty()
    }

    pub fn contains(&self, quad: &EncodedQuad) -> std::io::Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            Ok(self.dspo.contains_key(&buffer)?)
        } else {
            write_gspo_quad(&mut buffer, quad);
            Ok(self.gspo.contains_key(&buffer)?)
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
            iter: self.graphs.iter(),
        }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> std::io::Result<bool> {
        self.graphs.contains_key(&encode_term(graph_name))
    }

    fn spog_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        Self::inner_quads(&self.spog, prefix, QuadEncoding::Spog)
    }

    fn posg_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        Self::inner_quads(&self.posg, prefix, QuadEncoding::Posg)
    }

    fn ospg_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        Self::inner_quads(&self.ospg, prefix, QuadEncoding::Ospg)
    }

    fn gspo_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        Self::inner_quads(&self.gspo, prefix, QuadEncoding::Gspo)
    }

    fn gpos_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        Self::inner_quads(&self.gpos, prefix, QuadEncoding::Gpos)
    }

    fn gosp_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        Self::inner_quads(&self.gosp, prefix, QuadEncoding::Gosp)
    }

    fn dspo_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        Self::inner_quads(&self.dspo, prefix, QuadEncoding::Dspo)
    }

    fn dpos_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        Self::inner_quads(&self.dpos, prefix, QuadEncoding::Dpos)
    }

    fn dosp_quads(&self, prefix: &[u8]) -> DecodingQuadIterator {
        Self::inner_quads(&self.dosp, prefix, QuadEncoding::Dosp)
    }

    fn inner_quads(tree: &Tree, prefix: &[u8], encoding: QuadEncoding) -> DecodingQuadIterator {
        DecodingQuadIterator {
            iter: tree.scan_prefix(prefix),
            encoding,
        }
    }

    pub fn insert(&self, quad: QuadRef<'_>) -> std::io::Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);
        let encoded = quad.into();

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, &encoded);
            let is_new = self.dspo.insert_empty(buffer.as_slice())?;

            if is_new {
                buffer.clear();
                self.insert_quad_triple(quad, &encoded)?;

                write_pos_quad(&mut buffer, &encoded);
                self.dpos.insert_empty(buffer.as_slice())?;
                buffer.clear();

                write_osp_quad(&mut buffer, &encoded);
                self.dosp.insert_empty(buffer.as_slice())?;
                buffer.clear();
            }

            Ok(is_new)
        } else {
            write_spog_quad(&mut buffer, &encoded);
            let is_new = self.spog.insert_empty(buffer.as_slice())?;
            if is_new {
                buffer.clear();
                self.insert_quad_triple(quad, &encoded)?;

                write_posg_quad(&mut buffer, &encoded);
                self.posg.insert_empty(buffer.as_slice())?;
                buffer.clear();

                write_ospg_quad(&mut buffer, &encoded);
                self.ospg.insert_empty(buffer.as_slice())?;
                buffer.clear();

                write_gspo_quad(&mut buffer, &encoded);
                self.gspo.insert_empty(buffer.as_slice())?;
                buffer.clear();

                write_gpos_quad(&mut buffer, &encoded);
                self.gpos.insert_empty(buffer.as_slice())?;
                buffer.clear();

                write_gosp_quad(&mut buffer, &encoded);
                self.gosp.insert_empty(buffer.as_slice())?;
                buffer.clear();

                write_term(&mut buffer, &encoded.graph_name);
                if self.graphs.insert_empty(&buffer)? {
                    self.insert_graph_name(quad.graph_name, &encoded.graph_name)?;
                }
                buffer.clear();
            }

            Ok(is_new)
        }
    }

    pub fn remove(&self, quad: QuadRef<'_>) -> std::io::Result<bool> {
        self.remove_encoded(&quad.into())
    }

    fn remove_encoded(&self, quad: &EncodedQuad) -> std::io::Result<bool> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            let is_present = self.dspo.remove(buffer.as_slice())?;

            if is_present {
                buffer.clear();

                write_pos_quad(&mut buffer, quad);
                self.dpos.remove(buffer.as_slice())?;
                buffer.clear();

                write_osp_quad(&mut buffer, quad);
                self.dosp.remove(buffer.as_slice())?;
                buffer.clear();

                self.remove_quad_triple(quad)?;
            }

            Ok(is_present)
        } else {
            write_spog_quad(&mut buffer, quad);
            let is_present = self.spog.remove(buffer.as_slice())?;

            if is_present {
                buffer.clear();

                write_posg_quad(&mut buffer, quad);
                self.posg.remove(buffer.as_slice())?;
                buffer.clear();

                write_ospg_quad(&mut buffer, quad);
                self.ospg.remove(buffer.as_slice())?;
                buffer.clear();

                write_gspo_quad(&mut buffer, quad);
                self.gspo.remove(buffer.as_slice())?;
                buffer.clear();

                write_gpos_quad(&mut buffer, quad);
                self.gpos.remove(buffer.as_slice())?;
                buffer.clear();

                write_gosp_quad(&mut buffer, quad);
                self.gosp.remove(buffer.as_slice())?;
                buffer.clear();

                self.remove_quad_triple(quad)?;
            }

            Ok(is_present)
        }
    }

    pub fn insert_named_graph(&self, graph_name: NamedOrBlankNodeRef<'_>) -> std::io::Result<bool> {
        let encoded = graph_name.into();
        Ok(if self.insert_encoded_named_graph(&encoded)? {
            self.insert_term(graph_name.into(), &encoded)?;
            true
        } else {
            false
        })
    }

    fn insert_encoded_named_graph(&self, graph_name: &EncodedTerm) -> std::io::Result<bool> {
        self.graphs.insert_empty(&encode_term(graph_name))
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
        let graph_name = graph_name.into();
        for quad in self.quads_for_graph(&graph_name) {
            self.remove_encoded(&quad?)?;
        }
        Ok(if self.graphs.remove(&encode_term(&graph_name))? {
            self.remove_term(&graph_name)?;
            true
        } else {
            false
        })
    }

    pub fn remove_all_named_graphs(&self) -> std::io::Result<()> {
        self.gspo.clear()?;
        self.gpos.clear()?;
        self.gosp.clear()?;
        self.spog.clear()?;
        self.posg.clear()?;
        self.ospg.clear()?;
        self.graphs.clear()?;
        Ok(())
    }

    pub fn clear(&self) -> std::io::Result<()> {
        self.dspo.clear()?;
        self.dpos.clear()?;
        self.dosp.clear()?;
        self.gspo.clear()?;
        self.gpos.clear()?;
        self.gosp.clear()?;
        self.spog.clear()?;
        self.posg.clear()?;
        self.ospg.clear()?;
        self.graphs.clear()?;
        self.id2str.clear()?;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn flush(&self) -> std::io::Result<()> {
        self.default.flush()?;
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn flush_async(&self) -> std::io::Result<()> {
        self.default.flush_async().await?;
        Ok(())
    }

    pub fn get_str(&self, key: &StrHash) -> std::io::Result<Option<String>> {
        self.id2str
            .get(&key.to_be_bytes())?
            .map(|v| String::from_utf8(v[4..].to_vec()))
            .transpose()
            .map_err(invalid_data_error)
    }

    pub fn contains_str(&self, key: &StrHash) -> std::io::Result<bool> {
        self.id2str.contains_key(&key.to_be_bytes())
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
        Some(match self.iter.next()? {
            Ok((encoded, _)) => self.encoding.decode(&encoded),
            Err(error) => Err(error.into()),
        })
    }
}

pub struct DecodingGraphIterator {
    iter: Iter,
}

impl Iterator for DecodingGraphIterator {
    type Item = std::io::Result<EncodedTerm>;

    fn next(&mut self) -> Option<std::io::Result<EncodedTerm>> {
        Some(match self.iter.next()? {
            Ok((encoded, _)) => decode_term(&encoded),
            Err(error) => Err(error.into()),
        })
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct StorageTransaction<'a> {
    id2str: &'a TransactionalTree,
    spog: &'a TransactionalTree,
    posg: &'a TransactionalTree,
    ospg: &'a TransactionalTree,
    gspo: &'a TransactionalTree,
    gpos: &'a TransactionalTree,
    gosp: &'a TransactionalTree,
    dspo: &'a TransactionalTree,
    dpos: &'a TransactionalTree,
    dosp: &'a TransactionalTree,
    graphs: &'a TransactionalTree,
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> StorageTransaction<'a> {
    pub fn insert(&self, quad: QuadRef<'_>) -> Result<bool, UnabortableTransactionError> {
        let encoded = quad.into();
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, &encoded);
            let is_new = self.dspo.insert(buffer.as_slice(), &[])?.is_none();

            if is_new {
                buffer.clear();
                self.insert_quad_triple(quad, &encoded)?;

                write_pos_quad(&mut buffer, &encoded);
                self.dpos.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_osp_quad(&mut buffer, &encoded);
                self.dosp.insert(buffer.as_slice(), &[])?;
                buffer.clear();
            }

            Ok(is_new)
        } else {
            write_spog_quad(&mut buffer, &encoded);
            let is_new = self.spog.insert(buffer.as_slice(), &[])?.is_none();

            if is_new {
                buffer.clear();
                self.insert_quad_triple(quad, &encoded)?;

                write_posg_quad(&mut buffer, &encoded);
                self.posg.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_ospg_quad(&mut buffer, &encoded);
                self.ospg.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_gspo_quad(&mut buffer, &encoded);
                self.gspo.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_gpos_quad(&mut buffer, &encoded);
                self.gpos.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_gosp_quad(&mut buffer, &encoded);
                self.gosp.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_term(&mut buffer, &encoded.graph_name);
                if self.graphs.insert(buffer.as_slice(), &[])?.is_none() {
                    self.insert_graph_name(quad.graph_name, &encoded.graph_name)?;
                }
                buffer.clear();
            }

            Ok(is_new)
        }
    }

    pub fn remove(&self, quad: QuadRef<'_>) -> Result<bool, UnabortableTransactionError> {
        let quad = EncodedQuad::from(quad);
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, &quad);
            let is_present = self.dspo.remove(buffer.as_slice())?.is_some();

            if is_present {
                buffer.clear();

                write_pos_quad(&mut buffer, &quad);
                self.dpos.remove(buffer.as_slice())?;
                buffer.clear();

                write_osp_quad(&mut buffer, &quad);
                self.dosp.remove(buffer.as_slice())?;
                buffer.clear();

                self.remove_quad_triple(&quad)?;
            }

            Ok(is_present)
        } else {
            write_spog_quad(&mut buffer, &quad);
            let is_present = self.spog.remove(buffer.as_slice())?.is_some();

            if is_present {
                buffer.clear();

                write_posg_quad(&mut buffer, &quad);
                self.posg.remove(buffer.as_slice())?;
                buffer.clear();

                write_ospg_quad(&mut buffer, &quad);
                self.ospg.remove(buffer.as_slice())?;
                buffer.clear();

                write_gspo_quad(&mut buffer, &quad);
                self.gspo.remove(buffer.as_slice())?;
                buffer.clear();

                write_gpos_quad(&mut buffer, &quad);
                self.gpos.remove(buffer.as_slice())?;
                buffer.clear();

                write_gosp_quad(&mut buffer, &quad);
                self.gosp.remove(buffer.as_slice())?;
                buffer.clear();

                self.remove_quad_triple(&quad)?;
            }

            Ok(is_present)
        }
    }

    pub fn insert_named_graph(
        &self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, UnabortableTransactionError> {
        let encoded = graph_name.into();
        Ok(
            if self.graphs.insert(encode_term(&encoded), &[])?.is_none() {
                self.insert_term(graph_name.into(), &encoded)?;
                true
            } else {
                false
            },
        )
    }

    pub fn get_str(&self, key: &StrHash) -> Result<Option<String>, UnabortableTransactionError> {
        self.id2str
            .get(key.to_be_bytes())?
            .map(|v| String::from_utf8(v[4..].to_vec()))
            .transpose()
            .map_err(|e| UnabortableTransactionError::Storage(invalid_data_error(e)))
    }

    pub fn contains_str(&self, key: &StrHash) -> Result<bool, UnabortableTransactionError> {
        Ok(self.id2str.get(key.to_be_bytes())?.is_some())
    }
}

/// Error returned by a Sled transaction
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub enum TransactionError<T> {
    /// A failure returned by the API user that have aborted the transaction
    Abort(T),
    /// A storage related error
    Storage(std::io::Error),
}

#[cfg(not(target_arch = "wasm32"))]
impl<T: fmt::Display> fmt::Display for TransactionError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Abort(e) => e.fmt(f),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Error + 'static> Error for TransactionError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Abort(e) => Some(e),
            Self::Storage(e) => Some(e),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> From<Sled2TransactionError<T>> for TransactionError<T> {
    fn from(e: Sled2TransactionError<T>) -> Self {
        match e {
            Sled2TransactionError::Abort(e) => Self::Abort(e),
            Sled2TransactionError::Storage(e) => Self::Storage(e.into()),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Into<Self>> From<TransactionError<T>> for std::io::Error {
    fn from(e: TransactionError<T>) -> Self {
        match e {
            TransactionError::Abort(e) => e.into(),
            TransactionError::Storage(e) => e,
        }
    }
}

/// An error returned from the transaction methods.
/// Should be returned as it is
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub enum UnabortableTransactionError {
    #[doc(hidden)]
    Conflict,
    /// A regular error
    Storage(std::io::Error),
}

#[cfg(not(target_arch = "wasm32"))]
impl fmt::Display for UnabortableTransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(f, "Transaction conflict"),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Error for UnabortableTransactionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<UnabortableTransactionError> for EvaluationError {
    fn from(e: UnabortableTransactionError) -> Self {
        match e {
            UnabortableTransactionError::Storage(e) => Self::Io(e),
            UnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<StoreOrParseError<Self>> for UnabortableTransactionError {
    fn from(e: StoreOrParseError<Self>) -> Self {
        match e {
            StoreOrParseError::Store(e) => e,
            StoreOrParseError::Parse(e) => Self::Storage(e),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl From<Sled2UnabortableTransactionError> for UnabortableTransactionError {
    fn from(e: Sled2UnabortableTransactionError) -> Self {
        match e {
            Sled2UnabortableTransactionError::Storage(e) => Self::Storage(e.into()),
            Sled2UnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

/// An error returned from the transaction closure
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug)]
pub enum ConflictableTransactionError<T> {
    /// A failure returned by the user that will abort the transaction
    Abort(T),
    #[doc(hidden)]
    Conflict,
    /// A storage related error
    Storage(std::io::Error),
}

#[cfg(not(target_arch = "wasm32"))]
impl<T: fmt::Display> fmt::Display for ConflictableTransactionError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(f, "Transaction conflict"),
            Self::Storage(e) => e.fmt(f),
            Self::Abort(e) => e.fmt(f),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T: Error + 'static> Error for ConflictableTransactionError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Abort(e) => Some(e),
            Self::Storage(e) => Some(e),
            _ => None,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> From<UnabortableTransactionError> for ConflictableTransactionError<T> {
    fn from(e: UnabortableTransactionError) -> Self {
        match e {
            UnabortableTransactionError::Storage(e) => Self::Storage(e),
            UnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<T> From<ConflictableTransactionError<T>> for Sled2ConflictableTransactionError<T> {
    fn from(e: ConflictableTransactionError<T>) -> Self {
        match e {
            ConflictableTransactionError::Abort(e) => Self::Abort(e),
            ConflictableTransactionError::Conflict => Self::Conflict,
            ConflictableTransactionError::Storage(e) => Self::Storage(e.into()),
        }
    }
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

#[cfg(not(target_arch = "wasm32"))]
impl<'a> StrLookup for StorageTransaction<'a> {
    type Error = UnabortableTransactionError;

    fn get_str(&self, key: &StrHash) -> Result<Option<String>, UnabortableTransactionError> {
        self.get_str(key)
    }

    fn contains_str(&self, key: &StrHash) -> Result<bool, UnabortableTransactionError> {
        self.contains_str(key)
    }
}

impl TermEncoder for Storage {
    type Error = std::io::Error;

    fn insert_str(&self, key: &StrHash, value: &str) -> std::io::Result<()> {
        self.id2str.merge(&key.to_be_bytes(), value.as_bytes())?;
        Ok(())
    }

    fn remove_str(&self, key: &StrHash) -> std::io::Result<()> {
        self.id2str.update_and_fetch(&key.to_be_bytes(), |old| {
            let old = old?;
            match u32::from_be_bytes(old[..4].try_into().ok()?) {
                0 | 1 => None,
                u32::MAX => Some(old.to_vec()),
                number => {
                    let mut value = old.to_vec();
                    value[..4].copy_from_slice(&(number - 1).to_be_bytes());
                    Some(value)
                }
            }
        })?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> TermEncoder for StorageTransaction<'a> {
    type Error = UnabortableTransactionError;

    fn insert_str(&self, key: &StrHash, value: &str) -> Result<(), UnabortableTransactionError> {
        let new_value = if let Some(old) = self.id2str.get(key.to_be_bytes())? {
            let mut new_value = old.to_vec();
            let number = u32::from_be_bytes(new_value[..4].try_into().ok().unwrap_or_default());
            new_value[..4].copy_from_slice(&number.saturating_add(1).to_be_bytes()); //TODO: check
            new_value
        } else {
            let mut new_value = Vec::with_capacity(value.len() + 4);
            new_value.extend_from_slice(&1_u32.to_be_bytes());
            new_value.extend_from_slice(value.as_bytes());
            new_value
        };
        self.id2str.insert(&key.to_be_bytes(), new_value)?;
        Ok(())
    }

    fn remove_str(&self, key: &StrHash) -> Result<(), UnabortableTransactionError> {
        if let Some(old) = self.id2str.get(key.to_be_bytes())? {
            if let Ok(number) = old[..4].try_into() {
                match u32::from_be_bytes(number) {
                    0 | 1 => {
                        self.id2str.remove(&key.to_be_bytes())?;
                    }
                    u32::MAX => (),
                    number => {
                        let mut value = old;
                        value[..4].copy_from_slice(&(number - 1).to_be_bytes());
                        self.id2str.insert(&key.to_be_bytes(), value)?;
                    }
                }
            }
        }
        Ok(())
    }
}

pub trait StorageLike: StrLookup {
    fn insert(&self, quad: QuadRef<'_>) -> Result<bool, Self::Error>;

    fn remove(&self, quad: QuadRef<'_>) -> Result<bool, Self::Error>;
}

impl StorageLike for Storage {
    fn insert(&self, quad: QuadRef<'_>) -> std::io::Result<bool> {
        self.insert(quad)
    }

    fn remove(&self, quad: QuadRef<'_>) -> std::io::Result<bool> {
        self.remove(quad)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl<'a> StorageLike for StorageTransaction<'a> {
    fn insert(&self, quad: QuadRef<'_>) -> Result<bool, UnabortableTransactionError> {
        self.insert(quad)
    }

    fn remove(&self, quad: QuadRef<'_>) -> Result<bool, UnabortableTransactionError> {
        self.remove(quad)
    }
}

fn id2str_merge(
    _key: &[u8],              // the key being merged
    old_value: Option<&[u8]>, // the previous value, if one existed
    merged_bytes: &[u8],      // the new bytes being merged in
) -> Option<Vec<u8>> {
    Some(if let Some(value) = old_value {
        let mut value = value.to_vec();
        let number = u32::from_be_bytes(value[..4].try_into().ok()?);
        value[..4].copy_from_slice(&number.saturating_add(1).to_be_bytes()); //TODO: check
        value
    } else {
        let mut value = Vec::with_capacity(merged_bytes.len() + 4);
        value.extend_from_slice(&1_u32.to_be_bytes());
        value.extend_from_slice(merged_bytes);
        value
    })
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

    #[test]
    #[cfg(not(target_arch = "wasm32"))]
    fn test_strings_removal_in_transaction() -> std::io::Result<()> {
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
        transac(&storage, |t| t.insert(quad))?;
        transac(&storage, |t| t.insert(quad2))?;
        transac(&storage, |t| t.remove(quad2))?;
        assert!(storage
            .get_str(&StrHash::new("http://example.com/s"))?
            .is_some());
        assert!(storage
            .get_str(&StrHash::new("http://example.com/p"))?
            .is_some());
        assert!(storage
            .get_str(&StrHash::new("http://example.com/o2"))?
            .is_none());
        transac(&storage, |t| t.remove(quad))?;
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
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn transac<T>(
        storage: &Storage,
        f: impl Fn(StorageTransaction<'_>) -> Result<T, UnabortableTransactionError>,
    ) -> Result<(), TransactionError<std::io::Error>> {
        storage.transaction(|t| {
            f(t)?;
            Ok(())
        })
    }
}
