use model::vocab::rdf;
use model::*;
use std::collections::HashSet;
use std::fmt;
use std::iter::FromIterator;

#[derive(Debug, Clone, Default)]
pub struct MemoryGraph {
    triples: HashSet<Triple>,
}

impl MemoryGraph {
    pub fn iter(&self) -> impl Iterator<Item = &Triple> {
        self.triples.iter()
    }

    pub fn triples_for_subject<'a>(
        &'a self,
        subject: &'a NamedOrBlankNode,
    ) -> impl Iterator<Item = &'a Triple> {
        self.iter().filter(move |t| t.subject() == subject)
    }

    pub fn triples_for_predicate<'a>(
        &'a self,
        predicate: &'a NamedNode,
    ) -> impl Iterator<Item = &'a Triple> {
        self.iter().filter(move |t| t.predicate() == predicate)
    }

    pub fn triples_for_object<'a>(&'a self, object: &'a Term) -> impl Iterator<Item = &'a Triple> {
        self.iter().filter(move |t| t.object() == object)
    }

    pub fn triples_for_subject_predicate<'a>(
        &'a self,
        subject: &'a NamedOrBlankNode,
        predicate: &'a NamedNode,
    ) -> impl Iterator<Item = &'a Triple> {
        self.iter()
            .filter(move |t| t.subject() == subject && t.predicate() == predicate)
    }

    pub fn objects_for_subject_predicate<'a>(
        &'a self,
        subject: &'a NamedOrBlankNode,
        predicate: &'a NamedNode,
    ) -> impl Iterator<Item = &'a Term> {
        self.triples_for_subject_predicate(subject, predicate)
            .map(|t| t.object())
    }

    pub fn object_for_subject_predicate<'a>(
        &'a self,
        subject: &'a NamedOrBlankNode,
        predicate: &'a NamedNode,
    ) -> Option<&'a Term> {
        self.objects_for_subject_predicate(subject, predicate)
            .nth(0)
    }

    pub fn triples_for_predicate_object<'a>(
        &'a self,
        predicate: &'a NamedNode,
        object: &'a Term,
    ) -> impl Iterator<Item = &'a Triple> {
        self.iter()
            .filter(move |t| t.predicate() == predicate && t.object() == object)
    }

    pub fn subjects_for_predicate_object<'a>(
        &'a self,
        predicate: &'a NamedNode,
        object: &'a Term,
    ) -> impl Iterator<Item = &'a NamedOrBlankNode> {
        self.triples_for_predicate_object(predicate, object)
            .map(|t| t.subject())
    }

    pub fn subject_for_predicate_object<'a>(
        &'a self,
        predicate: &'a NamedNode,
        object: &'a Term,
    ) -> Option<&'a NamedOrBlankNode> {
        self.subjects_for_predicate_object(predicate, object).nth(0)
    }

    pub fn values_for_list<'a>(&'a self, root: NamedOrBlankNode) -> ListIterator<'a> {
        ListIterator {
            graph: self,
            current_node: Some(root),
        }
    }

    pub fn len(&self) -> usize {
        self.triples.len()
    }

    pub fn is_empty(&self) -> bool {
        self.triples.is_empty()
    }

    pub fn contains(&self, value: &Triple) -> bool {
        self.triples.contains(value)
    }

    pub fn insert(&mut self, value: Triple) -> bool {
        self.triples.insert(value)
    }
}

impl fmt::Display for MemoryGraph {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        for triple in &self.triples {
            write!(fmt, "{}\n", triple)?;
        }
        Ok(())
    }
}

impl IntoIterator for MemoryGraph {
    type Item = Triple;
    type IntoIter = <HashSet<Triple> as IntoIterator>::IntoIter;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.triples.into_iter()
    }
}

impl<'a> IntoIterator for &'a MemoryGraph {
    type Item = &'a Triple;
    type IntoIter = <&'a HashSet<Triple> as IntoIterator>::IntoIter;

    fn into_iter(self) -> <Self as IntoIterator>::IntoIter {
        self.triples.iter()
    }
}

impl FromIterator<Triple> for MemoryGraph {
    fn from_iter<I: IntoIterator<Item = Triple>>(iter: I) -> Self {
        let triples = HashSet::from_iter(iter);
        Self { triples }
    }
}

impl Extend<Triple> for MemoryGraph {
    fn extend<I: IntoIterator<Item = Triple>>(&mut self, iter: I) {
        self.triples.extend(iter)
    }
}

impl<'a> Extend<&'a Triple> for MemoryGraph {
    fn extend<I: IntoIterator<Item = &'a Triple>>(&mut self, iter: I) {
        self.triples.extend(iter.into_iter().cloned())
    }
}

pub struct ListIterator<'a> {
    graph: &'a MemoryGraph,
    current_node: Option<NamedOrBlankNode>,
}

impl<'a> Iterator for ListIterator<'a> {
    type Item = Term;

    fn next(&mut self) -> Option<Term> {
        match self.current_node.clone() {
            Some(current) => {
                let result = self.graph
                    .object_for_subject_predicate(&current, &rdf::FIRST)?
                    .clone();
                self.current_node = match self.graph
                    .object_for_subject_predicate(&current, &rdf::REST)
                {
                    Some(Term::NamedNode(n)) if *n == *rdf::NIL => None,
                    Some(Term::NamedNode(n)) => Some(n.clone().into()),
                    Some(Term::BlankNode(n)) => Some(n.clone().into()),
                    _ => None,
                };
                Some(result)
            }
            None => None,
        }
    }
}
