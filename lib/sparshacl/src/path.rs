//! SHACL Property Path implementation.
//!
//! Implements property paths as defined in the SHACL specification:
//! - Predicate path (simple IRI)
//! - Sequence path (list of paths)
//! - Alternative path (sh:alternativePath)
//! - Inverse path (sh:inversePath)
//! - Zero-or-more path (sh:zeroOrMorePath)
//! - One-or-more path (sh:oneOrMorePath)
//! - Zero-or-one path (sh:zeroOrOnePath)

use oxrdf::{vocab::shacl, Graph, NamedNode, NamedNodeRef, Term, TermRef};
use rustc_hash::FxHashSet;
use std::fmt;

use crate::error::ShaclParseError;

/// Represents a SHACL property path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PropertyPath {
    /// A simple predicate path (IRI).
    Predicate(NamedNode),

    /// A sequence of paths (executed in order).
    Sequence(Vec<PropertyPath>),

    /// Alternative paths (any one can match).
    Alternative(Vec<PropertyPath>),

    /// Inverse path (traverse in reverse direction).
    Inverse(Box<PropertyPath>),

    /// Zero or more repetitions of the path.
    ZeroOrMore(Box<PropertyPath>),

    /// One or more repetitions of the path.
    OneOrMore(Box<PropertyPath>),

    /// Zero or one occurrence of the path.
    ZeroOrOne(Box<PropertyPath>),
}

impl PropertyPath {
    /// Creates a predicate path from a named node.
    pub fn predicate(predicate: impl Into<NamedNode>) -> Self {
        Self::Predicate(predicate.into())
    }

    /// Creates a sequence path from a list of paths.
    pub fn sequence(paths: Vec<PropertyPath>) -> Self {
        Self::Sequence(paths)
    }

    /// Creates an alternative path from a list of paths.
    pub fn alternative(paths: Vec<PropertyPath>) -> Self {
        Self::Alternative(paths)
    }

    /// Creates an inverse path.
    pub fn inverse(path: PropertyPath) -> Self {
        Self::Inverse(Box::new(path))
    }

    /// Creates a zero-or-more path.
    pub fn zero_or_more(path: PropertyPath) -> Self {
        Self::ZeroOrMore(Box::new(path))
    }

    /// Creates a one-or-more path.
    pub fn one_or_more(path: PropertyPath) -> Self {
        Self::OneOrMore(Box::new(path))
    }

    /// Creates a zero-or-one path.
    pub fn zero_or_one(path: PropertyPath) -> Self {
        Self::ZeroOrOne(Box::new(path))
    }

    /// Parses a property path from a term in an RDF graph.
    pub fn parse(graph: &Graph, term: TermRef<'_>) -> Result<Self, ShaclParseError> {
        match term {
            // Simple predicate path (IRI)
            TermRef::NamedNode(node) => Ok(Self::Predicate(node.into_owned())),

            // Complex path (blank node with path operators)
            TermRef::BlankNode(bnode) => {
                let bnode_term: Term = bnode.into_owned().into();

                // Check for alternative path
                if let Some(list_head) = get_object(graph, &bnode_term, shacl::ALTERNATIVE_PATH) {
                    let paths = parse_path_list(graph, list_head, &bnode_term)?;
                    return Ok(Self::Alternative(paths));
                }

                // Check for inverse path
                if let Some(inner) = get_object(graph, &bnode_term, shacl::INVERSE_PATH) {
                    let inner_path = Self::parse(graph, inner.as_ref())?;
                    return Ok(Self::Inverse(Box::new(inner_path)));
                }

                // Check for zero-or-more path
                if let Some(inner) = get_object(graph, &bnode_term, shacl::ZERO_OR_MORE_PATH) {
                    let inner_path = Self::parse(graph, inner.as_ref())?;
                    return Ok(Self::ZeroOrMore(Box::new(inner_path)));
                }

                // Check for one-or-more path
                if let Some(inner) = get_object(graph, &bnode_term, shacl::ONE_OR_MORE_PATH) {
                    let inner_path = Self::parse(graph, inner.as_ref())?;
                    return Ok(Self::OneOrMore(Box::new(inner_path)));
                }

                // Check for zero-or-one path
                if let Some(inner) = get_object(graph, &bnode_term, shacl::ZERO_OR_ONE_PATH) {
                    let inner_path = Self::parse(graph, inner.as_ref())?;
                    return Ok(Self::ZeroOrOne(Box::new(inner_path)));
                }

                // Check for sequence path (RDF list starting from this blank node)
                if is_rdf_list_head(graph, &bnode_term) {
                    let paths = parse_path_list(graph, bnode_term.clone(), &bnode_term)?;
                    if paths.len() >= 2 {
                        return Ok(Self::Sequence(paths));
                    }
                }

                Err(ShaclParseError::invalid_property_path(
                    bnode_term,
                    "Unknown property path structure",
                ))
            }

            _ => Err(ShaclParseError::invalid_property_path(
                term.into_owned(),
                "Property path must be an IRI or blank node",
            )),
        }
    }

