use crate::error::invalid_data_error;
use crate::sparql::EvaluationError;
use crate::store::binary_encoder::*;
use crate::store::numeric_encoder::*;
use crate::store::StoreOrParseError;
use sled::transaction::{
    ConflictableTransactionError, TransactionError, TransactionalTree, UnabortableTransactionError,
};
use sled::{Config, Db, Iter, Transactional, Tree};
use std::error::Error;
use std::fmt;
use std::io;
use std::path::Path;

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
    pub fn new() -> Result<Self, io::Error> {
        Self::do_open(&Config::new().temporary(true))
    }

    pub fn open(path: &Path) -> Result<Self, io::Error> {
        Self::do_open(&Config::new().path(path))
    }

    fn do_open(config: &Config) -> Result<Self, io::Error> {
        let db = config.open()?;
        let this = Self {
            default: db.clone(),
            id2str: db.open_tree("id2str")?,
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
        };

        let mut version = this.ensure_version()?;
        if version == 0 {
            // We migrate to v1
            for quad in this.quads() {
                let quad = quad?;
                if !quad.graph_name.is_default_graph() {
                    this.insert_named_graph(quad.graph_name)?;
                }
            }
            version = 1;
            this.set_version(version)?;
            this.graphs.flush()?;
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

    fn ensure_version(&self) -> Result<u64, io::Error> {
        Ok(if let Some(version) = self.default.get("oxversion")? {
            let mut buffer = [0; 8];
            buffer.copy_from_slice(&version);
            u64::from_be_bytes(buffer)
        } else {
            self.set_version(LATEST_STORAGE_VERSION)?;
            LATEST_STORAGE_VERSION
        })
    }

    fn set_version(&self, version: u64) -> Result<(), io::Error> {
        self.default.insert("oxversion", &version.to_be_bytes())?;
        Ok(())
    }

    pub fn transaction<T, E>(
        &self,
        f: impl Fn(StorageTransaction<'_>) -> Result<T, SledConflictableTransactionError<E>>,
    ) -> Result<T, SledTransactionError<E>> {
        Ok((
            &self.id2str,
            &self.spog,
            &self.posg,
            &self.ospg,
            &self.gspo,
            &self.gpos,
            &self.gosp,
            &self.dspo,
            &self.dpos,
            &self.dosp,
            &self.graphs,
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

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool, io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            Ok(self.dspo.contains_key(buffer)?)
        } else {
            write_gspo_quad(&mut buffer, quad);
            Ok(self.gspo.contains_key(buffer)?)
        }
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
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
        ChainedDecodingQuadIterator::pair(
            self.dspo_quads(Vec::default()),
            self.gspo_quads(Vec::default()),
        )
    }

    fn quads_for_subject(&self, subject: EncodedTerm) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dspo_quads(encode_term(subject)),
            self.spog_quads(encode_term(subject)),
        )
    }

    fn quads_for_subject_predicate(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dspo_quads(encode_term_pair(subject, predicate)),
            self.spog_quads(encode_term_pair(subject, predicate)),
        )
    }

    fn quads_for_subject_predicate_object(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dspo_quads(encode_term_triple(subject, predicate, object)),
            self.spog_quads(encode_term_triple(subject, predicate, object)),
        )
    }

    fn quads_for_subject_object(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dosp_quads(encode_term_pair(object, subject)),
            self.ospg_quads(encode_term_pair(object, subject)),
        )
    }

    fn quads_for_predicate(&self, predicate: EncodedTerm) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dpos_quads(encode_term(predicate)),
            self.posg_quads(encode_term(predicate)),
        )
    }

    fn quads_for_predicate_object(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dpos_quads(encode_term_pair(predicate, object)),
            self.posg_quads(encode_term_pair(predicate, object)),
        )
    }

    fn quads_for_object(&self, object: EncodedTerm) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::pair(
            self.dosp_quads(encode_term(object)),
            self.ospg_quads(encode_term(object)),
        )
    }

    fn quads_for_graph(&self, graph_name: EncodedTerm) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(Vec::default())
        } else {
            self.gspo_quads(encode_term(graph_name))
        })
    }

    fn quads_for_subject_graph(
        &self,
        subject: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(encode_term(subject))
        } else {
            self.gspo_quads(encode_term_pair(graph_name, subject))
        })
    }

    fn quads_for_subject_predicate_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(encode_term_pair(subject, predicate))
        } else {
            self.gspo_quads(encode_term_triple(graph_name, subject, predicate))
        })
    }

    fn quads_for_subject_predicate_object_graph(
        &self,
        subject: EncodedTerm,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dspo_quads(encode_term_triple(subject, predicate, object))
        } else {
            self.gspo_quads(encode_term_quad(graph_name, subject, predicate, object))
        })
    }

    fn quads_for_subject_object_graph(
        &self,
        subject: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(encode_term_pair(object, subject))
        } else {
            self.gosp_quads(encode_term_triple(graph_name, object, subject))
        })
    }

    fn quads_for_predicate_graph(
        &self,
        predicate: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(encode_term(predicate))
        } else {
            self.gpos_quads(encode_term_pair(graph_name, predicate))
        })
    }

    fn quads_for_predicate_object_graph(
        &self,
        predicate: EncodedTerm,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dpos_quads(encode_term_pair(predicate, object))
        } else {
            self.gpos_quads(encode_term_triple(graph_name, predicate, object))
        })
    }

    fn quads_for_object_graph(
        &self,
        object: EncodedTerm,
        graph_name: EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
            self.dosp_quads(encode_term(object))
        } else {
            self.gosp_quads(encode_term_pair(graph_name, object))
        })
    }

    pub fn named_graphs(&self) -> DecodingGraphIterator {
        DecodingGraphIterator {
            iter: self.graphs.iter(),
        }
    }

    pub fn contains_named_graph(&self, graph_name: EncodedTerm) -> Result<bool, io::Error> {
        Ok(self.graphs.contains_key(&encode_term(graph_name))?)
    }

    fn spog_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.spog, prefix, QuadEncoding::Spog)
    }

    fn posg_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.posg, prefix, QuadEncoding::Posg)
    }

    fn ospg_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.ospg, prefix, QuadEncoding::Ospg)
    }

    fn gspo_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gspo, prefix, QuadEncoding::Gspo)
    }

    fn gpos_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gpos, prefix, QuadEncoding::Gpos)
    }

    fn gosp_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.gosp, prefix, QuadEncoding::Gosp)
    }

    fn dspo_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.dspo, prefix, QuadEncoding::Dspo)
    }

    fn dpos_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.dpos, prefix, QuadEncoding::Dpos)
    }

    fn dosp_quads(&self, prefix: Vec<u8>) -> DecodingQuadIterator {
        self.inner_quads(&self.dosp, prefix, QuadEncoding::Dosp)
    }

    fn inner_quads(
        &self,
        tree: &Tree,
        prefix: impl AsRef<[u8]>,
        encoding: QuadEncoding,
    ) -> DecodingQuadIterator {
        DecodingQuadIterator {
            iter: tree.scan_prefix(prefix),
            encoding,
        }
    }

    pub fn insert(&self, quad: &EncodedQuad) -> Result<bool, io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            let is_new = self.dspo.insert(buffer.as_slice(), &[])?.is_none();

            if is_new {
                buffer.clear();

                write_pos_quad(&mut buffer, quad);
                self.dpos.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_osp_quad(&mut buffer, quad);
                self.dosp.insert(buffer.as_slice(), &[])?;
                buffer.clear();
            }

            Ok(is_new)
        } else {
            write_spog_quad(&mut buffer, quad);
            let is_new = self.spog.insert(buffer.as_slice(), &[])?.is_none();
            if is_new {
                buffer.clear();

                write_posg_quad(&mut buffer, quad);
                self.posg.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_ospg_quad(&mut buffer, quad);
                self.ospg.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_gspo_quad(&mut buffer, quad);
                self.gspo.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_gpos_quad(&mut buffer, quad);
                self.gpos.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_gosp_quad(&mut buffer, quad);
                self.gosp.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_term(&mut buffer, quad.graph_name);
                self.graphs.insert(&buffer, &[])?;
                buffer.clear();
            }

            Ok(is_new)
        }
    }

    pub fn remove(&self, quad: &EncodedQuad) -> Result<bool, io::Error> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            let is_present = self.dspo.remove(buffer.as_slice())?.is_some();

            if is_present {
                buffer.clear();

                write_pos_quad(&mut buffer, quad);
                self.dpos.remove(buffer.as_slice())?;
                buffer.clear();

                write_osp_quad(&mut buffer, quad);
                self.dosp.remove(buffer.as_slice())?;
                buffer.clear();
            }

            Ok(is_present)
        } else {
            write_spog_quad(&mut buffer, quad);
            let is_present = self.spog.remove(buffer.as_slice())?.is_some();

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
            }

            Ok(is_present)
        }
    }

    pub fn insert_named_graph(&self, graph_name: EncodedTerm) -> Result<bool, io::Error> {
        Ok(self.graphs.insert(&encode_term(graph_name), &[])?.is_none())
    }

    pub fn clear_graph(&self, graph_name: EncodedTerm) -> Result<(), io::Error> {
        if graph_name.is_default_graph() {
            self.dspo.clear()?;
            self.dpos.clear()?;
            self.dosp.clear()?;
        } else {
            for quad in self.quads_for_graph(graph_name) {
                self.remove(&quad?)?;
            }
        }
        Ok(())
    }

    pub fn remove_named_graph(&self, graph_name: EncodedTerm) -> Result<bool, io::Error> {
        for quad in self.quads_for_graph(graph_name) {
            self.remove(&quad?)?;
        }
        Ok(self.graphs.remove(&encode_term(graph_name))?.is_some())
    }

    pub fn clear(&self) -> Result<(), io::Error> {
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

    pub fn get_str(&self, key: StrHash) -> Result<Option<String>, io::Error> {
        self.id2str
            .get(key.to_be_bytes())?
            .map(|v| String::from_utf8(v.to_vec()))
            .transpose()
            .map_err(invalid_data_error)
    }

    pub fn contains_str(&self, key: StrHash) -> Result<bool, io::Error> {
        Ok(self.id2str.contains_key(key.to_be_bytes())?)
    }

    pub fn insert_str(&self, key: StrHash, value: &str) -> Result<bool, io::Error> {
        Ok(self.id2str.insert(key.to_be_bytes(), value)?.is_none())
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
    type Item = Result<EncodedQuad, io::Error>;

    fn next(&mut self) -> Option<Result<EncodedQuad, io::Error>> {
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
    type Item = Result<EncodedQuad, io::Error>;

    fn next(&mut self) -> Option<Result<EncodedQuad, io::Error>> {
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
    type Item = Result<EncodedTerm, io::Error>;

    fn next(&mut self) -> Option<Result<EncodedTerm, io::Error>> {
        Some(match self.iter.next()? {
            Ok((encoded, _)) => decode_term(&encoded),
            Err(error) => Err(error.into()),
        })
    }
}

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

impl<'a> StorageTransaction<'a> {
    pub fn insert(&self, quad: &EncodedQuad) -> Result<bool, SledUnabortableTransactionError> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            let is_new = self.dspo.insert(buffer.as_slice(), &[])?.is_none();

            if is_new {
                buffer.clear();

                write_pos_quad(&mut buffer, quad);
                self.dpos.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_osp_quad(&mut buffer, quad);
                self.dosp.insert(buffer.as_slice(), &[])?;
                buffer.clear();
            }

            Ok(is_new)
        } else {
            write_spog_quad(&mut buffer, quad);
            let is_new = self.spog.insert(buffer.as_slice(), &[])?.is_none();

            if is_new {
                buffer.clear();

                write_posg_quad(&mut buffer, quad);
                self.posg.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_ospg_quad(&mut buffer, quad);
                self.ospg.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_gspo_quad(&mut buffer, quad);
                self.gspo.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_gpos_quad(&mut buffer, quad);
                self.gpos.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_gosp_quad(&mut buffer, quad);
                self.gosp.insert(buffer.as_slice(), &[])?;
                buffer.clear();

                write_term(&mut buffer, quad.graph_name);
                self.graphs.insert(buffer.as_slice(), &[])?;
                buffer.clear();
            }

            Ok(is_new)
        }
    }

    pub fn remove(&self, quad: &EncodedQuad) -> Result<bool, SledUnabortableTransactionError> {
        let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE + 1);

        if quad.graph_name.is_default_graph() {
            write_spo_quad(&mut buffer, quad);
            let is_present = self.dspo.remove(buffer.as_slice())?.is_some();

            if is_present {
                buffer.clear();

                write_pos_quad(&mut buffer, quad);
                self.dpos.remove(buffer.as_slice())?;
                buffer.clear();

                write_osp_quad(&mut buffer, quad);
                self.dosp.remove(buffer.as_slice())?;
                buffer.clear();
            }

            Ok(is_present)
        } else {
            write_spog_quad(&mut buffer, quad);
            let is_present = self.spog.remove(buffer.as_slice())?.is_some();

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
            }

            Ok(is_present)
        }
    }

    pub fn insert_named_graph(
        &self,
        graph_name: EncodedTerm,
    ) -> Result<bool, SledUnabortableTransactionError> {
        Ok(self.graphs.insert(encode_term(graph_name), &[])?.is_none())
    }

    pub fn get_str(&self, key: StrHash) -> Result<Option<String>, SledUnabortableTransactionError> {
        self.id2str
            .get(key.to_be_bytes())?
            .map(|v| String::from_utf8(v.to_vec()))
            .transpose()
            .map_err(|e| SledUnabortableTransactionError::Storage(invalid_data_error(e)))
    }

    pub fn contains_str(&self, key: StrHash) -> Result<bool, SledUnabortableTransactionError> {
        Ok(self.id2str.get(key.to_be_bytes())?.is_some())
    }

    pub fn insert_str(
        &self,
        key: StrHash,
        value: &str,
    ) -> Result<bool, SledUnabortableTransactionError> {
        Ok(self.id2str.insert(&key.to_be_bytes(), value)?.is_none())
    }
}

