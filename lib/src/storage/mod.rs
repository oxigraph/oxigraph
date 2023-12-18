#![allow(clippy::same_name_method)]
use crate::model::vocab::rdf;
#[cfg(not(target_family = "wasm"))]
use crate::model::Quad;
use crate::model::{GraphNameRef, NamedNodeRef, NamedOrBlankNodeRef, QuadRef, Term, TermRef};
use crate::storage::backend::{Reader, Transaction};
#[cfg(not(target_family = "wasm"))]
use crate::storage::binary_encoder::LATEST_STORAGE_VERSION;
use crate::storage::binary_encoder::{
    decode_term, encode_term, encode_term_pair, encode_term_quad, encode_term_triple,
    write_gosp_quad, write_gpos_quad, write_gspo_quad, write_osp_quad, write_ospg_quad,
    write_pos_quad, write_posg_quad, write_spo_quad, write_spog_quad, write_term, QuadEncoding,
    WRITTEN_TERM_MAX_SIZE,
};
pub use crate::storage::error::{CorruptionError, LoaderError, SerializerError, StorageError};
#[cfg(not(target_family = "wasm"))]
use crate::storage::numeric_encoder::Decoder;
use crate::storage::numeric_encoder::{insert_term, EncodedQuad, EncodedTerm, StrHash, StrLookup};
use crate::storage::vg_vocab::{faldo, vg};
use backend::{ColumnFamily, ColumnFamilyDefinition, Db, Iter};
use gfa::gfa::Orientation;
use gfa::parser::GFAParser;
use handlegraph::handle::{Direction, Handle};
use handlegraph::{
    conversion::from_gfa, handlegraph::IntoHandles, handlegraph::IntoNeighbors,
    handlegraph::IntoSequences, packedgraph::PackedGraph,
};
use oxrdf::{Literal, NamedNode};
use std::str;

#[cfg(not(target_family = "wasm"))]
use std::collections::VecDeque;
#[cfg(not(target_family = "wasm"))]
use std::collections::{HashMap, HashSet};
use std::error::Error;
#[cfg(not(target_family = "wasm"))]
use std::mem::{swap, take};
#[cfg(not(target_family = "wasm"))]
use std::path::{Path, PathBuf};
#[cfg(not(target_family = "wasm"))]
use std::sync::atomic::{AtomicU64, Ordering};
#[cfg(not(target_family = "wasm"))]
use std::sync::Arc;
#[cfg(not(target_family = "wasm"))]
use std::thread::spawn;
#[cfg(not(target_family = "wasm"))]
use std::thread::JoinHandle;

mod backend;
mod binary_encoder;
mod error;
pub mod numeric_encoder;
pub mod small_string;
mod vg_vocab;

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
#[cfg(not(target_family = "wasm"))]
const DEFAULT_CF: &str = "default";
#[cfg(not(target_family = "wasm"))]
const DEFAULT_BULK_LOAD_BATCH_SIZE: usize = 1_000_000;

/// Low level storage primitives
#[derive(Clone)]
pub struct Storage {
    graph: PackedGraph,
    base: String,
}

