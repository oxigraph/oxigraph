use crate::model::*;
use std::collections::hash_map::{DefaultHasher, RandomState};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::hash::Hash;
use std::hash::Hasher;

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
struct SubjectPredicate<'a> {
    subject: &'a NamedOrBlankNode,
    predicate: &'a NamedNode,
}

#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
struct PredicateObject<'a> {
    predicate: &'a NamedNode,
    object: &'a Term,
}

fn subject_predicates_for_object<'a>(
    graph: &'a SimpleGraph,
    object: &'a Term,
) -> impl Iterator<Item = SubjectPredicate<'a>> + 'a {
    graph.triples_for_object(object).map(|t| SubjectPredicate {
        subject: t.subject(),
        predicate: t.predicate(),
    })
}

fn predicate_objects_for_subject<'a>(
    graph: &'a SimpleGraph,
    subject: &'a NamedOrBlankNode,
) -> impl Iterator<Item = PredicateObject<'a>> + 'a {
    graph.triples_for_subject(subject).map(|t| PredicateObject {
        predicate: t.predicate(),
        object: t.object(),
    })
}

fn split_hash_buckets<'a>(
    bnodes_by_hash: HashMap<u64, Vec<&'a BlankNode>>,
    graph: &'a SimpleGraph,
    distance: usize,
) -> HashMap<u64, Vec<&'a BlankNode>> {
    let mut new_bnodes_by_hash = HashMap::default();

    for (hash, bnodes) in bnodes_by_hash {
        if bnodes.len() == 1 {
            new_bnodes_by_hash.insert(hash, bnodes); // Nothing to improve
        } else {
            for bnode in bnodes {
                let mut starts = vec![NamedOrBlankNode::from(*bnode)];
                for _ in 0..distance {
                    let mut new_starts = Vec::default();
                    for s in starts {
                        for t in graph.triples_for_subject(&s) {
                            match t.object() {
                                Term::NamedNode(t) => new_starts.push(t.clone().into()),
                                Term::BlankNode(t) => new_starts.push(t.clone().into()),
                                Term::Literal(_) => (),
                            }
                        }
                        for t in graph.triples_for_object(&s.into()) {
                            new_starts.push(t.subject().clone());
                        }
                    }
                    starts = new_starts;
                }

                // We do the hashing
                let mut hasher = DefaultHasher::default();
                hash.hash(&mut hasher); // We start with the previous hash

                // NB: we need to sort the triples to have the same hash
                let mut po_set: BTreeSet<PredicateObject<'_>> = BTreeSet::default();
                for start in &starts {
                    for po in predicate_objects_for_subject(graph, start) {
                        match &po.object {
                            Term::BlankNode(_) => (),
                            _ => {
                                po_set.insert(po);
                            }
                        }
                    }
                }
                for po in &po_set {
                    po.hash(&mut hasher);
                }

                let mut sp_set: BTreeSet<SubjectPredicate<'_>> = BTreeSet::default();
                let term_starts: Vec<_> = starts.into_iter().map(|t| t.into()).collect();
                for start in &term_starts {
                    for sp in subject_predicates_for_object(graph, start) {
                        match &sp.subject {
                            NamedOrBlankNode::BlankNode(_) => (),
                            _ => {
                                sp_set.insert(sp);
                            }
                        }
                    }
                }
                for sp in &sp_set {
                    sp.hash(&mut hasher);
                }

                new_bnodes_by_hash
                    .entry(hasher.finish())
                    .or_insert_with(Vec::default)
                    .push(bnode);
            }
        }
    }
    new_bnodes_by_hash
}

