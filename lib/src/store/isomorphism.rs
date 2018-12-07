use crate::model::*;
use crate::Result;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::hash::Hasher;

#[derive(Eq, PartialEq, Hash, Ord, PartialOrd)]
struct SubjectPredicate {
    subject: NamedOrBlankNode,
    predicate: NamedNode,
}

impl SubjectPredicate {
    fn new(subject: NamedOrBlankNode, predicate: NamedNode) -> Self {
        Self { subject, predicate }
    }
}

#[derive(Eq, PartialEq, Hash, Ord, PartialOrd)]
struct PredicateObject {
    predicate: NamedNode,
    object: Term,
}

impl PredicateObject {
    fn new(predicate: NamedNode, object: Term) -> Self {
        Self { predicate, object }
    }
}

fn subject_predicates_for_object(
    graph: &impl Graph,
    object: &Term,
) -> Result<impl Iterator<Item = Result<SubjectPredicate>>> {
    Ok(graph
        .triples_for_object(object)?
        .map(|t| t.map(|t| SubjectPredicate::new(t.subject().clone(), t.predicate_owned()))))
}

fn predicate_objects_for_subject(
    graph: &impl Graph,
    subject: &NamedOrBlankNode,
) -> Result<impl Iterator<Item = Result<PredicateObject>>> {
    Ok(graph
        .triples_for_subject(subject)?
        .map(|t| t.map(|t| PredicateObject::new(t.predicate().clone(), t.object_owned()))))
}

fn hash_blank_nodes(
    bnodes: HashSet<BlankNode>,
    graph: &impl Graph,
) -> Result<HashMap<u64, Vec<BlankNode>>> {
    let mut bnodes_by_hash: HashMap<u64, Vec<BlankNode>> = HashMap::default();

    // NB: we need to sort the triples to have the same hash
    for bnode in bnodes {
        let mut hasher = DefaultHasher::new();

        {
            let subject = NamedOrBlankNode::from(bnode.clone());
            let mut po_set: BTreeSet<PredicateObject> = BTreeSet::default();
            for po in predicate_objects_for_subject(graph, &subject)? {
                let po = po?;
                if !po.object.is_blank_node() {
                    po_set.insert(po);
                }
            }
            for po in po_set {
                po.hash(&mut hasher);
            }
        }

        {
            let object = Term::from(bnode.clone());
            let mut sp_set: BTreeSet<SubjectPredicate> = BTreeSet::default();
            for sp in subject_predicates_for_object(graph, &object)? {
                let sp = sp?;
                if !sp.subject.is_blank_node() {
                    sp_set.insert(sp);
                }
            }
            for sp in sp_set {
                sp.hash(&mut hasher);
            }
        }

        bnodes_by_hash
            .entry(hasher.finish())
            .or_insert_with(Vec::default)
            .push(bnode);
    }

    Ok(bnodes_by_hash)
}

pub trait GraphIsomorphism {
    /// Checks if two graphs are [isomorphic](https://www.w3.org/TR/rdf11-concepts/#dfn-graph-isomorphism)
    fn is_isomorphic(&self, other: &Self) -> Result<bool>;
}

impl<G: Graph> GraphIsomorphism for G {
    //TODO: proper isomorphism building
    fn is_isomorphic(&self, other: &Self) -> Result<bool> {
        if self.len()? != other.len()? {
            return Ok(false);
        }

        let mut self_bnodes: HashSet<BlankNode> = HashSet::default();
        let mut other_bnodes: HashSet<BlankNode> = HashSet::default();

        for t in self.iter()? {
            let t = t?;
            if let NamedOrBlankNode::BlankNode(subject) = t.subject() {
                self_bnodes.insert(subject.clone());
                if let Term::BlankNode(object) = t.object() {
                    self_bnodes.insert(object.clone());
                }
            } else if let Term::BlankNode(object) = t.object() {
                self_bnodes.insert(object.clone());
            } else if !other.contains(&t)? {
                return Ok(false);
            }
        }
        for t in other.iter()? {
            let t = t?;
            if let NamedOrBlankNode::BlankNode(subject) = t.subject() {
                other_bnodes.insert(subject.clone());
                if let Term::BlankNode(object) = t.object() {
                    other_bnodes.insert(object.clone());
                }
            } else if let Term::BlankNode(object) = t.object() {
                other_bnodes.insert(object.clone());
            } else if !self.contains(&t)? {
                return Ok(false);
            }
        }

        let self_bnodes_by_hash = hash_blank_nodes(self_bnodes, self)?;
        let other_bnodes_by_hash = hash_blank_nodes(other_bnodes, other)?;

        if self_bnodes_by_hash.len() != other_bnodes_by_hash.len() {
            return Ok(false);
        }

        for hash in self_bnodes_by_hash.keys() {
            if self_bnodes_by_hash.get(hash).map(|l| l.len())
                != other_bnodes_by_hash.get(hash).map(|l| l.len())
            {
                return Ok(false);
            }
        }

        Ok(true)
    }
}
