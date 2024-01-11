#![allow(clippy::same_name_method)]
use super::numeric_encoder::{StrHash, StrLookup};
use super::{ChainedDecodingQuadIterator, Storage};
use crate::model::vocab::rdf;
use crate::model::{NamedNodeRef, Term};
use crate::storage::binary_encoder::QuadEncoding;
pub use crate::storage::error::{CorruptionError, LoaderError, SerializerError, StorageError};
use crate::storage::numeric_encoder::Decoder;
#[cfg(not(target_family = "wasm"))]
use crate::storage::numeric_encoder::{EncodedQuad, EncodedTerm};
use crate::storage::vg_vocab::{faldo, vg};
use crate::storage::DecodingQuadIterator;
use gfa::gfa::Orientation;
use handlegraph::handle::{Direction, Handle};
use handlegraph::packed::PackedElement;
use handlegraph::packedgraph::paths::StepPtr;
use handlegraph::pathhandlegraph::{path::PathStep, GraphPathsRef, IntoPathIds, PathBase};
use handlegraph::pathhandlegraph::{GraphPathNames, GraphPaths, PathId, PathSequences};
use handlegraph::{
    handlegraph::IntoHandles, handlegraph::IntoNeighbors, handlegraph::IntoSequences,
};
use oxrdf::{Literal, NamedNode};
use std::str;

pub struct StorageGenerator {
    storage: Storage,
}

