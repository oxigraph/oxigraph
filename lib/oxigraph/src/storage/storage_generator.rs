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
use handlegraph::pathhandlegraph::{path::PathStep, GraphPathsRef, IntoPathIds, PathBase};
use handlegraph::pathhandlegraph::{GraphPathNames, GraphPaths, PathId, PathSequences, GraphPathsSteps};
use handlegraph::{
    handlegraph::IntoHandles, handlegraph::IntoNeighbors, handlegraph::IntoSequences,
};
use oxrdf::vocab::rdfs;
use oxrdf::{Literal, NamedNode};
use std::str;

pub struct StorageGenerator {
    storage: Storage,
}

impl StorageGenerator {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    fn print_quad(&self, quad: &EncodedQuad) {
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
            EncodedTerm::IntegerLiteral(value) => value.to_string(),
            _ => "NOT NAMED".to_owned(),
        };
        println!("\t- {}\t{}\t{} .", sub, pre, obj);
    }

    pub fn quads_for_pattern(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
    ) -> ChainedDecodingQuadIterator {

        // There should be no blank nodes in the data
        if subject.is_some_and(|s| s.is_blank_node()) || object.is_some_and(|o| o.is_blank_node()) {
            println!("OF: blanks");
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms: Vec::new(),
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };
        }

        if self.is_vocab(predicate, rdf::TYPE) && object.is_some() {
            println!("OF: rdf::type");
            let terms = self.type_triples(subject, predicate, object, graph_name);
            for triple in &terms {
                self.print_quad(triple);
            }
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms,
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };
        } else if self.is_node_related(predicate) {
            println!("OF: nodes");
            let terms = self.nodes(subject, predicate, object, graph_name);
            for triple in &terms {
                self.print_quad(triple);
            }
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms,
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };
        } else if self.is_step_associated(predicate) {
            println!("OF: steps");
            let terms = self.steps(subject, predicate, object, graph_name);
            for triple in &terms {
                self.print_quad(triple);
            }
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms,
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };
        } else if self.is_vocab(predicate, rdfs::LABEL) {
            println!("OF: rdfs::label");
            let terms = self.paths(subject, predicate, object, graph_name);
            for triple in &terms {
                self.print_quad(triple);
            }
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms,
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };

        } else if subject.is_none() && predicate.is_none() && object.is_none() {
            println!("OF: triple none");
            let mut terms = self.nodes(subject, predicate, object, graph_name);
            let terms_paths = self.paths(subject, predicate, object, graph_name);
            let terms_steps = self.steps(subject, predicate, object, graph_name);
            terms.extend(terms_paths);
            terms.extend(terms_steps);
            for triple in &terms {
                self.print_quad(triple);
            }
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms,
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };
        } else if subject.is_some() {
            println!("OF: subject some");
            let terms = match self.get_term_type(subject.unwrap()) {
                Some(SubjectType::NodeIri) => {
                    let mut terms = self.handle_to_triples(subject.unwrap(), predicate, object, graph_name);
                    let terms_edge = self.handle_to_edge_triples(subject.unwrap(), predicate, object, graph_name);
                    terms.extend(terms_edge);
                    terms
                },
                Some(SubjectType::PathIri) => {
                    self.paths(subject, predicate, object, graph_name)
                },
                Some(SubjectType::StepIri) => {
                    self.steps(subject, predicate, object, graph_name)
                },
                Some(SubjectType::StepBorderIri) => {
                    self.steps(subject, predicate, object, graph_name)
                },
                None => {
                    Vec::new()
                }
            };
            for triple in &terms {
                self.print_quad(triple);
            }
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms,
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };

        } else {
            return ChainedDecodingQuadIterator {
                first: DecodingQuadIterator {
                    terms: Vec::new(),
                    encoding: QuadEncoding::Spog,
                },
                second: None,
            };
        }
    }

    fn get_term_type(&self, term: &EncodedTerm) -> Option<SubjectType> {
    if let EncodedTerm::NamedNode { iri_id: _, value } = term {
        let mut parts = value.split("/").collect::<Vec<_>>();
        parts.reverse();
        if parts[1] == "node" {
            return Some(SubjectType::NodeIri);
        } else if parts[3] == "path" && parts[1] == "step" {
            return Some(SubjectType::StepIri);
        } else if parts[3] == "path" && parts[1] == "position" {
            return Some(SubjectType::StepBorderIri);
        } else if parts[1] == "path" {
            return Some(SubjectType::PathIri);
        } else {
            return None;
        }
    } else {
        None
    }

    }

    fn type_triples(
        &self,
        subject: Option<&EncodedTerm>,
        predicate: Option<&EncodedTerm>,
        object: Option<&EncodedTerm>,
        graph_name: &EncodedTerm,
        ) -> Vec<EncodedQuad> {
        if self.is_vocab(object, vg::NODE) {
            self.nodes(subject, predicate, object, graph_name)
        } else if self.is_vocab(object, vg::PATH) {
            self.paths(subject, predicate, object, graph_name)
        } else if self.is_step_associated_type(object) {
            self.steps(subject, predicate, object, graph_name)
        } else {
            Vec::new()
        }
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
                let is_node_iri = self.is_node_iri_in_graph(sub);
                if self.is_vocab(predicate, rdf::TYPE)
                    && (self.is_vocab(object, vg::NODE) || object.is_none())
                    && is_node_iri
                {
                    println!("NF: type predicate");
                    results.push(EncodedQuad::new(
                        sub.to_owned(),
                        rdf::TYPE.into(),
                        vg::NODE.into(),
                        graph_name.to_owned(),
                    ));
                } else if predicate.is_none() && self.is_vocab(object, vg::NODE) && is_node_iri {
                    println!("NF: node object");
                    results.push(EncodedQuad::new(
                        sub.to_owned(),
                        rdf::TYPE.into(),
                        vg::NODE.into(),
                        graph_name.to_owned(),
                    ));
                } else if predicate.is_none() && is_node_iri {
                    println!("NF: none predicate");
                    results.push(EncodedQuad::new(
                        sub.to_owned(),
                        rdf::TYPE.into(),
                        vg::NODE.into(),
                        graph_name.to_owned(),
                    ));
                }

                if is_node_iri {
                    println!("NF: is_node_iri");
                    let mut triples = self.handle_to_triples(sub, predicate, object, graph_name);
                    let mut edge_triples =
                        self.handle_to_edge_triples(sub, predicate, object, graph_name);
                    results.append(&mut triples);
                    results.append(&mut edge_triples);
                }
            }
            None => {
                for handle in self.storage.graph.handles() {
                    let term = self
                        .handle_to_namednode(handle)
                        .expect("Can turn handle to namednode");
                    let mut recursion_results =
                        self.nodes(Some(&term), predicate, object, graph_name);
                    results.append(&mut recursion_results);
                }
                // println!("{:?}", results);
            }
        }
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
            println!("SF: none subject");
            for path_id in self.storage.graph.path_ids() {
                if let Some(path_ref) = self.storage.graph.get_path_ref(path_id) {
                    let path_name = self.get_path_name(path_id).unwrap();
                    let mut rank = 1;
                    let mut position = 1;
                    let step_handle = path_ref.step_at(path_ref.first_step());
                    if step_handle.is_none() {
                        continue;
                    }
                    let mut step_handle = step_handle.unwrap();
                    let mut node_handle = step_handle.handle();
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

                    let steps = self.storage.graph.path_steps(path_id).expect("Path has steps");
                    for _ in steps.skip(1) {
                        step_handle = path_ref.next_step(step_handle.0).unwrap();
                        position += self.storage.graph.node_len(node_handle);
                        node_handle = step_handle.handle();
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
            println!("SF: some subject");
            match step_type {
                StepType::Rank(path_name, target_rank) => {
                    println!("RANK: {}, {}", path_name, target_rank);
                    if let Some(id) = self.storage.graph.get_path_id(path_name.as_bytes()) {
                        let path_ref = self.storage.graph.get_path_ref(id).unwrap();
                        let step_handle = path_ref.step_at(path_ref.first_step());
                        let mut step_handle = step_handle.unwrap();
                        let mut node_handle = step_handle.handle();
                        let mut rank = 1;
                        let mut position = 1;

                        let steps = self.storage.graph.path_steps(id).expect("Path has steps");
                        for _ in steps.skip(1) {
                            if rank >= target_rank {
                                break;
                            }
                            step_handle = path_ref.next_step(step_handle.0).unwrap();
                            position += self.storage.graph.node_len(node_handle);
                            node_handle = step_handle.handle();
                            rank += 1;
                        }
                        println!("Now handling: {}, {}, {}", rank, position, node_handle.0);
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
                    println!("POSITION: {}, {}", path_name, position);
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
        if let EncodedTerm::NamedNode { iri_id: _, value } = term {
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
        let position_literal = EncodedTerm::IntegerLiteral(position.into());
        println!("SH");

        if subject.is_none() || step_iri == subject.unwrap().to_owned() {
            if self.is_vocab(predicate, rdf::TYPE) || predicate.is_none() {
                if object.is_none() || self.is_vocab(object, vg::STEP) {
                    println!("SH: none/type predicate");
                    results.push(EncodedQuad::new(
                        step_iri.clone(),
                        rdf::TYPE.into(),
                        vg::STEP.into(),
                        graph_name.to_owned(),
                    ));
                }
                if object.is_none() || self.is_vocab(object, faldo::REGION) {
                    println!("SH: region object");
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
                println!("SH: node object");
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
                println!("SH: reverse node object");
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
                    println!("SH: rank predicate");
                    results.push(EncodedQuad::new(
                        step_iri.clone(),
                        vg::RANK.into(),
                        rank_literal,
                        graph_name.to_owned(),
                    ));
                }
            }

            if self.is_vocab(predicate, vg::POSITION) || predicate.is_none() {
                if object.is_none() || object.unwrap().to_owned() == position_literal {
                    println!("SH: position predicate");
                    results.push(EncodedQuad::new(
                        step_iri.clone(),
                        vg::POSITION.into(),
                        position_literal.clone(),
                        graph_name.to_owned(),
                    ));
                }
            }

            if self.is_vocab(predicate, vg::PATH_PRED) || predicate.is_none() {
                if object.is_none() || path_iri == object.unwrap().to_owned() {
                    println!("SH: path predicate");
                    results.push(EncodedQuad::new(
                        step_iri.clone(),
                        vg::PATH_PRED.into(),
                        path_iri.clone(),
                        graph_name.to_owned(),
                    ));
                }
            }

            if predicate.is_none() || self.is_vocab(predicate, faldo::BEGIN) {
                if object.is_none() || object.unwrap().to_owned() == position_literal {
                    println!("SH: begin predicate");
                    results.push(EncodedQuad::new(
                        step_iri.clone(),
                        faldo::BEGIN.into(),
                        self.get_faldo_border_namednode(position as usize, path_name)
                            .unwrap(), // FIX
                        graph_name.to_owned(),
                    ));
                }
            }
            if predicate.is_none() || self.is_vocab(predicate, faldo::END) {
                // FIX: End position_literal vs position + node_len
                if object.is_none() || object.unwrap().to_owned() == position_literal {
                    println!("SH: end predicate");
                    results.push(EncodedQuad::new(
                        step_iri,
                        faldo::END.into(),
                        self.get_faldo_border_namednode(position as usize + node_len, path_name)
                            .unwrap(), // FIX
                        graph_name.to_owned(),
                    ));
                }
            }

            if subject.is_none() {
                println!("SH: trailing none subject");
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
            println!("FS: position");
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
            println!("FS: position");
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
        if (predicate.is_none()
            || self.is_vocab(predicate, faldo::REFERENCE))
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
            println!("Decoding1");
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
        }
        // else if (self.is_vocab(predicate, rdf::TYPE) || predicate.is_none())
        //     && (object.is_none() || self.is_vocab(object, vg::NODE))
        // {
        //     results.push(EncodedQuad::new(
        //         subject.to_owned(),
        //         rdf::TYPE.into(),
        //         vg::NODE.into(),
        //         graph_name.to_owned(),
        //     ));
        // }
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
        if predicate.is_none() || self.is_node_related(predicate) {
            let handle = Handle::new(
                self.get_node_id(subject).expect("Subject has node id"),
                Orientation::Forward,
            );
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

    fn is_step_associated_type(&self, object: Option<&EncodedTerm>) -> bool {
        let types = [
            faldo::REGION,
            faldo::EXACT_POSITION,
            faldo::POSITION,
            vg::STEP,
        ];
        if object.is_none() {
            return false;
        }
        types
            .into_iter()
            .map(|x| self.is_vocab(object, x))
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

    //fn contains_str(&self, key: &StrHash) -> Result<bool, StorageError> {
    //    self.contains_str(key)
    //}
}

// FIX: Change usize to u64
enum StepType {
    Rank(String, usize),
    Position(String, usize),
}

enum SubjectType {
    PathIri,
    StepBorderIri,
    NodeIri,
    StepIri,
}

#[cfg(test)]
mod tests {
    use std::{path::Path, str::FromStr};

    use crate::storage::small_string::SmallString;

    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;
    const BASE: &'static str = "https://example.org";

    fn _get_generator(gfa: &str) -> StorageGenerator {
        let storage = Storage::from_str(gfa).unwrap();
        StorageGenerator::new(storage)
    }

    fn get_odgi_test_file_generator(file_name: &str) -> StorageGenerator {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(file_name);
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
            EncodedTerm::IntegerLiteral(value) => value.to_string(),
            _ => "NOT NAMED".to_owned(),
        };
        println!("{}\t{}\t{} .", sub, pre, obj);
    }

    fn get_node(id: i64) -> EncodedTerm {
        let text = format!("{}/node/{}", BASE, id);
        let named_node = NamedNode::new(text).unwrap();
        named_node.as_ref().into()
    }

    fn get_step(path: &str, id: i64) -> EncodedTerm {
        let text = format!("{}/path/{}/step/{}", BASE, path, id);
        let named_node = NamedNode::new(text).unwrap();
        named_node.as_ref().into()
    }

    fn get_position(path: &str, id: i64) -> EncodedTerm {
        let text = format!("{}/path/{}/position/{}", BASE, path, id);
        let named_node = NamedNode::new(text).unwrap();
        named_node.as_ref().into()
    }

    fn count_subjects(subject: &EncodedTerm, triples: &Vec<EncodedQuad>) -> usize {
        let mut count = 0;
        for triple in triples {
            if &triple.subject == subject {
                count += 1;
            }
        }
        count
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
    fn test_single_node_type_spo() {
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
    fn test_single_node_type_s() {
        let gen = get_odgi_test_file_generator("t_red.gfa");
        let node_triple = gen.nodes(
            Some(&get_node(1)),
            None,
            None,
            &EncodedTerm::DefaultGraph,
        );
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
        for tripe in &node_triple {
            print_quad(tripe);
        }
        assert_eq!(node_triple.len(), 2);
        assert!(node_triple.contains(&node_id_quad));
        assert!(node_triple.contains(&sequence_quad));
    }

    #[test]
    fn test_single_node_type_p() {
        let gen = get_odgi_test_file_generator("t_red.gfa");
        let node_triple = gen.nodes(
            None,
            Some(&rdf::TYPE.into()),
            None,
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
    fn test_single_node_type_o() {
        let gen = get_odgi_test_file_generator("t_red.gfa");
        let node_triple = gen.nodes(
            None,
            None,
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

    #[test]
    // TODO: Fix position numbers e.g. having pos/1 + pos/9 and pos/9 + pos/10
    fn test_step() {
        let gen = get_odgi_test_file_generator("t_step.gfa");
        let step_triples = gen.steps(None, None, None, &EncodedTerm::DefaultGraph);
        for triple in &step_triples {
            print_quad(triple);
        }
        let count_step1 = count_subjects(&get_step("x", 1), &step_triples);
        let count_step2 = count_subjects(&get_step("x", 2), &step_triples);
        let count_pos1 = count_subjects(&get_position("x", 1), &step_triples);
        let count_pos9 = count_subjects(&get_position("x", 9), &step_triples);
        let count_pos10 = count_subjects(&get_position("x", 10), &step_triples);
        assert_eq!(count_step1, 8, "Number of step 1 triples");
        assert_eq!(count_step2, 8, "Number of step 2 triples");
        assert_eq!(count_pos1, 4, "Number of pos 1 triples");
        assert_eq!(count_pos9, 8, "Number of pos 9 triples");
        assert_eq!(count_pos10, 4, "Number of pos 10 triples");
    }

    #[test]
    fn test_step_s() {
        let gen = get_odgi_test_file_generator("t_step.gfa");
        let step_triples = gen.steps(Some(&get_step("x", 1)), None, None, &EncodedTerm::DefaultGraph);
        for triple in &step_triples {
            print_quad(triple);
        }
        assert_eq!(step_triples.len(), 8, "Number of step 1 triples");
    }

    #[test]
    fn test_step_p() {
        let gen = get_odgi_test_file_generator("t_step.gfa");
        let step_triples = gen.steps(None, Some(&rdf::TYPE.into()), None, &EncodedTerm::DefaultGraph);
        for triple in &step_triples {
            print_quad(triple);
        }
        assert_eq!(step_triples.len(), 12, "Number of type triples");
    }

    #[test]
    fn test_step_o() {
        let gen = get_odgi_test_file_generator("t_step.gfa");
        let step_triples = gen.steps(None, None, Some(&get_node(1)), &EncodedTerm::DefaultGraph);
        for triple in &step_triples {
            print_quad(triple);
        }
        assert_eq!(step_triples.len(), 1, "Number of type triples");
    }

    #[test]
    fn test_full() {
        let gen = get_odgi_test_file_generator("t.gfa");
        let node_triple = gen.quads_for_pattern(None, None, None, &EncodedTerm::DefaultGraph);
        for tripe in &node_triple.first.terms {
            print_quad(tripe);
        }
        assert_eq!(1, 1);
    }
}