    /// Evaluates the property path starting from a focus node and returns all value nodes.
    pub fn evaluate<'a>(
        &self,
        graph: &'a Graph,
        focus_node: TermRef<'a>,
    ) -> Vec<Term> {
        let mut results = Vec::new();
        self.evaluate_into(graph, focus_node, &mut results, &mut FxHashSet::default(), 0);
        results
    }

    fn evaluate_into<'a>(
        &self,
        graph: &'a Graph,
        focus_node: TermRef<'a>,
        results: &mut Vec<Term>,
        visited: &mut FxHashSet<Term>,
        depth: usize,
    ) {
        // Prevent infinite recursion
        const MAX_DEPTH: usize = 100;
        if depth > MAX_DEPTH {
            return;
        }

        match self {
            Self::Predicate(predicate) => {
                // Get all objects where focus_node is the subject
                if let TermRef::NamedNode(subj) = focus_node {
                    for obj in graph.objects_for_subject_predicate(subj, predicate) {
                        results.push(obj.into_owned());
                    }
                } else if let TermRef::BlankNode(subj) = focus_node {
                    for obj in graph.objects_for_subject_predicate(subj, predicate) {
                        results.push(obj.into_owned());
                    }
                }
            }

            Self::Sequence(paths) => {
                if paths.is_empty() {
                    results.push(focus_node.into_owned());
                    return;
                }

                let mut current_nodes = vec![focus_node.into_owned()];

                for path in paths {
                    let mut next_nodes = Vec::new();
                    for node in &current_nodes {
                        path.evaluate_into(graph, node.as_ref(), &mut next_nodes, visited, depth + 1);
                    }
                    current_nodes = next_nodes;
                }

                results.extend(current_nodes);
            }

            Self::Alternative(paths) => {
                for path in paths {
                    path.evaluate_into(graph, focus_node, results, visited, depth + 1);
                }
            }

            Self::Inverse(inner) => {
                // For inverse, we need to find subjects where focus_node is the object
                if let Self::Predicate(predicate) = inner.as_ref() {
                    for subj in graph.subjects_for_predicate_object(predicate, focus_node) {
                        results.push(subj.into_owned().into());
                    }
                } else {
                    // For complex inverse paths, we need to iterate all triples
                    // This is less efficient but correct
                    for triple in graph.iter() {
                        let mut temp_results = Vec::new();
                        inner.evaluate_into(
                            graph,
                            triple.subject.into(),
                            &mut temp_results,
                            visited,
                            depth + 1,
                        );
                        if temp_results.iter().any(|r| r.as_ref() == focus_node) {
                            results.push(triple.subject.into_owned().into());
                        }
                    }
                }
            }

            Self::ZeroOrMore(inner) => {
                // Include the focus node itself (zero repetitions)
                let focus_owned = focus_node.into_owned();
                if visited.insert(focus_owned.clone()) {
                    results.push(focus_owned.clone());

                    // Recursively follow the path
                    let mut temp_results = Vec::new();
                    inner.evaluate_into(graph, focus_node, &mut temp_results, visited, depth + 1);

                    for node in temp_results {
                        if visited.insert(node.clone()) {
                            results.push(node.clone());
                            self.evaluate_into(graph, node.as_ref(), results, visited, depth + 1);
                        }
                    }
                }
            }

            Self::OneOrMore(inner) => {
                // Don't include the focus node itself (one or more repetitions required)
                let _focus_owned = focus_node.into_owned();

                let mut temp_results = Vec::new();
                inner.evaluate_into(graph, focus_node, &mut temp_results, visited, depth + 1);

                for node in temp_results {
                    if visited.insert(node.clone()) {
                        results.push(node.clone());
                        // Continue with zero-or-more from here
                        let zero_or_more = Self::ZeroOrMore(inner.clone());
                        zero_or_more.evaluate_into(graph, node.as_ref(), results, visited, depth + 1);
                    }
                }
            }

            Self::ZeroOrOne(inner) => {
                // Include the focus node itself
                results.push(focus_node.into_owned());

                // Also include one step
                inner.evaluate_into(graph, focus_node, results, visited, depth + 1);
            }
        }
    }

    /// Returns true if this is a simple predicate path.
    pub fn is_predicate(&self) -> bool {
        matches!(self, Self::Predicate(_))
    }

    /// Returns the predicate if this is a simple predicate path.
    pub fn as_predicate(&self) -> Option<&NamedNode> {
        match self {
            Self::Predicate(p) => Some(p),
            _ => None,
        }
    }
}