impl Storage {
    pub fn new() -> Result<Self, StorageError> {
        Ok(Self {
            graph: PackedGraph::new(),
            base: "https://example.org".to_owned(),
        })
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn open(path: &Path) -> Result<Self, StorageError> {
        let gfa_parser = GFAParser::new();
        let gfa = gfa_parser
            .parse_file(path)
            .map_err(|err| StorageError::Other(Box::new(err)))?;
        let graph = from_gfa::<PackedGraph, ()>(&gfa);
        Ok(Self {
            graph,
            base: "https://example.org".to_owned(),
        })
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn open_secondary(primary_path: &Path) -> Result<Self, StorageError> {
        let gfa_parser = GFAParser::new();
        let gfa = gfa_parser
            .parse_file(primary_path)
            .map_err(|err| StorageError::Other(Box::new(err)))?;
        let graph = from_gfa::<PackedGraph, ()>(&gfa);
        Ok(Self {
            graph,
            base: "https://example.org".to_owned(),
        })
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn open_persistent_secondary(
        primary_path: &Path,
        secondary_path: &Path,
    ) -> Result<Self, StorageError> {
        let gfa_parser = GFAParser::new();
        let gfa = gfa_parser
            .parse_file(primary_path)
            .map_err(|err| StorageError::Other(Box::new(err)))?;
        let graph = from_gfa::<PackedGraph, ()>(&gfa);
        Ok(Self {
            graph,
            base: "https://example.org".to_owned(),
        })
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn open_read_only(path: &Path) -> Result<Self, StorageError> {
        let gfa_parser = GFAParser::new();
        let gfa = gfa_parser
            .parse_file(path)
            .map_err(|err| StorageError::Other(Box::new(err)))?;
        let graph = from_gfa::<PackedGraph, ()>(&gfa);
        Ok(Self {
            graph,
            base: "https://example.org".to_owned(),
        })
    }

    pub fn snapshot(&self) -> StorageReader {
        StorageReader {
            // reader: self.db.snapshot(),
            storage: self.clone(),
        }
    }

    // pub fn transaction<'a, 'b: 'a, T, E: Error + 'static + From<StorageError>>(
    //     &'b self,
    //     f: impl Fn(StorageWriter<'a>) -> Result<T, E>,
    // ) -> Result<T, E> {
    //     // self.db.transaction(|transaction| {
    //     //     f(StorageWriter {
    //     //         buffer: Vec::new(),
    //     //         transaction,
    //     //         storage: self,
    //     //     })
    //     // })
    //     Err(StorageError::Io(std::io::Error::new(
    //         std::io::ErrorKind::Unsupported,
    //         "Transactions are currently not supported",
    //     )))
    // }
    #[cfg(not(target_family = "wasm"))]
    pub fn flush(&self) -> Result<(), StorageError> {
        Ok(())
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn compact(&self) -> Result<(), StorageError> {
        Ok(())
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn backup(&self, target_directory: &Path) -> Result<(), StorageError> {
        Ok(())
    }
}

pub struct StorageReader {
    // reader: Reader,
    storage: Storage,
}

impl StorageReader {
    pub fn len(&self) -> Result<usize, StorageError> {
        // Ok(self.reader.len(&self.storage.gspo_cf)? + self.reader.len(&self.storage.dspo_cf)?)
        Ok(0)
    }

    pub fn is_empty(&self) -> Result<bool, StorageError> {
        // Ok(self.reader.is_empty(&self.storage.gspo_cf)?
        // && self.reader.is_empty(&self.storage.dspo_cf)?)
        Ok(true)
    }

    pub fn contains(&self, quad: &EncodedQuad) -> Result<bool, StorageError> {
        // let mut buffer = Vec::with_capacity(4 * WRITTEN_TERM_MAX_SIZE);
        // if quad.graph_name.is_default_graph() {
        //     write_spo_quad(&mut buffer, quad);
        //     Ok(self.reader.contains_key(&self.storage.dspo_cf, &buffer)?)
        // } else {
        //     write_gspo_quad(&mut buffer, quad);
        //     Ok(self.reader.contains_key(&self.storage.gspo_cf, &buffer)?)
        // }
        Ok(true)
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: Option<&EncodedTerm>,
    ) -> ChainedDecodingQuadIterator {
        println!("Receiving quads_for_pattern");
        // let sub = subject.map(|s| self.decode_term(s).ok()).flatten();
        // let pre = predicate.map(|s| self.decode_term(s).ok()).flatten();
        let graph_name = graph_name.expect("Graph name is given");
        // let obj = object.map(|s| self.decode_term(s).ok()).flatten();
        if subject.is_some_and(|s| s.is_blank_node()) || object.is_some_and(|o| o.is_blank_node()) {
            println!("Containing blank nodes");
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms: Vec::new(),
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };
        }

        if self.is_vocab(predicate, rdf::TYPE) && object.is_some() {
            //TODO
            println!("Containing type predicate");
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms: Vec::new(),
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };
        } else if self.is_node_related(predicate) {
            println!("Containing node-related predicate");
            let terms = self.nodes(subject, predicate, object, graph_name);
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms,
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };
        }
        return ChainedDecodingQuadIterator {
            first: DecodingQuadIterator {
                terms: Vec::new(),
                encoding: QuadEncoding::Spog,
            },
            second: None,
        };
    }

    fn nodes(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let mut results = Vec::new();
        match subject {
            Some(sub) => {
                println!("Real subject: {}", sub.get_named_node_value().unwrap());
                let is_node_iri = self.is_node_iri_in_graph(sub);
                if self.is_vocab(predicate, rdf::TYPE)
                    && self.is_vocab(object, vg::NODE)
                    && is_node_iri
                {
                    println!("First");
                    results.push(EncodedQuad::new(
                        sub.to_owned(),
                        rdf::TYPE.into(),
                        vg::NODE.into(),
                        graph_name.to_owned(),
                    ));
                } else if predicate.is_none() && self.is_vocab(object, vg::NODE) && is_node_iri {
                    println!("Second");
                    results.push(EncodedQuad::new(
                        sub.to_owned(),
                        rdf::TYPE.into(),
                        vg::NODE.into(),
                        graph_name.to_owned(),
                    ));
                } else if predicate.is_none() && is_node_iri {
                    println!("Third");
                    results.push(EncodedQuad::new(
                        sub.to_owned(),
                        rdf::TYPE.into(),
                        vg::NODE.into(),
                        graph_name.to_owned(),
                    ));
                }

                if is_node_iri {
                    println!("Fourth");
                    let mut triples = self.handle_to_triples(sub, predicate, object, graph_name);
                    let mut edge_triples =
                        self.handle_to_edge_triples(sub, predicate, object, graph_name);
                    println!("Normal: {:?}", triples);
                    println!("Edge: {:?}", edge_triples);
                    results.append(&mut triples);
                    results.append(&mut edge_triples);
                }
            }
            None => {
                println!("None subject");
                for handle in self.storage.graph.handles() {
                    println!("{:?}", handle);
                    let term = self
                        .handle_to_namednode(handle)
                        .expect("Can turn handle to namednode");
                    let mut recursion_results =
                        self.nodes(Some(&term), predicate, object, graph_name);
                    println!("{:?}", recursion_results);
                    println!("---------------------------");
                    results.append(&mut recursion_results);
                }
                // println!("{:?}", results);
            }
        }
        println!("Nodes successfully done!");
        results
    }

    fn handle_to_triples(
        &self,
        subject: &EncodedTerm,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let mut results = Vec::new();
        if self.is_vocab(predicate, rdf::VALUE) || predicate.is_none() {
            let handle = Handle::new(
                self.get_node_id(subject).expect("Subject is node"),
                Orientation::Forward,
            );
            let seq_bytes = self.storage.graph.sequence_vec(handle);
            let seq = str::from_utf8(&seq_bytes).expect("Node contains sequence");
            let seq_value = Literal::new_simple_literal(seq);
            println!("Decoding 338");
            if object.is_none()
                || self.decode_term(object.unwrap()).unwrap() == Term::Literal(seq_value.clone())
            {
                results.push(EncodedQuad::new(
                    subject.to_owned(),
                    rdf::VALUE.into(),
                    seq_value.as_ref().into(),
                    graph_name.to_owned(),
                ));
            }
            println!("Done decoding 338");
        } else if (self.is_vocab(predicate, rdf::TYPE) || predicate.is_none())
            && (object.is_none() || self.is_vocab(object, vg::NODE))
        {
            results.push(EncodedQuad::new(
                subject.to_owned(),
                rdf::TYPE.into(),
                vg::NODE.into(),
                graph_name.to_owned(),
            ));
        }
        results
    }

    fn handle_to_edge_triples(
        &self,
        subject: &EncodedTerm,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let mut results = Vec::new();
        print!("Subject: {:?}, ", subject);
        if predicate.is_none() || self.is_node_related(predicate) {
            let handle = Handle::new(
                self.get_node_id(subject).expect("Subject has node id"),
                Orientation::Forward,
            );
            println!("Handle: {:?}", handle);
            let neighbors = self.storage.graph.neighbors(handle, Direction::Right);
            for neighbor in neighbors {
                if object.is_none()
                    || self
                        .get_node_id(object.unwrap())
                        .expect("Object has node id")
                        == neighbor.unpack_number()
                {
                    let mut edge_triples =
                        self.generate_edge_triples(handle, neighbor, predicate, graph_name);
                    results.append(&mut edge_triples);
                }
            }
        }
        results
    }

    fn generate_edge_triples(
        &self,
        subject: Handle,
        object: Handle,
        predicate: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let mut results = Vec::new();
        let node_is_reverse = subject.is_reverse();
        let other_is_reverse = object.is_reverse();
        if (predicate.is_none() || self.is_vocab(predicate, vg::LINKS_FORWARD_TO_FORWARD))
            && !node_is_reverse
            && !other_is_reverse
        {
            results.push(EncodedQuad::new(
                self.handle_to_namednode(subject).expect("Subject is fine"),
                vg::LINKS_FORWARD_TO_FORWARD.into(),
                self.handle_to_namednode(object).expect("Object is fine"),
                graph_name.to_owned(),
            ));
        }
        if (predicate.is_none() || self.is_vocab(predicate, vg::LINKS_FORWARD_TO_REVERSE))
            && !node_is_reverse
            && other_is_reverse
        {
            results.push(EncodedQuad::new(
                self.handle_to_namednode(subject).expect("Subject is fine"),
                vg::LINKS_FORWARD_TO_REVERSE.into(),
                self.handle_to_namednode(object).expect("Object is fine"),
                graph_name.to_owned(),
            ));
        }
        if (predicate.is_none() || self.is_vocab(predicate, vg::LINKS_REVERSE_TO_FORWARD))
            && node_is_reverse
            && !other_is_reverse
        {
            results.push(EncodedQuad::new(
                self.handle_to_namednode(subject).expect("Subject is fine"),
                vg::LINKS_REVERSE_TO_FORWARD.into(),
                self.handle_to_namednode(object).expect("Object is fine"),
                graph_name.to_owned(),
            ));
        }
        if (predicate.is_none() || self.is_vocab(predicate, vg::LINKS_REVERSE_TO_REVERSE))
            && node_is_reverse
            && other_is_reverse
        {
            results.push(EncodedQuad::new(
                self.handle_to_namednode(subject).expect("Subject is fine"),
                vg::LINKS_REVERSE_TO_REVERSE.into(),
                self.handle_to_namednode(object).expect("Object is fine"),
                graph_name.to_owned(),
            ));
        }
        if predicate.is_none() || self.is_vocab(predicate, vg::LINKS) {
            results.push(EncodedQuad::new(
                self.handle_to_namednode(subject).expect("Subject is fine"),
                vg::LINKS.into(),
                self.handle_to_namednode(object).expect("Object is fine"),
                graph_name.to_owned(),
            ));
        }
        results
    }

    fn handle_to_namednode(&self, handle: Handle) -> Option<EncodedTerm> {
        let id = handle.unpack_number();
        let text = format!("{}/node/{}", self.storage.base, id);
        let named_node = NamedNode::new(text).unwrap();
        Some(named_node.as_ref().into())
    }

    fn is_node_related(&self, predicate: Option<&EncodedTerm>) -> bool {
        let predicates = [
            vg::LINKS,
            vg::LINKS_FORWARD_TO_FORWARD,
            vg::LINKS_FORWARD_TO_REVERSE,
            vg::LINKS_REVERSE_TO_FORWARD,
            vg::LINKS_REVERSE_TO_REVERSE,
        ];
        if predicate.is_none() {
            return false;
        }
        predicates
            .into_iter()
            .map(|x| self.is_vocab(predicate, x))
            .reduce(|acc, x| acc || x)
            .unwrap()
    }

    fn is_vocab(&self, term: Option<&EncodedTerm>, vocab: NamedNodeRef) -> bool {
        if term.is_none() {
            return false;
        }
        let term = term.unwrap();
        if !term.is_named_node() {
            return false;
        }
        let named_node = term.get_named_node_value().expect("Is named node");
        named_node == vocab.as_str()
    }

    fn is_node_iri_in_graph(&self, term: &EncodedTerm) -> bool {
        match self.get_node_id(term) {
            Some(id) => self.storage.graph.has_node(id),
            None => false,
        }
    }

    fn get_node_id(&self, term: &EncodedTerm) -> Option<u64> {
        match term.is_named_node() {
            true => {
                let mut text = term
                    .get_named_node_value()
                    .expect("Encoded NamedNode has to have value")
                    .to_owned();

                // Remove trailing '>'
                println!("Text: {}", text);
                // text.pop();

                let mut parts_iter = text.rsplit("/");
                let last = parts_iter.next();
                let pre_last = parts_iter.next();
                match last.is_some()
                    && pre_last.is_some()
                    && pre_last.expect("Option is some") == "node"
                {
                    true => last.expect("Option is some").parse::<u64>().ok(),
                    false => None,
                }
            }
            false => None,
        }
    }

    pub fn quads(&self) -> ChainedDecodingQuadIterator {
        ChainedDecodingQuadIterator::new(DecodingQuadIterator {
            terms: Vec::new(),
            encoding: QuadEncoding::Spog,
        })
        // ChainedDecodingQuadIterator::pair(self.dspo_quads(&[]), self.gspo_quads(&[]))
    }

    // fn quads_in_named_graph(&self) -> DecodingQuadIterator {
    //     self.gspo_quads(&[])
    // }

    // fn quads_for_subject(&self, subject: &EncodedTerm) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::pair(
    //         self.dspo_quads(&encode_term(subject)),
    //         self.spog_quads(&encode_term(subject)),
    //     )
    // }

    // fn quads_for_subject_predicate(
    //     &self,
    //     subject: &EncodedTerm,
    //     predicate: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::pair(
    //         self.dspo_quads(&encode_term_pair(subject, predicate)),
    //         self.spog_quads(&encode_term_pair(subject, predicate)),
    //     )
    // }

    // fn quads_for_subject_predicate_object(
    //     &self,
    //     subject: &EncodedTerm,
    //     predicate: &EncodedTerm,
    //     object: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::pair(
    //         self.dspo_quads(&encode_term_triple(subject, predicate, object)),
    //         self.spog_quads(&encode_term_triple(subject, predicate, object)),
    //     )
    // }

    // fn quads_for_subject_object(
    //     &self,
    //     subject: &EncodedTerm,
    //     object: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::pair(
    //         self.dosp_quads(&encode_term_pair(object, subject)),
    //         self.ospg_quads(&encode_term_pair(object, subject)),
    //     )
    // }

    // fn quads_for_predicate(&self, predicate: &EncodedTerm) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::pair(
    //         self.dpos_quads(&encode_term(predicate)),
    //         self.posg_quads(&encode_term(predicate)),
    //     )
    // }

    // fn quads_for_predicate_object(
    //     &self,
    //     predicate: &EncodedTerm,
    //     object: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::pair(
    //         self.dpos_quads(&encode_term_pair(predicate, object)),
    //         self.posg_quads(&encode_term_pair(predicate, object)),
    //     )
    // }

    // fn quads_for_object(&self, object: &EncodedTerm) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::pair(
    //         self.dosp_quads(&encode_term(object)),
    //         self.ospg_quads(&encode_term(object)),
    //     )
    // }

    // fn quads_for_graph(&self, graph_name: &EncodedTerm) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
    //         self.dspo_quads(&Vec::default())
    //     } else {
    //         self.gspo_quads(&encode_term(graph_name))
    //     })
    // }

    // fn quads_for_subject_graph(
    //     &self,
    //     subject: &EncodedTerm,
    //     graph_name: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
    //         self.dspo_quads(&encode_term(subject))
    //     } else {
    //         self.gspo_quads(&encode_term_pair(graph_name, subject))
    //     })
    // }

    // fn quads_for_subject_predicate_graph(
    //     &self,
    //     subject: &EncodedTerm,
    //     predicate: &EncodedTerm,
    //     graph_name: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
    //         self.dspo_quads(&encode_term_pair(subject, predicate))
    //     } else {
    //         self.gspo_quads(&encode_term_triple(graph_name, subject, predicate))
    //     })
    // }

    // fn quads_for_subject_predicate_object_graph(
    //     &self,
    //     subject: &EncodedTerm,
    //     predicate: &EncodedTerm,
    //     object: &EncodedTerm,
    //     graph_name: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
    //         self.dspo_quads(&encode_term_triple(subject, predicate, object))
    //     } else {
    //         self.gspo_quads(&encode_term_quad(graph_name, subject, predicate, object))
    //     })
    // }

    // fn quads_for_subject_object_graph(
    //     &self,
    //     subject: &EncodedTerm,
    //     object: &EncodedTerm,
    //     graph_name: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
    //         self.dosp_quads(&encode_term_pair(object, subject))
    //     } else {
    //         self.gosp_quads(&encode_term_triple(graph_name, object, subject))
    //     })
    // }

    // fn quads_for_predicate_graph(
    //     &self,
    //     predicate: &EncodedTerm,
    //     graph_name: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
    //         self.dpos_quads(&encode_term(predicate))
    //     } else {
    //         self.gpos_quads(&encode_term_pair(graph_name, predicate))
    //     })
    // }

    // fn quads_for_predicate_object_graph(
    //     &self,
    //     predicate: &EncodedTerm,
    //     object: &EncodedTerm,
    //     graph_name: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
    //         self.dpos_quads(&encode_term_pair(predicate, object))
    //     } else {
    //         self.gpos_quads(&encode_term_triple(graph_name, predicate, object))
    //     })
    // }

    // fn quads_for_object_graph(
    //     &self,
    //     object: &EncodedTerm,
    //     graph_name: &EncodedTerm,
    // ) -> ChainedDecodingQuadIterator {
    //     ChainedDecodingQuadIterator::new(if graph_name.is_default_graph() {
    //         self.dosp_quads(&encode_term(object))
    //     } else {
    //         self.gosp_quads(&encode_term_pair(graph_name, object))
    //     })
    // }

    pub fn named_graphs(&self) -> DecodingGraphIterator {
        DecodingGraphIterator { terms: Vec::new() }
    }

    pub fn contains_named_graph(&self, graph_name: &EncodedTerm) -> Result<bool, StorageError> {
        // self.reader
        // .contains_key(&self.storage.graphs_cf, &encode_term(graph_name))
        Ok(true)
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(None)
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
        Ok(true)
    }

    /// Validates that all the storage invariants held in the data
    #[cfg(not(target_family = "wasm"))]
    pub fn validate(&self) -> Result<(), StorageError> {
        Ok(())
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
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Result<EncodedQuad, StorageError>> {
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
    terms: Vec<EncodedQuad>,
    encoding: QuadEncoding,
}

impl Iterator for DecodingQuadIterator {
    type Item = Result<EncodedQuad, StorageError>;

    fn next(&mut self) -> Option<Result<EncodedQuad, StorageError>> {
        // if let Err(e) = self.iter.status() {
        //     return Some(Err(e));
        // }
        // let term = self.encoding.decode(self.iter.key()?);
        // self.iter.next();
        self.terms.pop().map(|x| Ok(x))
    }
}

pub struct DecodingGraphIterator {
    terms: Vec<EncodedTerm>,
}

impl Iterator for DecodingGraphIterator {
    type Item = Result<EncodedTerm, StorageError>;

    fn next(&mut self) -> Option<Result<EncodedTerm, StorageError>> {
        // if let Err(e) = self.iter.status() {
        //     return Some(Err(e));
        // }
        // let term = self.encoding.decode(self.iter.key()?);
        // self.iter.next();
        self.terms.pop().map(|x| Ok(x))
    }
}

impl StrLookup for StorageReader {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        self.get_str(key)
    }

    fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
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
            // reader: self.transaction.reader(),
            storage: self.storage.clone(),
        }
    }

    pub fn insert(&mut self, quad: QuadRef<'_>) -> Result<bool, StorageError> {
        Ok(true)
        //     let encoded = quad.into();
        //     self.buffer.clear();
        //     let result = if quad.graph_name.is_default_graph() {
        //         write_spo_quad(&mut self.buffer, &encoded);
        //         if self
        //             .transaction
        //             .contains_key_for_update(&self.storage.dspo_cf, &self.buffer)?
        //         {
        //             false
        //         } else {
        //             self.transaction
        //                 .insert_empty(&self.storage.dspo_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_pos_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.dpos_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_osp_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.dosp_cf, &self.buffer)?;

        //             self.insert_term(quad.subject.into(), &encoded.subject)?;
        //             self.insert_term(quad.predicate.into(), &encoded.predicate)?;
        //             self.insert_term(quad.object, &encoded.object)?;
        //             true
        //         }
        //     } else {
        //         write_spog_quad(&mut self.buffer, &encoded);
        //         if self
        //             .transaction
        //             .contains_key_for_update(&self.storage.spog_cf, &self.buffer)?
        //         {
        //             false
        //         } else {
        //             self.transaction
        //                 .insert_empty(&self.storage.spog_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_posg_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.posg_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_ospg_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.ospg_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_gspo_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.gspo_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_gpos_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.gpos_cf, &self.buffer)?;

        //             self.buffer.clear();
        //             write_gosp_quad(&mut self.buffer, &encoded);
        //             self.transaction
        //                 .insert_empty(&self.storage.gosp_cf, &self.buffer)?;

        //             self.insert_term(quad.subject.into(), &encoded.subject)?;
        //             self.insert_term(quad.predicate.into(), &encoded.predicate)?;
        //             self.insert_term(quad.object, &encoded.object)?;

        //             self.buffer.clear();
        //             write_term(&mut self.buffer, &encoded.graph_name);
        //             if !self
        //                 .transaction
        //                 .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
        //             {
        //                 self.transaction
        //                     .insert_empty(&self.storage.graphs_cf, &self.buffer)?;
        //                 self.insert_graph_name(quad.graph_name, &encoded.graph_name)?;
        //             }
        //             true
        //         }
        //     };
        //     Ok(result)
    }

    pub fn insert_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, StorageError> {
        Ok(true)
        //     let encoded_graph_name = graph_name.into();

        //     self.buffer.clear();
        //     write_term(&mut self.buffer, &encoded_graph_name);
        //     let result = if self
        //         .transaction
        //         .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
        //     {
        //         false
        //     } else {
        //         self.transaction
        //             .insert_empty(&self.storage.graphs_cf, &self.buffer)?;
        //         self.insert_term(graph_name.into(), &encoded_graph_name)?;
        //         true
        //     };
        //     Ok(result)
    }

    // fn insert_term(
    //     &mut self,
    //     term: TermRef<'_>,
    //     encoded: &EncodedTerm,
    // ) -> Result<(), StorageError> {
    //     insert_term(term, encoded, &mut |key, value| self.insert_str(key, value))
    // }

    // fn insert_graph_name(
    //     &mut self,
    //     graph_name: GraphNameRef<'_>,
    //     encoded: &EncodedTerm,
    // ) -> Result<(), StorageError> {
    //     match graph_name {
    //         GraphNameRef::NamedNode(graph_name) => self.insert_term(graph_name.into(), encoded),
    //         GraphNameRef::BlankNode(graph_name) => self.insert_term(graph_name.into(), encoded),
    //         GraphNameRef::DefaultGraph => Ok(()),
    //     }
    // }

    // #[cfg(not(target_family = "wasm"))]
    // fn insert_str(&mut self, key: &StrHash, value: &str) -> Result<(), StorageError> {
    //     if self
    //         .storage
    //         .db
    //         .contains_key(&self.storage.id2str_cf, &key.to_be_bytes())?
    //     {
    //         return Ok(());
    //     }
    //     self.storage.db.insert(
    //         &self.storage.id2str_cf,
    //         &key.to_be_bytes(),
    //         value.as_bytes(),
    //     )
    // }

    // #[cfg(target_family = "wasm")]
    // fn insert_str(&mut self, key: &StrHash, value: &str) -> Result<(), StorageError> {
    //     self.transaction.insert(
    //         &self.storage.id2str_cf,
    //         &key.to_be_bytes(),
    //         value.as_bytes(),
    //     )
    // }

    pub fn remove(&mut self, quad: QuadRef<'_>) -> Result<bool, StorageError> {
        // self.remove_encoded(&quad.into())
        Ok(true)
    }

    // fn remove_encoded(&mut self, quad: &EncodedQuad) -> Result<bool, StorageError> {
    //     self.buffer.clear();
    //     let result = if quad.graph_name.is_default_graph() {
    //         write_spo_quad(&mut self.buffer, quad);

    //         if self
    //             .transaction
    //             .contains_key_for_update(&self.storage.dspo_cf, &self.buffer)?
    //         {
    //             self.transaction
    //                 .remove(&self.storage.dspo_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_pos_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.dpos_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_osp_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.dosp_cf, &self.buffer)?;
    //             true
    //         } else {
    //             false
    //         }
    //     } else {
    //         write_spog_quad(&mut self.buffer, quad);

    //         if self
    //             .transaction
    //             .contains_key_for_update(&self.storage.spog_cf, &self.buffer)?
    //         {
    //             self.transaction
    //                 .remove(&self.storage.spog_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_posg_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.posg_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_ospg_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.ospg_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_gspo_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.gspo_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_gpos_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.gpos_cf, &self.buffer)?;

    //             self.buffer.clear();
    //             write_gosp_quad(&mut self.buffer, quad);
    //             self.transaction
    //                 .remove(&self.storage.gosp_cf, &self.buffer)?;
    //             true
    //         } else {
    //             false
    //         }
    //     };
    //     Ok(result)
    // }

    pub fn clear_graph(&mut self, graph_name: GraphNameRef<'_>) -> Result<(), StorageError> {
        // if graph_name.is_default_graph() {
        //     for quad in self.reader().quads_for_graph(&EncodedTerm::DefaultGraph) {
        //         self.remove_encoded(&quad?)?;
        //     }
        // } else {
        //     self.buffer.clear();
        //     write_term(&mut self.buffer, &graph_name.into());
        //     if self
        //         .transaction
        //         .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
        //     {
        //         // The condition is useful to lock the graph itself and ensure no quad is inserted at the same time
        //         for quad in self.reader().quads_for_graph(&graph_name.into()) {
        //             self.remove_encoded(&quad?)?;
        //         }
        //     }
        // }
        Ok(())
    }

    pub fn clear_all_named_graphs(&mut self) -> Result<(), StorageError> {
        // for quad in self.reader().quads_in_named_graph() {
        //     self.remove_encoded(&quad?)?;
        // }
        Ok(())
    }

    pub fn clear_all_graphs(&mut self) -> Result<(), StorageError> {
        // for quad in self.reader().quads() {
        //     self.remove_encoded(&quad?)?;
        // }
        Ok(())
    }

    pub fn remove_named_graph(
        &mut self,
        graph_name: NamedOrBlankNodeRef<'_>,
    ) -> Result<bool, StorageError> {
        // self.remove_encoded_named_graph(&graph_name.into())
        Ok(true)
    }

    // fn remove_encoded_named_graph(
    //     &mut self,
    //     graph_name: &EncodedTerm,
    // ) -> Result<bool, StorageError> {
    //     self.buffer.clear();
    //     write_term(&mut self.buffer, graph_name);
    //     let result = if self
    //         .transaction
    //         .contains_key_for_update(&self.storage.graphs_cf, &self.buffer)?
    //     {
    //         // The condition is done ASAP to lock the graph itself
    //         for quad in self.reader().quads_for_graph(graph_name) {
    //             self.remove_encoded(&quad?)?;
    //         }
    //         self.buffer.clear();
    //         write_term(&mut self.buffer, graph_name);
    //         self.transaction
    //             .remove(&self.storage.graphs_cf, &self.buffer)?;
    //         true
    //     } else {
    //         false
    //     };
    //     Ok(result)
    // }

    pub fn remove_all_named_graphs(&mut self) -> Result<(), StorageError> {
        // for graph_name in self.reader().named_graphs() {
        //     self.remove_encoded_named_graph(&graph_name?)?;
        // }
        Ok(())
    }

    pub fn clear(&mut self) -> Result<(), StorageError> {
        // for graph_name in self.reader().named_graphs() {
        //     self.remove_encoded_named_graph(&graph_name?)?;
        // }
        // for quad in self.reader().quads() {
        //     self.remove_encoded(&quad?)?;
        // }
        Ok(())
    }
}

#[cfg(not(target_family = "wasm"))]
pub struct StorageBulkLoader {
    storage: Storage,
    hooks: Vec<Box<dyn Fn(u64)>>,
    num_threads: Option<usize>,
    max_memory_size: Option<usize>,
}

#[cfg(not(target_family = "wasm"))]
impl StorageBulkLoader {
    pub fn new(storage: Storage) -> Self {
        Self {
            storage,
            hooks: Vec::new(),
            num_threads: None,
            max_memory_size: None,
        }
    }

    pub fn set_num_threads(mut self, num_threads: usize) -> Self {
        self.num_threads = Some(num_threads);
        self
    }

    pub fn set_max_memory_size_in_megabytes(mut self, max_memory_size: usize) -> Self {
        self.max_memory_size = Some(max_memory_size);
        self
    }

    pub fn on_progress(mut self, callback: impl Fn(u64) + 'static) -> Self {
        self.hooks.push(Box::new(callback));
        self
    }

    #[allow(clippy::trait_duplication_in_bounds)]
    pub fn load<EI, EO: From<StorageError> + From<EI>>(
        &self,
        quads: impl IntoIterator<Item = Result<Quad, EI>>,
    ) -> Result<(), EO> {
        let num_threads = self.num_threads.unwrap_or(2);
        if num_threads < 2 {
            return Err(
                StorageError::Other("The bulk loader needs at least 2 threads".into()).into(),
            );
        }
        let batch_size = if let Some(max_memory_size) = self.max_memory_size {
            max_memory_size * 1000 / num_threads
        } else {
            DEFAULT_BULK_LOAD_BATCH_SIZE
        };
        if batch_size < 10_000 {
            return Err(StorageError::Other(
                "The bulk loader memory bound is too low. It needs at least 100MB".into(),
            )
            .into());
        }
        let mut threads = VecDeque::with_capacity(num_threads - 1);
        let mut buffer = Vec::with_capacity(batch_size);
        let done_counter = Arc::new(AtomicU64::new(0));
        let mut done_and_displayed_counter = 0;
        for quad in quads {
            let quad = quad?;
            buffer.push(quad);
            if buffer.len() >= batch_size {
                self.spawn_load_thread(
                    &mut buffer,
                    &mut threads,
                    &done_counter,
                    &mut done_and_displayed_counter,
                    num_threads,
                    batch_size,
                )?;
            }
        }
        self.spawn_load_thread(
            &mut buffer,
            &mut threads,
            &done_counter,
            &mut done_and_displayed_counter,
            num_threads,
            batch_size,
        )?;
        for thread in threads {
            thread.join().unwrap()?;
            self.on_possible_progress(&done_counter, &mut done_and_displayed_counter);
        }
        Ok(())
    }

    fn spawn_load_thread(
        &self,
        buffer: &mut Vec<Quad>,
        threads: &mut VecDeque<JoinHandle<Result<(), StorageError>>>,
        done_counter: &Arc<AtomicU64>,
        done_and_displayed_counter: &mut u64,
        num_threads: usize,
        batch_size: usize,
    ) -> Result<(), StorageError> {
        self.on_possible_progress(done_counter, done_and_displayed_counter);
        // We avoid to have too many threads
        if threads.len() >= num_threads {
            if let Some(thread) = threads.pop_front() {
                thread.join().unwrap()?;
                self.on_possible_progress(done_counter, done_and_displayed_counter);
            }
        }
        let mut buffer_to_load = Vec::with_capacity(batch_size);
        swap(buffer, &mut buffer_to_load);
        let storage = self.storage.clone();
        let done_counter_clone = Arc::clone(done_counter);
        threads.push_back(spawn(move || {
            FileBulkLoader::new(storage, batch_size).load(buffer_to_load, &done_counter_clone)
        }));
        Ok(())
    }

    fn on_possible_progress(&self, done: &AtomicU64, done_and_displayed: &mut u64) {
        let new_counter = done.load(Ordering::Relaxed);
        let display_step = u64::try_from(DEFAULT_BULK_LOAD_BATCH_SIZE).unwrap();
        if new_counter / display_step > *done_and_displayed / display_step {
            for hook in &self.hooks {
                hook(new_counter);
            }
        }
        *done_and_displayed = new_counter;
    }
}

#[cfg(not(target_family = "wasm"))]
struct FileBulkLoader {
    storage: Storage,
    id2str: HashMap<StrHash, Box<str>>,
    quads: HashSet<EncodedQuad>,
    triples: HashSet<EncodedQuad>,
    graphs: HashSet<EncodedTerm>,
}

#[cfg(not(target_family = "wasm"))]
impl FileBulkLoader {
    fn new(storage: Storage, batch_size: usize) -> Self {
        Self {
            storage,
            id2str: HashMap::with_capacity(3 * batch_size),
            quads: HashSet::with_capacity(batch_size),
            triples: HashSet::with_capacity(batch_size),
            graphs: HashSet::default(),
        }
    }

    fn load(&mut self, quads: Vec<Quad>, counter: &AtomicU64) -> Result<(), StorageError> {
        self.encode(quads)?;
        let size = self.triples.len() + self.quads.len();
        // self.save()?;
        counter.fetch_add(size.try_into().unwrap(), Ordering::Relaxed);
        Ok(())
    }

    fn encode(&mut self, quads: Vec<Quad>) -> Result<(), StorageError> {
        for quad in quads {
            let encoded = EncodedQuad::from(quad.as_ref());
            if quad.graph_name.is_default_graph() {
                if self.triples.insert(encoded.clone()) {
                    self.insert_term(quad.subject.as_ref().into(), &encoded.subject)?;
                    self.insert_term(quad.predicate.as_ref().into(), &encoded.predicate)?;
                    self.insert_term(quad.object.as_ref(), &encoded.object)?;
                }
            } else if self.quads.insert(encoded.clone()) {
                self.insert_term(quad.subject.as_ref().into(), &encoded.subject)?;
                self.insert_term(quad.predicate.as_ref().into(), &encoded.predicate)?;
                self.insert_term(quad.object.as_ref(), &encoded.object)?;

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
        Ok(())
    }

    // fn save(&mut self) -> Result<(), StorageError> {
    //     let mut to_load = Vec::new();

    //     // id2str
    //     if !self.id2str.is_empty() {
    //         let mut id2str = take(&mut self.id2str)
    //             .into_iter()
    //             .map(|(k, v)| (k.to_be_bytes(), v))
    //             .collect::<Vec<_>>();
    //         id2str.sort_unstable();
    //         let mut id2str_sst = self.storage.db.new_sst_file()?;
    //         for (k, v) in id2str {
    //             id2str_sst.insert(&k, v.as_bytes())?;
    //         }
    //         to_load.push((&self.storage.id2str_cf, id2str_sst.finish()?));
    //     }

    //     if !self.triples.is_empty() {
    //         to_load.push((
    //             &self.storage.dspo_cf,
    //             self.build_sst_for_keys(
    //                 self.triples.iter().map(|quad| {
    //                     encode_term_triple(&quad.subject, &quad.predicate, &quad.object)
    //                 }),
    //             )?,
    //         ));
    //         to_load.push((
    //             &self.storage.dpos_cf,
    //             self.build_sst_for_keys(
    //                 self.triples.iter().map(|quad| {
    //                     encode_term_triple(&quad.predicate, &quad.object, &quad.subject)
    //                 }),
    //             )?,
    //         ));
    //         to_load.push((
    //             &self.storage.dosp_cf,
    //             self.build_sst_for_keys(
    //                 self.triples.iter().map(|quad| {
    //                     encode_term_triple(&quad.object, &quad.subject, &quad.predicate)
    //                 }),
    //             )?,
    //         ));
    //         self.triples.clear();
    //     }

    //     if !self.quads.is_empty() {
    //         to_load.push((
    //             &self.storage.graphs_cf,
    //             self.build_sst_for_keys(self.graphs.iter().map(encode_term))?,
    //         ));
    //         self.graphs.clear();

    //         to_load.push((
    //             &self.storage.gspo_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.graph_name,
    //                     &quad.subject,
    //                     &quad.predicate,
    //                     &quad.object,
    //                 )
    //             }))?,
    //         ));
    //         to_load.push((
    //             &self.storage.gpos_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.graph_name,
    //                     &quad.predicate,
    //                     &quad.object,
    //                     &quad.subject,
    //                 )
    //             }))?,
    //         ));
    //         to_load.push((
    //             &self.storage.gosp_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.graph_name,
    //                     &quad.object,
    //                     &quad.subject,
    //                     &quad.predicate,
    //                 )
    //             }))?,
    //         ));
    //         to_load.push((
    //             &self.storage.spog_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.subject,
    //                     &quad.predicate,
    //                     &quad.object,
    //                     &quad.graph_name,
    //                 )
    //             }))?,
    //         ));
    //         to_load.push((
    //             &self.storage.posg_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.predicate,
    //                     &quad.object,
    //                     &quad.subject,
    //                     &quad.graph_name,
    //                 )
    //             }))?,
    //         ));
    //         to_load.push((
    //             &self.storage.ospg_cf,
    //             self.build_sst_for_keys(self.quads.iter().map(|quad| {
    //                 encode_term_quad(
    //                     &quad.object,
    //                     &quad.subject,
    //                     &quad.predicate,
    //                     &quad.graph_name,
    //                 )
    //             }))?,
    //         ));
    //         self.quads.clear();
    //     }

    //     self.storage.db.insert_stt_files(&to_load)
    // }

    fn insert_term(
        &mut self,
        term: TermRef<'_>,
        encoded: &EncodedTerm,
    ) -> Result<(), StorageError> {
        insert_term(term, encoded, &mut |key, value| {
            self.id2str.entry(*key).or_insert_with(|| value.into());
            Ok(())
        })
    }

    // fn build_sst_for_keys(
    //     &self,
    //     values: impl Iterator<Item = Vec<u8>>,
    // ) -> Result<PathBuf, StorageError> {
    //     let mut values = values.collect::<Vec<_>>();
    //     values.sort_unstable();
    //     let mut sst = self.storage.db.new_sst_file()?;
    //     for value in values {
    //         sst.insert_empty(&value)?;
    //     }
    //     sst.finish()
    // }
}