fn build_and_check_containment_from_hashes<'a>(
    a_bnodes_by_hash: &mut Vec<(u64, Vec<&'a BlankNode>)>,
    b_bnodes_by_hash: &'a HashMap<u64, Vec<&'a BlankNode>>,
    a_to_b_mapping: &mut HashMap<&'a BlankNode, &'a BlankNode>,
    a: &'a SimpleGraph,
    b: &'a SimpleGraph,
    current_a_nodes: &[&'a BlankNode],
    current_b_nodes: &mut BTreeSet<&'a BlankNode>,
) -> bool {
    if let Some((a_node, remaining_a_node)) = current_a_nodes.split_last() {
        let b_nodes = current_b_nodes.iter().cloned().collect::<Vec<_>>();
        for b_node in b_nodes {
            current_b_nodes.remove(b_node);
            a_to_b_mapping.insert(a_node, b_node);
            if check_is_contained_focused(a_to_b_mapping, a_node, a, b)
                && build_and_check_containment_from_hashes(
                    a_bnodes_by_hash,
                    b_bnodes_by_hash,
                    a_to_b_mapping,
                    a,
                    b,
                    remaining_a_node,
                    current_b_nodes,
                )
            {
                return true;
            }
            current_b_nodes.insert(b_node);
        }
        a_to_b_mapping.remove(a_node);
        false
    } else {
        let (hash, new_a_nodes) = match a_bnodes_by_hash.pop() {
            Some(v) => v,
            None => return true,
        };

        let mut new_b_nodes = b_bnodes_by_hash
            .get(&hash)
            .map_or(BTreeSet::default(), |v| v.iter().cloned().collect());
        if new_a_nodes.len() != new_b_nodes.len() {
            return false;
        }

        if new_a_nodes.len() > 10 {
            eprintln!("Too big instance, aborting");
            return true; //TODO: Very very very bad
        }

        if build_and_check_containment_from_hashes(
            a_bnodes_by_hash,
            b_bnodes_by_hash,
            a_to_b_mapping,
            a,
            b,
            &new_a_nodes,
            &mut new_b_nodes,
        ) {
            true
        } else {
            a_bnodes_by_hash.push((hash, new_a_nodes));
            false
        }
    }
}

fn check_is_contained_focused<'a>(
    a_to_b_mapping: &mut HashMap<&'a BlankNode, &'a BlankNode>,
    a_bnode_focus: &'a BlankNode,
    a: &'a SimpleGraph,
    b: &'a SimpleGraph,
) -> bool {
    let a_bnode_subject = a_bnode_focus.clone().into();
    let a_bnode_object = a_bnode_focus.clone().into();
    let ts_a = a
        .triples_for_subject(&a_bnode_subject)
        .chain(a.triples_for_object(&a_bnode_object));
    for t_a in ts_a {
        let subject: NamedOrBlankNode = if let NamedOrBlankNode::BlankNode(s_a) = &t_a.subject() {
            if let Some(s_a) = a_to_b_mapping.get(s_a) {
                (*s_a).clone().into()
            } else {
                continue; // We skip for now
            }
        } else {
            t_a.subject().clone()
        };
        let predicate = t_a.predicate().clone();
        let object: Term = if let Term::BlankNode(o_a) = &t_a.object() {
            if let Some(o_a) = a_to_b_mapping.get(o_a) {
                (*o_a).clone().into()
            } else {
                continue; // We skip for now
            }
        } else {
            t_a.object().clone()
        };
        if !b.contains(&Triple::new(subject, predicate, object)) {
            return false;
        }
    }

    true
}

fn graph_blank_nodes(graph: &SimpleGraph) -> Vec<&BlankNode> {
    let mut blank_nodes: HashSet<&BlankNode, RandomState> = HashSet::default();
    for t in graph {
        if let NamedOrBlankNode::BlankNode(subject) = t.subject() {
            blank_nodes.insert(subject);
        }
        if let Term::BlankNode(object) = &t.object() {
            blank_nodes.insert(object);
        }
    }
    blank_nodes.into_iter().collect()
}

pub fn are_graphs_isomorphic(a: &SimpleGraph, b: &SimpleGraph) -> bool {
    if a.len() != b.len() {
        return false;
    }

    // We check containment of everything buts triples with blank nodes
    let mut a_bnodes_triples = SimpleGraph::default();
    for t in a {
        if t.subject().is_blank_node() || t.object().is_blank_node() {
            a_bnodes_triples.insert(t.clone());
        } else if !b.contains(t) {
            return false; // Triple in a not in b without blank nodes
        }
    }

    let mut b_bnodes_triples = SimpleGraph::default();
    for t in b {
        if t.subject().is_blank_node() || t.object().is_blank_node() {
            b_bnodes_triples.insert(t.clone());
        } else if !a.contains(t) {
            return false; // Triple in a not in b without blank nodes
        }
    }

    let mut a_bnodes_by_hash = HashMap::default();
    a_bnodes_by_hash.insert(0, graph_blank_nodes(&a_bnodes_triples));
    let mut b_bnodes_by_hash = HashMap::default();
    b_bnodes_by_hash.insert(0, graph_blank_nodes(&b_bnodes_triples));

    for distance in 0..5 {
        let max_size = a_bnodes_by_hash.values().map(Vec::len).max().unwrap_or(0);
        if max_size < 2 {
            break; // We only have small buckets
        }

        a_bnodes_by_hash = split_hash_buckets(a_bnodes_by_hash, a, distance);
        b_bnodes_by_hash = split_hash_buckets(b_bnodes_by_hash, b, distance);

        // Hashes should have the same size
        if a_bnodes_by_hash.len() != b_bnodes_by_hash.len() {
            return false;
        }
    }

    let mut sorted_a_bnodes_by_hash: Vec<_> = a_bnodes_by_hash.into_iter().collect();
    sorted_a_bnodes_by_hash.sort_by(|(_, l1), (_, l2)| l1.len().cmp(&l2.len()));

    build_and_check_containment_from_hashes(
        &mut sorted_a_bnodes_by_hash,
        &b_bnodes_by_hash,
        &mut HashMap::default(),
        &a_bnodes_triples,
        &b_bnodes_triples,
        &[],
        &mut BTreeSet::default(),
    )
}