impl fmt::Display for PropertyPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Predicate(p) => write!(f, "<{}>", p.as_str()),
            Self::Sequence(paths) => {
                write!(f, "(")?;
                for (i, p) in paths.iter().enumerate() {
                    if i > 0 {
                        write!(f, " / ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ")")
            }
            Self::Alternative(paths) => {
                write!(f, "(")?;
                for (i, p) in paths.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ")")
            }
            Self::Inverse(p) => write!(f, "^{p}"),
            Self::ZeroOrMore(p) => write!(f, "{p}*"),
            Self::OneOrMore(p) => write!(f, "{p}+"),
            Self::ZeroOrOne(p) => write!(f, "{p}?"),
        }
    }
}

// Helper functions

fn get_object(graph: &Graph, subject: &Term, predicate: NamedNodeRef<'_>) -> Option<Term> {
    match subject {
        Term::NamedNode(n) => graph
            .object_for_subject_predicate(n, predicate)
            .map(|t| t.into_owned()),
        Term::BlankNode(b) => graph
            .object_for_subject_predicate(b, predicate)
            .map(|t| t.into_owned()),
        _ => None,
    }
}

fn is_rdf_list_head(graph: &Graph, term: &Term) -> bool {
    use oxrdf::vocab::rdf;
    get_object(graph, term, rdf::FIRST).is_some()
}

fn parse_path_list(
    graph: &Graph,
    list_head: Term,
    shape: &Term,
) -> Result<Vec<PropertyPath>, ShaclParseError> {
    use oxrdf::vocab::rdf;

    let mut paths = Vec::new();
    let mut current = list_head;

    loop {
        // Check for nil (end of list)
        if let Term::NamedNode(n) = &current {
            if n.as_ref() == rdf::NIL {
                break;
            }
        }

        // Get first element
        let first = get_object(graph, &current, rdf::FIRST).ok_or_else(|| {
            ShaclParseError::invalid_rdf_list(shape.clone(), "Missing rdf:first")
        })?;

        // Parse the path element
        let path = PropertyPath::parse(graph, first.as_ref())?;
        paths.push(path);

        // Get rest of list
        let rest = get_object(graph, &current, rdf::REST).ok_or_else(|| {
            ShaclParseError::invalid_rdf_list(shape.clone(), "Missing rdf:rest")
        })?;

        current = rest;
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::Triple;

    #[test]
    fn test_predicate_path() {
        let mut graph = Graph::new();
        let s = NamedNode::new("http://example.org/s").unwrap();
        let p = NamedNode::new("http://example.org/p").unwrap();
        let o = NamedNode::new("http://example.org/o").unwrap();

        graph.insert(&Triple::new(s.clone(), p.clone(), o.clone()));

        let path = PropertyPath::predicate(p);
        let results = path.evaluate(&graph, s.as_ref().into());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], Term::NamedNode(o));
    }

    #[test]
    fn test_inverse_path() {
        let mut graph = Graph::new();
        let s = NamedNode::new("http://example.org/s").unwrap();
        let p = NamedNode::new("http://example.org/p").unwrap();
        let o = NamedNode::new("http://example.org/o").unwrap();

        graph.insert(&Triple::new(s.clone(), p.clone(), o.clone()));

        let path = PropertyPath::inverse(PropertyPath::predicate(p));
        let results = path.evaluate(&graph, o.as_ref().into());

        assert_eq!(results.len(), 1);
        assert_eq!(results[0], Term::NamedNode(s));
    }
}