/// Error returned by a Sled transaction
#[derive(Debug)]
pub enum SledTransactionError<T> {
    /// A failure returned by the API user that have aborted the transaction
    Abort(T),
    /// A storage related error
    Storage(io::Error),
}

impl<T: fmt::Display> fmt::Display for SledTransactionError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Abort(e) => e.fmt(f),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl<T: Error + 'static> Error for SledTransactionError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Abort(e) => Some(e),
            Self::Storage(e) => Some(e),
        }
    }
}

impl<T> From<TransactionError<T>> for SledTransactionError<T> {
    fn from(e: TransactionError<T>) -> Self {
        match e {
            TransactionError::Abort(e) => Self::Abort(e),
            TransactionError::Storage(e) => Self::Storage(e.into()),
        }
    }
}

impl<T: Into<io::Error>> From<SledTransactionError<T>> for io::Error {
    fn from(e: SledTransactionError<T>) -> Self {
        match e {
            SledTransactionError::Abort(e) => e.into(),
            SledTransactionError::Storage(e) => e,
        }
    }
}

/// An error returned from the transaction methods.
/// Should be returned as it is
#[derive(Debug)]
pub enum SledUnabortableTransactionError {
    #[doc(hidden)]
    Conflict,
    /// A regular error
    Storage(io::Error),
}