impl StorageGenerator {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {
        println!("Receiving quads_for_pattern");
        // let sub = subject.map(|s| self.decode_term(s).ok()).flatten();
        // let pre = predicate.map(|s| self.decode_term(s).ok()).flatten();
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
        } else if self.is_step_associated(predicate) {
            println!("Containing node-related predicate");
            let terms = self.steps(subject, predicate, object, graph_name);
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

    fn paths(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let mut results = Vec::new();
        for path_id in self.storage.graph.path_ids() {
            let Some(path_name) = self.storage.graph.get_path_name(path_id) else {
                continue;
            };
            let path_name = path_name.collect::<Vec<_>>();
            let path_name = str::from_utf8(&path_name).unwrap();
            let path_node = self.path_to_namednode(path_name);
            if subject.is_none() || path_node.as_ref() == subject {
                if (predicate.is_none() || self.is_vocab(predicate, rdf::TYPE))
                    && (object.is_none() || self.is_vocab(object, vg::PATH))
                {
                    results.push(EncodedQuad::new(
                        path_node.unwrap(),
                        rdf::TYPE.into(),
                        vg::PATH.into(),
                        graph_name.to_owned(),
                    ));
                }
            }
        }
        results
    }

    fn steps(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let mut results = Vec::new();
        if subject.is_none() {
            for path_id in self.storage.graph.path_ids() {
                if let Some(path_ref) = self.storage.graph.get_path_ref(path_id) {
                    let path_name = self.get_path_name(path_id).unwrap();
                    let mut rank = 1;
                    let mut position = 1;
                    let step_handle = path_ref.step_at(path_ref.first_step());
                    if step_handle.is_none() {
                        continue;
                    }
                    let step_handle = step_handle.unwrap();
                    let node_handle = step_handle.handle();
                    let mut triples = self.step_handle_to_triples(
                        &path_name,
                        subject,
                        predicate,
                        object,
                        graph_name,
                        node_handle,
                        Some(rank),
                        Some(position),
                    );
                    results.append(&mut triples);

                    while path_ref.next_step(step_handle.0).is_some() {
                        let step_handle = path_ref.next_step(step_handle.0).unwrap();
                        position += self.storage.graph.node_len(node_handle);
                        let node_handle = step_handle.handle();
                        rank += 1;
                        let mut triples = self.step_handle_to_triples(
                            &path_name,
                            subject,
                            predicate,
                            object,
                            graph_name,
                            node_handle,
                            Some(rank),
                            Some(position),
                        );
                        results.append(&mut triples);
                    }
                }
            }
        } else if let Some(step_type) = self.get_step_iri_fields(subject) {
            match step_type {
                StepType::Rank(path_name, target_rank) => {
                    if let Some(id) = self.storage.graph.get_path_id(path_name.as_bytes()) {
                        let path_ref = self.storage.graph.get_path_ref(id).unwrap();
                        let step_handle = path_ref.step_at(path_ref.first_step());
                        let step_handle = step_handle.unwrap();
                        let mut node_handle = step_handle.handle();
                        let mut rank = 1;
                        let mut position = 1;

                        while path_ref.next_step(step_handle.0).is_some() && rank < target_rank {
                            let step_handle = path_ref.next_step(step_handle.0).unwrap();
                            position += self.storage.graph.node_len(node_handle);
                            node_handle = step_handle.handle();
                            rank += 1;
                        }
                        let mut triples = self.step_handle_to_triples(
                            &path_name,
                            subject,
                            predicate,
                            object,
                            graph_name,
                            node_handle,
                            Some(rank),
                            Some(position),
                        );
                        results.append(&mut triples);
                    }
                }
                StepType::Position(path_name, position) => {
                    if let Some(id) = self.storage.graph.get_path_id(path_name.as_bytes()) {
                        if let Some(step) = self.storage.graph.path_step_at_base(id, position) {
                            let node_handle =
                                self.storage.graph.path_handle_at_step(id, step).unwrap();
                            let rank = step.pack() as usize + 1;
                            let mut triples = self.step_handle_to_triples(
                                &path_name,
                                subject,
                                predicate,
                                object,
                                graph_name,
                                node_handle,
                                Some(rank),
                                Some(position),
                            );
                            results.append(&mut triples);
                        }
                    }
                }
            }
        }
        results
    }

    fn get_step_iri_fields(&self, term: Option<&EncodedTerm>) -> Option<StepType> {
        let term = term?;
        if let EncodedTerm::NamedNode { iri_id, value } = term {
            let mut parts = value.split("/").collect::<Vec<_>>();
            parts.reverse();
            if parts.len() < 5 || parts[3] != "path" {
                return None;
            }
            let path_name = parts[2].to_owned();
            match parts[1] {
                "step" => Some(StepType::Rank(path_name, parts[0].parse().ok()?)),
                "position" => Some(StepType::Position(path_name, parts[0].parse().ok()?)),
                _ => None,
            }
        } else {
            None
        }
    }

    fn step_handle_to_triples(
        &self,
        path_name: &str,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
        node_handle: Handle,
        rank: Option<usize>,
        position: Option<usize>,
    ) -> Vec<EncodedQuad> {
        let mut results = Vec::new();
        let step_iri = self.step_to_namednode(path_name, rank).unwrap();
        let node_len = self.storage.graph.node_len(node_handle);
        let path_iri = self.path_to_namednode(path_name).unwrap();
        let rank = rank.unwrap() as i64;
        let position = position.unwrap() as i64;

        if subject.is_none() || step_iri == subject.unwrap().to_owned() {
            if self.is_vocab(predicate, rdf::TYPE) || predicate.is_none() {
                if object.is_none() || self.is_vocab(object, vg::STEP) {
                    results.push(EncodedQuad::new(
                        step_iri.clone(),
                        rdf::TYPE.into(),
                        vg::STEP.into(),
                        graph_name.to_owned(),
                    ));
                }
                if object.is_none() || self.is_vocab(object, faldo::REGION) {
                    results.push(EncodedQuad::new(
                        step_iri.clone(),
                        rdf::TYPE.into(),
                        faldo::REGION.into(),
                        graph_name.to_owned(),
                    ));
                }
            }
            let node_iri = self.handle_to_namednode(node_handle).unwrap();
            if (self.is_vocab(predicate, vg::NODE_PRED)
                || predicate.is_none() && !node_handle.is_reverse())
                && (object.is_none() || node_iri == object.unwrap().to_owned())
            {
                results.push(EncodedQuad::new(
                    step_iri.clone(),
                    vg::NODE_PRED.into(),
                    node_iri.clone(),
                    graph_name.to_owned(),
                ));
            }

            if (self.is_vocab(predicate, vg::REVERSE_OF_NODE)
                || predicate.is_none() && node_handle.is_reverse())
                && (object.is_none() || node_iri == object.unwrap().to_owned())
            {
                results.push(EncodedQuad::new(
                    step_iri.clone(),
                    vg::REVERSE_OF_NODE.into(),
                    node_iri,
                    graph_name.to_owned(),
                ));
            }

            if self.is_vocab(predicate, vg::RANK) || predicate.is_none() {
                let rank_literal = EncodedTerm::IntegerLiteral(rank.into());
                if object.is_none() || object.unwrap().to_owned() == rank_literal {
                    results.push(EncodedQuad::new(
                        step_iri.clone(),
                        vg::RANK.into(),
                        rank_literal,
                        graph_name.to_owned(),
                    ));
                }
            }

            if self.is_vocab(predicate, vg::POSITION) || predicate.is_none() {
                let position_literal = EncodedTerm::IntegerLiteral(position.into());
                if object.is_none() || object.unwrap().to_owned() == position_literal {
                    results.push(EncodedQuad::new(
                        step_iri.clone(),
                        vg::RANK.into(),
                        position_literal,
                        graph_name.to_owned(),
                    ));
                }
            }

            if self.is_vocab(predicate, vg::PATH_PRED) || predicate.is_none() {
                if object.is_none() || path_iri == object.unwrap().to_owned() {
                    results.push(EncodedQuad::new(
                        step_iri.clone(),
                        vg::PATH_PRED.into(),
                        path_iri.clone(),
                        graph_name.to_owned(),
                    ));
                }
            }

            if predicate.is_none() || self.is_vocab(predicate, faldo::BEGIN) {
                results.push(EncodedQuad::new(
                    step_iri.clone(),
                    faldo::BEGIN.into(),
                    self.get_faldo_border_namednode(position as usize, path_name)
                        .unwrap(), // FIX
                    graph_name.to_owned(),
                ));
            }
            if predicate.is_none() || self.is_vocab(predicate, faldo::END) {
                results.push(EncodedQuad::new(
                    step_iri,
                    faldo::END.into(),
                    self.get_faldo_border_namednode(position as usize + node_len, path_name)
                        .unwrap(), // FIX
                    graph_name.to_owned(),
                ));
            }

            if subject.is_none() {
                let begin_pos = position as usize;
                let begin = self.get_faldo_border_namednode(begin_pos, path_name);
                let mut begins = self.faldo_for_step(
                    begin_pos,
                    path_iri.clone(),
                    begin,
                    predicate,
                    object,
                    graph_name,
                );
                results.append(&mut begins);
                let end_pos = position as usize + node_len;
                let end = self.get_faldo_border_namednode(end_pos, path_name);
                let mut ends =
                    self.faldo_for_step(end_pos, path_iri, end, predicate, object, graph_name);
                results.append(&mut ends);
            }
        }
        // TODO: reverse parsing
        results
    }

    fn get_faldo_border_namednode(&self, position: usize, path_name: &str) -> Option<EncodedTerm> {
        let text = format!(
            "{}/path/{}/position/{}",
            self.storage.base, path_name, position
        );
        let named_node = NamedNode::new(text).unwrap();
        Some(named_node.as_ref().into())
    }

    fn faldo_for_step(
        &self,
        position: usize,
        path_iri: EncodedTerm,
        subject: Option<EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
    ) -> Vec<EncodedQuad> {
        let mut results = Vec::new();
        let ep = EncodedTerm::IntegerLiteral((position as i64).into());
        if (predicate.is_none() || self.is_vocab(predicate, faldo::POSITION_PRED))
            && (object.is_none() || object.unwrap().to_owned() == ep)
        {
            results.push(EncodedQuad::new(
                subject.clone().unwrap(),
                faldo::POSITION_PRED.into(),
                ep,
                graph_name.to_owned(),
            ));
        }
        if (predicate.is_none() || self.is_vocab(predicate, rdf::TYPE))
            && (object.is_none() || self.is_vocab(object, faldo::EXACT_POSITION))
        {
            results.push(EncodedQuad::new(
                subject.clone().unwrap(),
                rdf::TYPE.into(),
                faldo::EXACT_POSITION.into(),
                graph_name.to_owned(),
            ));
        }
        if (predicate.is_none() || self.is_vocab(predicate, rdf::TYPE))
            && (object.is_none() || self.is_vocab(object, faldo::POSITION))
        {
            results.push(EncodedQuad::new(
                subject.clone().unwrap(),
                rdf::TYPE.into(),
                faldo::POSITION.into(),
                graph_name.to_owned(),
            ));
        }
        if predicate.is_none()
            || self.is_vocab(predicate, faldo::REFERENCE)
                && (object.is_none() || object.unwrap().to_owned() == path_iri)
        {
            results.push(EncodedQuad::new(
                subject.unwrap(),
                faldo::REFERENCE.into(),
                path_iri,
                graph_name.to_owned(),
            ));
        }
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

    fn step_to_namednode(&self, path_name: &str, rank: Option<usize>) -> Option<EncodedTerm> {
        let text = format!("{}/path/{}/step/{}", self.storage.base, path_name, rank?);
        let named_node = NamedNode::new(text).ok()?;
        Some(named_node.as_ref().into())
    }

    fn path_to_namednode(&self, path_name: &str) -> Option<EncodedTerm> {
        let text = format!("{}/path/{}", self.storage.base, path_name);
        let named_node = NamedNode::new(text).ok()?;
        Some(named_node.as_ref().into())
    }

    fn get_path_name(&self, path_id: PathId) -> Option<String> {
        if let Some(path_name_iter) = self.storage.graph.get_path_name(path_id) {
            let path_name: Vec<u8> = path_name_iter.collect();
            let path_name = std::str::from_utf8(&path_name).ok()?;
            Some(path_name.to_owned())
        } else {
            None
        }
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

    fn is_step_associated(&self, predicate: Option<&EncodedTerm>) -> bool {
        let predicates = [
            vg::RANK,
            vg::POSITION,
            vg::PATH_PRED,
            vg::NODE_PRED,
            vg::REVERSE_OF_NODE,
            faldo::BEGIN,
            faldo::END,
            faldo::REFERENCE,
            faldo::POSITION_PRED,
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
                let text = term
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

    #[cfg(not(target_family = "wasm"))]
    pub fn get_str(&self, _key: &StrHash) -> Result<Option<String>, StorageError> {
        Ok(None)
    }

    #[cfg(not(target_family = "wasm"))]
    pub fn contains_str(&self, _key: &StrHash) -> Result<bool, StorageError> {
        Ok(true)
    }
}

impl StrLookup for StorageGenerator {
    fn get_str(&self, key: &StrHash) -> Result<Option<String>, StorageError> {
        self.get_str(key)
    }

    fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
        self.contains_str(key)
    }
}

// FIX: Change usize to u64
enum StepType {
    Rank(String, usize),
    Position(String, usize),
}

#[cfg(test)]
mod tests {
    use std::{path::Path, str::FromStr};

    use crate::storage::small_string::SmallString;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    const BASE: &'static str = "https://example.org";

    fn get_generator(gfa: &str) -> StorageGenerator {
        let storage = Storage::from_str(gfa).unwrap();
        StorageGenerator::new(storage)
    }

    fn get_odgi_test_file_generator(file_name: &str) -> StorageGenerator {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file_name);
        println!("{}", path.to_str().unwrap());
        let storage = Storage::open(&path).unwrap();
        StorageGenerator::new(storage)
    }

    fn print_quad(quad: &EncodedQuad) {
        let sub = match &quad.subject {
            EncodedTerm::NamedNode { iri_id: _, value } => value.to_owned(),
            _ => "NOT NAMED".to_owned(),
        };
        let pre = match &quad.predicate {
            EncodedTerm::NamedNode { iri_id: _, value } => value.to_owned(),
            _ => "NOT NAMED".to_owned(),
        };
        let obj = match &quad.object {
            EncodedTerm::NamedNode { iri_id: _, value } => value.to_owned(),
            EncodedTerm::SmallStringLiteral(value) => format!("\"{}\"", value).to_string(),
            _ => "NOT NAMED".to_owned(),
        };
        println!("{}\t{}\t{} .", sub, pre, obj);
    }

    fn get_node(id: i64) -> EncodedTerm {
        let text = format!("{}/node/{}", BASE, id);
        let named_node = NamedNode::new(text).unwrap();
        named_node.as_ref().into()
    }

    #[test]
    fn test_single_node() {
        let gen = get_odgi_test_file_generator("t_red.gfa");
        let node_triple = gen.nodes(None, None, None, &EncodedTerm::DefaultGraph);
        let node_id_quad = EncodedQuad::new(
            get_node(1),
            rdf::TYPE.into(),
            vg::NODE.into(),
            EncodedTerm::DefaultGraph,
        );
        let sequence_quad = EncodedQuad::new(
            get_node(1),
            rdf::VALUE.into(),
            EncodedTerm::SmallStringLiteral(SmallString::from_str("CAAATAAG").unwrap()),
            EncodedTerm::DefaultGraph,
        );
        assert_eq!(node_triple.len(), 2);
        assert!(node_triple.contains(&node_id_quad));
        assert!(node_triple.contains(&sequence_quad));
    }

    #[test]
    // FIX: Currently triple gets generated twice
    fn test_single_node_non_generic() {
        let gen = get_odgi_test_file_generator("t_red.gfa");
        let node_1 = get_node(1);
        let node_triple = gen.nodes(
            Some(&node_1),
            Some(&rdf::TYPE.into()),
            Some(&vg::NODE.into()),
            &EncodedTerm::DefaultGraph,
        );
        let node_id_quad = EncodedQuad::new(
            get_node(1),
            rdf::TYPE.into(),
            vg::NODE.into(),
            EncodedTerm::DefaultGraph,
        );
        for tripe in &node_triple {
            print_quad(tripe);
        }
        assert_eq!(node_triple.len(), 1);
        assert!(node_triple.contains(&node_id_quad));
    }

    #[test]
    fn test_double_node() {
        // Reminder: fails with "old" version of rs-handlegraph (use git-master)
        let gen = get_odgi_test_file_generator("t_double.gfa");
        let node_triple = gen.nodes(None, None, None, &EncodedTerm::DefaultGraph);
        let links_quad = EncodedQuad::new(
            get_node(1),
            vg::LINKS.into(),
            get_node(2),
            EncodedTerm::DefaultGraph,
        );
        let links_f2f_quad = EncodedQuad::new(
            get_node(1),
            vg::LINKS_FORWARD_TO_FORWARD.into(),
            get_node(2),
            EncodedTerm::DefaultGraph,
        );
        for tripe in &node_triple {
            print_quad(tripe);
        }
        assert_eq!(node_triple.len(), 6);
        assert!(node_triple.contains(&links_quad));
        assert!(node_triple.contains(&links_f2f_quad));
    }
}
