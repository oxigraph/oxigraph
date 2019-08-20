use crate::model::isomorphism::are_graphs_isomorphic;
use crate::model::*;
use std::collections::HashSet;
use std::fmt;
use std::iter::FromIterator;

/// A simple implementation of [RDF graphs](https://www.w3.org/TR/rdf11-concepts/#dfn-graph).
///
/// It is not done to hold big graphs.
///
/// Usage example:
/// ```
/// use rudf::model::*;
/// use rudf::model::SimpleGraph;
/// use std::str::FromStr;
///
/// let mut graph = SimpleGraph::default();
/// let ex = NamedNode::from_str("http://example.com").unwrap();
/// let triple = Triple::new(ex.clone(), ex.clone(), ex.clone());
/// graph.insert(triple.clone());
/// let results: Vec<Triple> = graph.triples_for_subject(&ex.into()).cloned().collect();
/// assert_eq!(vec![triple], results);
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Default)]
pub struct SimpleGraph {
    triples: HashSet<Triple>,
}

impl SimpleGraph {
    /// Returns all triples contained by the graph
    pub fn iter(&self) -> impl Iterator<Item = &Triple> {
        self.triples.iter()
    }

    pub fn triples_for_subject<'a>(
        &'a self,
        subject: &'a NamedOrBlankNode,
    ) -> impl Iterator<Item = &Triple> + 'a {
        self.iter().filter(move |t| t.subject() == subject)
    }

    pub fn objects_for_subject_predicate<'a>(
        &'a self,
        subject: &'a NamedOrBlankNode,
        predicate: &'a NamedNode,
    ) -> impl Iterator<Item = &Term> + 'a {
        self.iter()
            .filter(move |t| t.subject() == subject && t.predicate() == predicate)
            .map(|t| t.object())
    }

    pub fn object_for_subject_predicate<'a>(
        &'a self,
        subject: &'a NamedOrBlankNode,
        predicate: &'a NamedNode,
    ) -> Option<&'a Term> {
        self.objects_for_subject_predicate(subject, predicate)
            .next()
    }

    pub fn predicates_for_subject_object<'a>(
        &'a self,
        subject: &'a NamedOrBlankNode,
        object: &'a Term,
    ) -> impl Iterator<Item = &NamedNode> + 'a {
        self.iter()
            .filter(move |t| t.subject() == subject && t.object() == object)
            .map(|t| t.predicate())
    }

    pub fn triples_for_predicate<'a>(
        &'a self,
        predicate: &'a NamedNode,
    ) -> impl Iterator<Item = &Triple> + 'a {
        self.iter().filter(move |t| t.predicate() == predicate)
    }

    pub fn subjects_for_predicate_object<'a>(
        &'a self,
        predicate: &'a NamedNode,
        object: &'a Term,
    ) -> impl Iterator<Item = &NamedOrBlankNode> + 'a {
        self.iter()
            .filter(move |t| t.predicate() == predicate && t.object() == object)
            .map(|t| t.subject())
    }

    pub fn triples_for_object<'a>(
        &'a self,
        object: &'a Term,
    ) -> impl Iterator<Item = &Triple> + 'a {
        self.iter().filter(move |t| t.object() == object)
    }

    /// Checks if the graph contains the given triple
    pub fn contains(&self, triple: &Triple) -> bool {
        self.triples.contains(triple)
    }

    /// Adds a triple to the graph
    pub fn insert(&mut self, triple: Triple) -> bool {
        self.triples.insert(triple)
    }

    /// Removes a concrete triple from the graph
    pub fn remove(&mut self, triple: &Triple) -> bool {
        self.triples.remove(triple)
    }

    /// Returns the number of triples in this graph
    pub fn len(&self) -> usize {
        self.triples.len()
    }

    /// Checks if this graph contains a triple
    pub fn is_empty(&self) -> bool {
        self.triples.is_empty()
    }

    /// Checks if the current graph is [isomorphic](https://www.w3.org/TR/rdf11-concepts/#dfn-graph-isomorphism) with an other one
    ///
    /// Warning: This algorithm as a worst case complexity in n!
    pub fn is_isomorphic(&self, other: &SimpleGraph) -> bool {
        are_graphs_isomorphic(self, other)
    }
}

impl IntoIterator for SimpleGraph {
    type Item = Triple;
    type IntoIter = <HashSet<Triple> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.triples.into_iter()
    }
}

impl<'a> IntoIterator for &'a SimpleGraph {
    type Item = &'a Triple;
    type IntoIter = <&'a HashSet<Triple> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.triples.iter()
    }
}

impl FromIterator<Triple> for SimpleGraph {
    fn from_iter<I: IntoIterator<Item = Triple>>(iter: I) -> Self {
        Self {
            triples: HashSet::from_iter(iter),
        }
    }
}

impl Extend<Triple> for SimpleGraph {
    fn extend<I: IntoIterator<Item = Triple>>(&mut self, iter: I) {
        self.triples.extend(iter)
    }
}

impl fmt::Display for SimpleGraph {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for t in &self.triples {
            writeln!(f, "{}", t)?;
        }
        Ok(())
    }
}