impl fmt::Display for SledUnabortableTransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(f, "Transaction conflict"),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl Error for SledUnabortableTransactionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(e) => Some(e),
            _ => None,
        }
    }
}

impl From<SledUnabortableTransactionError> for EvaluationError {
    fn from(e: SledUnabortableTransactionError) -> Self {
        match e {
            SledUnabortableTransactionError::Storage(e) => Self::Io(e),
            SledUnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

impl From<StoreOrParseError<SledUnabortableTransactionError>> for SledUnabortableTransactionError {
    fn from(e: StoreOrParseError<SledUnabortableTransactionError>) -> Self {
        match e {
            StoreOrParseError::Store(e) => e,
            StoreOrParseError::Parse(e) => Self::Storage(e),
        }
    }
}

impl From<UnabortableTransactionError> for SledUnabortableTransactionError {
    fn from(e: UnabortableTransactionError) -> Self {
        match e {
            UnabortableTransactionError::Storage(e) => Self::Storage(e.into()),
            UnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

/// An error returned from the transaction closure
#[derive(Debug)]
pub enum SledConflictableTransactionError<T> {
    /// A failure returned by the user that will abort the transaction
    Abort(T),
    #[doc(hidden)]
    Conflict,
    /// A storage related error
    Storage(io::Error),
}

impl<T: fmt::Display> fmt::Display for SledConflictableTransactionError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict => write!(f, "Transaction conflict"),
            Self::Storage(e) => e.fmt(f),
            Self::Abort(e) => e.fmt(f),
        }
    }
}

impl<T: Error + 'static> Error for SledConflictableTransactionError<T> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Abort(e) => Some(e),
            Self::Storage(e) => Some(e),
            _ => None,
        }
    }
}

