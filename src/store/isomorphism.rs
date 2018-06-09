use model::*;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::hash::Hash;
use std::hash::Hasher;
use store::memory::MemoryGraph;

#[derive(Eq, PartialEq, Hash, Ord, PartialOrd)]
struct SubjectPredicate<'a> {
    subject: &'a NamedOrBlankNode,
    predicate: &'a NamedNode,
}

impl<'a> SubjectPredicate<'a> {
    fn new(subject: &'a NamedOrBlankNode, predicate: &'a NamedNode) -> Self {
        Self { subject, predicate }
    }
}

#[derive(Eq, PartialEq, Hash, Ord, PartialOrd)]
struct PredicateObject<'a> {
    predicate: &'a NamedNode,
    object: &'a Term,
}

impl<'a> PredicateObject<'a> {
    fn new(predicate: &'a NamedNode, object: &'a Term) -> Self {
        Self { predicate, object }
    }
}

fn subject_predicates_for_object<'a>(
    graph: &'a MemoryGraph,
    object: &'a Term,
) -> impl Iterator<Item = SubjectPredicate<'a>> {
    graph
        .triples_for_object(object)
        .map(|t| SubjectPredicate::new(t.subject(), t.predicate()))
}

fn predicate_objects_for_subject<'a>(
    graph: &'a MemoryGraph,
    subject: &'a NamedOrBlankNode,
) -> impl Iterator<Item = PredicateObject<'a>> {
    graph
        .triples_for_subject(subject)
        .map(|t| PredicateObject::new(t.predicate(), t.object()))
}

fn hash_blank_nodes<'a>(
    bnodes: HashSet<&'a BlankNode>,
    graph: &'a MemoryGraph,
) -> HashMap<u64, Vec<&'a BlankNode>> {
    let mut bnodes_by_hash: HashMap<u64, Vec<&BlankNode>> = HashMap::default();

    // NB: we need to sort the triples to have the same hash
    for bnode in bnodes.into_iter() {
        let mut hasher = DefaultHasher::new();

        {
            let subject = NamedOrBlankNode::from(bnode.clone());
            let mut po_set: BTreeSet<PredicateObject> = BTreeSet::default();
            for po in predicate_objects_for_subject(&graph, &subject) {
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
            for sp in subject_predicates_for_object(&graph, &object) {
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

    bnodes_by_hash
}

pub trait GraphIsomorphism {
    /// Checks if two graphs are [isomorphic](https://www.w3.org/TR/rdf11-concepts/#dfn-graph-isomorphism)
    fn is_isomorphic(&self, other: &Self) -> bool;
}

impl GraphIsomorphism for MemoryGraph {
    //TODO: proper isomorphism building
    fn is_isomorphic(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }

        let mut self_bnodes: HashSet<&BlankNode> = HashSet::default();
        let mut other_bnodes: HashSet<&BlankNode> = HashSet::default();

        for t in self {
            if let NamedOrBlankNode::BlankNode(subject) = t.subject() {
                self_bnodes.insert(subject);
                if let Term::BlankNode(object) = t.object() {
                    self_bnodes.insert(object);
                }
            } else if let Term::BlankNode(object) = t.object() {
                self_bnodes.insert(object);
            } else if !other.contains(t) {
                return false;
            }
        }
        for t in other {
            if let NamedOrBlankNode::BlankNode(subject) = t.subject() {
                other_bnodes.insert(subject);
                if let Term::BlankNode(object) = t.object() {
                    other_bnodes.insert(object);
                }
            } else if let Term::BlankNode(object) = t.object() {
                other_bnodes.insert(object);
            } else if !self.contains(t) {
                return false;
            }
        }

        let self_bnodes_by_hash = hash_blank_nodes(self_bnodes, &self);
        let other_bnodes_by_hash = hash_blank_nodes(other_bnodes, &other);

        if self_bnodes_by_hash.len() != other_bnodes_by_hash.len() {
            return false;
        }

        for hash in self_bnodes_by_hash.keys() {
            if self_bnodes_by_hash.get(hash).map(|l| l.len())
                != other_bnodes_by_hash.get(hash).map(|l| l.len())
            {
                return false;
            }
        }

        true
    }
}
