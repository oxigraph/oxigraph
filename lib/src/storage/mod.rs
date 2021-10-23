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
use fallback_backend::{Db, Iter, Tree};
#[cfg(not(target_arch = "wasm32"))]
use rocksdb_backend::{Db, Iter, Tree};
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

const COLUMN_FAMILIES: [&str; 11] = [
    ID2STR_CF, SPOG_CF, POSG_CF, OSPG_CF, GSPO_CF, GPOS_CF, GOSP_CF, DSPO_CF, DPOS_CF, DOSP_CF,
    GRAPHS_CF,
];

/// Low level storage primitives
#[derive(Clone)]
pub struct Storage {
    db: Db,
    default: Tree,
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
        Self::setup(Db::new(&COLUMN_FAMILIES)?)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(path: &Path) -> std::io::Result<Self> {
        Self::setup(Db::open(path, &COLUMN_FAMILIES)?)
    }

    fn setup(db: Db) -> std::io::Result<Self> {
        let this = Self {
            default: db.open_tree(DEFAULT_CF)?,
            id2str: db.open_tree(ID2STR_CF)?,
            spog: db.open_tree(SPOG_CF)?,
            posg: db.open_tree(POSG_CF)?,
            ospg: db.open_tree(OSPG_CF)?,
            gspo: db.open_tree(GSPO_CF)?,
            gpos: db.open_tree(GPOS_CF)?,
            gosp: db.open_tree(GOSP_CF)?,
            dspo: db.open_tree(DSPO_CF)?,
            dpos: db.open_tree(DPOS_CF)?,
            dosp: db.open_tree(DOSP_CF)?,
            graphs: db.open_tree(GRAPHS_CF)?,
            db,
        };

        let mut version = this.ensure_version()?;
        if version == 0 {
            // We migrate to v1
            for quad in this.quads() {
                let quad = quad?;
                if !quad.graph_name.is_default_graph() {
                    this.graphs.insert_empty(&encode_term(&quad.graph_name))?;
                }
            }
            version = 1;
            this.set_version(version)?;
            this.db.flush()?;
        }
        if version == 1 {
            // We migrate to v2
            let mut iter = this.id2str.iter();
            while let (Some(key), Some(value)) = (iter.key(), iter.value()) {
                let mut new_value = Vec::with_capacity(value.len() + 4);
                new_value.extend_from_slice(&u32::MAX.to_be_bytes());
                new_value.extend_from_slice(value);
                this.id2str.insert(key, &new_value)?;
                iter.next();
            }
            iter.status()?;
            version = 2;
            this.set_version(version)?;
            this.db.flush()?;
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
        self.default.insert(b"oxversion", &version.to_be_bytes())?;
        Ok(())
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

        Ok(if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, &encoded);
            if self.dspo.contains_key(buffer.as_slice())? {
                false
            } else {
                self.insert_quad_triple(quad, &encoded)?;

                self.dspo.insert_empty(buffer.as_slice())?;
                buffer.clear();

                write_pos_quad(&mut buffer, &encoded);
                self.dpos.insert_empty(buffer.as_slice())?;
                buffer.clear();

                write_osp_quad(&mut buffer, &encoded);
                self.dosp.insert_empty(buffer.as_slice())?;
                buffer.clear();

                true
            }
        } else {
            write_spog_quad(&mut buffer, &encoded);
            if self.spog.contains_key(buffer.as_slice())? {
                false
            } else {
                self.insert_quad_triple(quad, &encoded)?;

                self.spog.insert_empty(buffer.as_slice())?;
                buffer.clear();

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
                if !self.graphs.contains_key(&buffer)? {
                    self.graphs.insert_empty(&buffer)?;
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

            if self.dspo.contains_key(buffer.as_slice())? {
                self.dspo.remove(buffer.as_slice())?;
                buffer.clear();

                write_pos_quad(&mut buffer, quad);
                self.dpos.remove(buffer.as_slice())?;
                buffer.clear();

                write_osp_quad(&mut buffer, quad);
                self.dosp.remove(buffer.as_slice())?;
                buffer.clear();

                self.remove_quad_triple(quad)?;

                true
            } else {
                false
            }
        } else {
            write_spog_quad(&mut buffer, quad);

            if self.spog.contains_key(buffer.as_slice())? {
                self.spog.remove(buffer.as_slice())?;
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

                true
            } else {
                false
            }
        })
    }

    pub fn insert_named_graph(&self, graph_name: NamedOrBlankNodeRef<'_>) -> std::io::Result<bool> {
        let encoded_graph_name = graph_name.into();
        let encoded = encode_term(&encoded_graph_name);
        Ok(if self.graphs.contains_key(&encoded)? {
            false
        } else {
            self.graphs.insert_empty(&encoded)?;
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
        let graph_name = graph_name.into();
        for quad in self.quads_for_graph(&graph_name) {
            self.remove_encoded(&quad?)?;
        }
        let encoded_graph = encode_term(&graph_name);
        Ok(if self.graphs.contains_key(&encoded_graph)? {
            self.graphs.remove(&encoded_graph)?;
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
        self.db.flush()
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
        if let Some(value) = self.id2str.get(&key.to_be_bytes())? {
            let mut value = value.to_vec();
            let number = u32::from_be_bytes(value[..4].try_into().map_err(invalid_data_error)?);
            let new_number = number.saturating_add(1);
            value[..4].copy_from_slice(&new_number.to_be_bytes());
            self.id2str.insert(&key.to_be_bytes(), &value)?
        } else {
            let mut buffer = Vec::with_capacity(value.len() + 4);
            buffer.extend_from_slice(&1_u32.to_be_bytes());
            buffer.extend_from_slice(value.as_bytes());
            self.id2str.insert(&key.to_be_bytes(), &buffer)?;
        }
        Ok(())
    }

    fn remove_str(&self, key: &StrHash) -> std::io::Result<()> {
        if let Some(value) = self.id2str.get(&key.to_be_bytes())? {
            let number = u32::from_be_bytes(value[..4].try_into().map_err(invalid_data_error)?);
            let new_number = number.saturating_sub(1);
            if new_number == 0 {
                self.id2str.remove(&key.to_be_bytes())?;
            } else {
                let mut value = value.to_vec();
                value[..4].copy_from_slice(&new_number.to_be_bytes());
                self.id2str.insert(&key.to_be_bytes(), &value)?;
            }
        }
        Ok(())
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