impl<T> From<SledUnabortableTransactionError> for SledConflictableTransactionError<T> {
    fn from(e: SledUnabortableTransactionError) -> Self {
        match e {
            SledUnabortableTransactionError::Storage(e) => Self::Storage(e),
            SledUnabortableTransactionError::Conflict => Self::Conflict,
        }
    }
}

impl<T> From<SledConflictableTransactionError<T>> for ConflictableTransactionError<T> {
    fn from(e: SledConflictableTransactionError<T>) -> Self {
        match e {
            SledConflictableTransactionError::Abort(e) => ConflictableTransactionError::Abort(e),
            SledConflictableTransactionError::Conflict => ConflictableTransactionError::Conflict,
            SledConflictableTransactionError::Storage(e) => {
                ConflictableTransactionError::Storage(e.into())
            }
        }
    }
}

impl StrEncodingAware for Storage {
    type Error = io::Error;
}

impl StrLookup for Storage {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, io::Error> {
        self.get_str(id)
    }

    fn get_str_id(&self, value: &str) -> Result<Option<StrHash>, io::Error> {
        let key = StrHash::new(value);
        Ok(if self.contains_str(key)? {
            Some(key)
        } else {
            None
        })
    }
}

impl StrContainer for Storage {
    fn insert_str(&self, value: &str) -> Result<StrHash, io::Error> {
        let key = StrHash::new(value);
        self.insert_str(key, value)?;
        Ok(key)
    }
}

impl<'a> StrEncodingAware for StorageTransaction<'a> {
    type Error = SledUnabortableTransactionError;
}

impl<'a> StrLookup for StorageTransaction<'a> {
    fn get_str(&self, id: StrHash) -> Result<Option<String>, SledUnabortableTransactionError> {
        self.get_str(id)
    }

    fn get_str_id(&self, value: &str) -> Result<Option<StrHash>, SledUnabortableTransactionError> {
        let key = StrHash::new(value);
        Ok(if self.contains_str(key)? {
            Some(key)
        } else {
            None
        })
    }
}

impl<'a> StrContainer for StorageTransaction<'a> {
    fn insert_str(&self, value: &str) -> Result<StrHash, SledUnabortableTransactionError> {
        let key = StrHash::new(value);
        self.insert_str(key, value)?;
        Ok(key)
    }
}

pub(crate) trait StorageLike: StrLookup + StrContainer {
    fn insert(&self, quad: &EncodedQuad) -> Result<bool, Self::Error>;

    fn remove(&self, quad: &EncodedQuad) -> Result<bool, Self::Error>;
}

impl StorageLike for Storage {
    fn insert(&self, quad: &EncodedQuad) -> Result<bool, Self::Error> {
        self.insert(quad)
    }

    fn remove(&self, quad: &EncodedQuad) -> Result<bool, Self::Error> {
        self.remove(quad)
    }
}

impl<'a> StorageLike for StorageTransaction<'a> {
    fn insert(&self, quad: &EncodedQuad) -> Result<bool, Self::Error> {
        self.insert(quad)
    }

    fn remove(&self, quad: &EncodedQuad) -> Result<bool, Self::Error> {
        self.remove(quad)
    }
}
