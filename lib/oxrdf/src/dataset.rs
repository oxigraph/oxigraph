//! [In-memory implementation](super::Dataset) of [RDF datasets](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
//!
//! Usage example:
//! ```
//! use oxrdf::*;
//!
//! let mut dataset = Dataset::default();
//!
//! // insertion
//! let ex = NamedNodeRef::new("http://example.com")?;
//! let quad = QuadRef::new(ex, ex, ex, ex);
//! dataset.insert(quad);
//!
//! // simple filter
//! let results: Vec<_> = dataset.quads_for_subject(ex).collect();
//! assert_eq!(vec![quad], results);
//!
//! // direct access to a dataset graph
//! let results: Vec<_> = dataset.graph(ex).iter().collect();
//! assert_eq!(vec![TripleRef::new(ex, ex, ex)], results);
//! # Result::<_,Box<dyn std::error::Error>>::Ok(())
//! ```
//!
//! See also [`Graph`](super::Graph) if you only care about plain triples.

use crate::interning::*;
use crate::SubjectRef;
use crate::*;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::hash::{Hash, Hasher};

/// An in-memory [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
///
/// It can accommodate a fairly large number of quads (in the few millions).
/// Beware: it interns the string and does not do any garbage collection yet:
/// if you insert and remove a lot of different terms, memory will grow without any reduction.
///
/// Usage example:
/// ```
/// use oxrdf::*;
///
/// let mut dataset = Dataset::default();
///
/// // insertion
/// let ex = NamedNodeRef::new("http://example.com")?;
/// let quad = QuadRef::new(ex, ex, ex, ex);
/// dataset.insert(quad);
///
/// // simple filter
/// let results: Vec<_> = dataset.quads_for_subject(ex).collect();
/// assert_eq!(vec![quad], results);
///
/// // direct access to a dataset graph
/// let results: Vec<_> = dataset.graph(ex).iter().collect();
/// assert_eq!(vec![TripleRef::new(ex, ex, ex)], results);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Debug, Default)]
pub struct Dataset {
    interner: Interner,
    gspo: BTreeSet<(
        InternedGraphName,
        InternedSubject,
        InternedNamedNode,
        InternedTerm,
    )>,
    gpos: BTreeSet<(
        InternedGraphName,
        InternedNamedNode,
        InternedTerm,
        InternedSubject,
    )>,
    gosp: BTreeSet<(
        InternedGraphName,
        InternedTerm,
        InternedSubject,
        InternedNamedNode,
    )>,
    spog: BTreeSet<(
        InternedSubject,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    )>,
    posg: BTreeSet<(
        InternedNamedNode,
        InternedTerm,
        InternedSubject,
        InternedGraphName,
    )>,
    ospg: BTreeSet<(
        InternedTerm,
        InternedSubject,
        InternedNamedNode,
        InternedGraphName,
    )>,
}

impl Dataset {
    /// Creates a new dataset
    pub fn new() -> Self {
        Self::default()
    }

    /// Provides a read-only view on an [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) contained in this dataset.
    ///
    /// ```
    /// use oxrdf::*;
    ///
    /// let mut dataset = Dataset::default();
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// dataset.insert(QuadRef::new(ex, ex, ex, ex));
    ///
    /// let results: Vec<_> = dataset.graph(ex).iter().collect();
    /// assert_eq!(vec![TripleRef::new(ex, ex, ex)], results);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn graph<'a, 'b>(&'a self, graph_name: impl Into<GraphNameRef<'b>>) -> GraphView<'a> {
        let graph_name = self
            .encoded_graph_name(graph_name)
            .unwrap_or_else(InternedGraphName::impossible);
        GraphView {
            dataset: self,
            graph_name,
        }
    }

    /// Provides a read/write view on an [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) contained in this dataset.
    ///
    /// ```
    /// use oxrdf::*;
    ///
    /// let mut dataset = Dataset::default();
    /// let ex = NamedNodeRef::new("http://example.com")?;
    ///
    /// // We edit and query the dataset http://example.com graph
    /// {
    ///     let mut graph = dataset.graph_mut(ex);
    ///     graph.insert(TripleRef::new(ex, ex, ex));
    ///     let results: Vec<_> = graph.iter().collect();
    ///     assert_eq!(vec![TripleRef::new(ex, ex, ex)], results);
    /// }
    ///
    /// // We have also changes the dataset itself
    /// let results: Vec<_> = dataset.iter().collect();
    /// assert_eq!(vec![QuadRef::new(ex, ex, ex, ex)], results);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn graph_mut<'a, 'b>(
        &'a mut self,
        graph_name: impl Into<GraphNameRef<'b>>,
    ) -> GraphViewMut<'a> {
        let graph_name = InternedGraphName::encoded_into(graph_name.into(), &mut self.interner);
        GraphViewMut {
            dataset: self,
            graph_name,
        }
    }

    /// Returns all the quads contained by the dataset.
    pub fn iter(&self) -> Iter<'_> {
        let iter = self.spog.iter();
        Iter {
            dataset: self,
            inner: iter,
        }
    }

    pub fn quads_for_subject<'a, 'b>(
        &'a self,
        subject: impl Into<SubjectRef<'b>>,
    ) -> impl Iterator<Item = QuadRef<'a>> + 'a {
        let subject = self
            .encoded_subject(subject)
            .unwrap_or_else(InternedSubject::impossible);
        self.interned_quads_for_subject(&subject)
            .map(move |q| self.decode_spog(q))
    }

    fn interned_quads_for_subject(
        &self,
        subject: &InternedSubject,
    ) -> impl Iterator<
        Item = (
            &InternedSubject,
            &InternedNamedNode,
            &InternedTerm,
            &InternedGraphName,
        ),
    > + '_ {
        self.spog
            .range(
                &(
                    subject.clone(),
                    InternedNamedNode::first(),
                    InternedTerm::first(),
                    InternedGraphName::first(),
                )
                    ..&(
                        subject.next(),
                        InternedNamedNode::first(),
                        InternedTerm::first(),
                        InternedGraphName::first(),
                    ),
            )
            .map(|(s, p, o, g)| (s, p, o, g))
    }

    pub fn quads_for_predicate<'a, 'b>(
        &'a self,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = QuadRef<'a>> + 'a {
        let predicate = self
            .encoded_named_node(predicate)
            .unwrap_or_else(InternedNamedNode::impossible);
        self.interned_quads_for_predicate(predicate)
            .map(move |q| self.decode_spog(q))
    }

    fn interned_quads_for_predicate(
        &self,
        predicate: InternedNamedNode,
    ) -> impl Iterator<
        Item = (
            &InternedSubject,
            &InternedNamedNode,
            &InternedTerm,
            &InternedGraphName,
        ),
    > + '_ {
        self.posg
            .range(
                &(
                    predicate,
                    InternedTerm::first(),
                    InternedSubject::first(),
                    InternedGraphName::first(),
                )
                    ..&(
                        predicate.next(),
                        InternedTerm::first(),
                        InternedSubject::first(),
                        InternedGraphName::first(),
                    ),
            )
            .map(|(p, o, s, g)| (s, p, o, g))
    }

    pub fn quads_for_object<'a, 'b>(
        &'a self,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = QuadRef<'a>> + 'a {
        let object = self
            .encoded_term(object)
            .unwrap_or_else(InternedTerm::impossible);

        self.interned_quads_for_object(&object)
            .map(move |q| self.decode_spog(q))
    }

    fn interned_quads_for_object(
        &self,
        object: &InternedTerm,
    ) -> impl Iterator<
        Item = (
            &InternedSubject,
            &InternedNamedNode,
            &InternedTerm,
            &InternedGraphName,
        ),
    > + '_ {
        self.ospg
            .range(
                &(
                    object.clone(),
                    InternedSubject::first(),
                    InternedNamedNode::first(),
                    InternedGraphName::first(),
                )
                    ..&(
                        object.next(),
                        InternedSubject::first(),
                        InternedNamedNode::first(),
                        InternedGraphName::first(),
                    ),
            )
            .map(|(o, s, p, g)| (s, p, o, g))
    }

    fn interned_quads_for_graph_name(
        &self,
        graph_name: &InternedGraphName,
    ) -> impl Iterator<
        Item = (
            &InternedSubject,
            &InternedNamedNode,
            &InternedTerm,
            &InternedGraphName,
        ),
    > + '_ {
        self.gspo
            .range(
                &(
                    graph_name.clone(),
                    InternedSubject::first(),
                    InternedNamedNode::first(),
                    InternedTerm::first(),
                )
                    ..&(
                        graph_name.next(),
                        InternedSubject::first(),
                        InternedNamedNode::first(),
                        InternedTerm::first(),
                    ),
            )
            .map(|(g, s, p, o)| (s, p, o, g))
    }

    /// Checks if the dataset contains the given quad
    pub fn contains<'a>(&self, quad: impl Into<QuadRef<'a>>) -> bool {
        if let Some(q) = self.encoded_quad(quad.into()) {
            self.spog.contains(&q)
        } else {
            false
        }
    }

    /// Returns the number of quads in this dataset.
    pub fn len(&self) -> usize {
        self.gspo.len()
    }

    /// Checks if this dataset contains a quad.
    pub fn is_empty(&self) -> bool {
        self.gspo.is_empty()
    }

    /// Adds a quad to the dataset.
    pub fn insert<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> bool {
        let quad = self.encode_quad(quad.into());
        self.insert_encoded(quad)
    }

    fn insert_encoded(
        &mut self,
        quad: (
            InternedSubject,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
    ) -> bool {
        let (s, p, o, g) = quad;
        self.gspo.insert((g.clone(), s.clone(), p, o.clone()));
        self.gpos.insert((g.clone(), p, o.clone(), s.clone()));
        self.gosp.insert((g.clone(), o.clone(), s.clone(), p));
        self.spog.insert((s.clone(), p, o.clone(), g.clone()));
        self.posg.insert((p, o.clone(), s.clone(), g.clone()));
        self.ospg.insert((o, s, p, g))
    }

    /// Removes a concrete quad from the dataset.
    pub fn remove<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> bool {
        if let Some(quad) = self.encoded_quad(quad.into()) {
            self.remove_encoded(quad)
        } else {
            false
        }
    }

    fn remove_encoded(
        &mut self,
        quad: (
            InternedSubject,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
    ) -> bool {
        let (s, p, o, g) = quad;
        self.gspo.remove(&(g.clone(), s.clone(), p, o.clone()));
        self.gpos.remove(&(g.clone(), p, o.clone(), s.clone()));
        self.gosp.remove(&(g.clone(), o.clone(), s.clone(), p));
        self.spog.remove(&(s.clone(), p, o.clone(), g.clone()));
        self.posg.remove(&(p, o.clone(), s.clone(), g.clone()));
        self.ospg.remove(&(o, s, p, g))
    }

    /// Clears the dataset.
    pub fn clear(&mut self) {
        self.gspo.clear();
        self.gpos.clear();
        self.gosp.clear();
        self.spog.clear();
        self.posg.clear();
        self.ospg.clear();
    }

    fn encode_quad(
        &mut self,
        quad: QuadRef<'_>,
    ) -> (
        InternedSubject,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    ) {
        (
            InternedSubject::encoded_into(quad.subject, &mut self.interner),
            InternedNamedNode::encoded_into(quad.predicate, &mut self.interner),
            InternedTerm::encoded_into(quad.object, &mut self.interner),
            InternedGraphName::encoded_into(quad.graph_name, &mut self.interner),
        )
    }

    fn encoded_quad(
        &self,
        quad: QuadRef<'_>,
    ) -> Option<(
        InternedSubject,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    )> {
        Some((
            self.encoded_subject(quad.subject)?,
            self.encoded_named_node(quad.predicate)?,
            self.encoded_term(quad.object)?,
            self.encoded_graph_name(quad.graph_name)?,
        ))
    }

    pub(super) fn encoded_named_node<'a>(
        &self,
        node: impl Into<NamedNodeRef<'a>>,
    ) -> Option<InternedNamedNode> {
        InternedNamedNode::encoded_from(node.into(), &self.interner)
    }

    pub(super) fn encoded_subject<'a>(
        &self,
        node: impl Into<SubjectRef<'a>>,
    ) -> Option<InternedSubject> {
        InternedSubject::encoded_from(node.into(), &self.interner)
    }

    pub(super) fn encoded_term<'a>(&self, term: impl Into<TermRef<'a>>) -> Option<InternedTerm> {
        InternedTerm::encoded_from(term.into(), &self.interner)
    }

    pub(super) fn encoded_graph_name<'a>(
        &self,
        graph_name: impl Into<GraphNameRef<'a>>,
    ) -> Option<InternedGraphName> {
        InternedGraphName::encoded_from(graph_name.into(), &self.interner)
    }

    fn decode_spog(
        &self,
        quad: (
            &InternedSubject,
            &InternedNamedNode,
            &InternedTerm,
            &InternedGraphName,
        ),
    ) -> QuadRef<'_> {
        QuadRef {
            subject: quad.0.decode_from(&self.interner),
            predicate: quad.1.decode_from(&self.interner),
            object: quad.2.decode_from(&self.interner),
            graph_name: quad.3.decode_from(&self.interner),
        }
    }

    fn decode_spo(
        &self,
        triple: (&InternedSubject, &InternedNamedNode, &InternedTerm),
    ) -> TripleRef<'_> {
        TripleRef {
            subject: triple.0.decode_from(&self.interner),
            predicate: triple.1.decode_from(&self.interner),
            object: triple.2.decode_from(&self.interner),
        }
    }

    /// Applies on the dataset the canonicalization process described in
    /// [Canonical Forms for Isomorphic and Equivalent RDF Graphs: Algorithms for Leaning and Labelling Blank Nodes, Aidan Hogan, 2017](http://aidanhogan.com/docs/rdf-canonicalisation.pdf).
    ///
    /// Usage example ([Dataset isomorphism](https://www.w3.org/TR/rdf11-concepts/#dfn-dataset-isomorphism)):
    /// ```
    /// use oxrdf::*;
    ///
    /// let iri = NamedNodeRef::new("http://example.com")?;
    ///
    /// let mut graph1 = Graph::new();
    /// let bnode1 = BlankNode::default();
    /// let g1 = BlankNode::default();
    /// graph1.insert(QuadRef::new(iri, iri, &bnode1, &g1));
    /// graph1.insert(QuadRef::new(&bnode1, iri, iri, &g1));
    ///
    /// let mut graph2 = Graph::new();
    /// let bnode2 = BlankNode::default();
    /// let g2 = BlankNode::default();
    /// graph2.insert(QuadRef::new(iri, iri, &bnode2, &g2));
    /// graph2.insert(QuadRef::new(&bnode2, iri, iri, &g2));
    ///
    /// assert_ne!(graph1, graph2);
    /// graph1.canonicalize();
    /// graph2.canonicalize();
    /// assert_eq!(graph1, graph2);
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// Warning 1: Blank node ids depends on the current shape of the graph. Adding a new quad might change the ids of a lot of blank nodes.
    /// Hence, this canonization might not be suitable for diffs.
    ///
    /// Warning 2: The canonicalization algorithm is not stable and canonical blank node ids might change between Oxigraph version.
    ///
    /// Warning 3: This implementation worst-case complexity is in *O(b!)* with *b* the number of blank nodes in the input dataset.
    pub fn canonicalize(&mut self) {
        let bnodes = self.blank_nodes();
        let (hash, partition) =
            self.hash_bnodes(bnodes.into_iter().map(|bnode| (bnode, 0)).collect());
        let new_quads = self.distinguish(&hash, &partition);
        self.clear();
        for quad in new_quads {
            self.insert_encoded(quad);
        }
    }

    fn blank_nodes(&self) -> HashSet<InternedBlankNode> {
        let mut bnodes = HashSet::new();
        for (g, s, _, o) in &self.gspo {
            if let InternedSubject::BlankNode(bnode) = s {
                bnodes.insert(*bnode);
            }
            #[cfg(feature = "rdf-star")]
            if let InternedSubject::Triple(triple) = s {
                Self::triple_blank_nodes(triple, &mut bnodes);
            }
            if let InternedTerm::BlankNode(bnode) = o {
                bnodes.insert(*bnode);
            }
            #[cfg(feature = "rdf-star")]
            if let InternedTerm::Triple(triple) = o {
                Self::triple_blank_nodes(triple, &mut bnodes);
            }
            if let InternedGraphName::BlankNode(bnode) = g {
                bnodes.insert(*bnode);
            }
        }
        bnodes
    }

    #[cfg(feature = "rdf-star")]
    fn triple_blank_nodes(triple: &InternedTriple, bnodes: &mut HashSet<InternedBlankNode>) {
        if let InternedSubject::BlankNode(bnode) = &triple.subject {
            bnodes.insert(*bnode);
        } else if let InternedSubject::Triple(t) = &triple.subject {
            Self::triple_blank_nodes(t, bnodes);
        }
        if let InternedTerm::BlankNode(bnode) = &triple.object {
            bnodes.insert(*bnode);
        } else if let InternedTerm::Triple(t) = &triple.object {
            Self::triple_blank_nodes(t, bnodes);
        }
    }

    fn hash_bnodes(
        &self,
        mut hashes: HashMap<InternedBlankNode, u64>,
    ) -> (
        HashMap<InternedBlankNode, u64>,
        Vec<(u64, Vec<InternedBlankNode>)>,
    ) {
        let mut to_hash = Vec::new();
        let mut partition: HashMap<u64, Vec<InternedBlankNode>> = HashMap::new();
        let mut partition_len = 0;
        loop {
            //TODO: improve termination
            let mut new_hashes = HashMap::new();
            for (bnode, old_hash) in &hashes {
                for (_, p, o, g) in
                    self.interned_quads_for_subject(&InternedSubject::BlankNode(*bnode))
                {
                    to_hash.push((
                        self.hash_named_node(*p),
                        self.hash_term(o, &hashes),
                        self.hash_graph_name(g, &hashes),
                        0,
                    ));
                }
                for (s, p, _, g) in self.interned_quads_for_object(&InternedTerm::BlankNode(*bnode))
                {
                    to_hash.push((
                        self.hash_subject(s, &hashes),
                        self.hash_named_node(*p),
                        self.hash_graph_name(g, &hashes),
                        1,
                    ));
                }
                for (s, p, o, _) in
                    self.interned_quads_for_graph_name(&InternedGraphName::BlankNode(*bnode))
                {
                    to_hash.push((
                        self.hash_subject(s, &hashes),
                        self.hash_named_node(*p),
                        self.hash_term(o, &hashes),
                        2,
                    ));
                }
                to_hash.sort_unstable();
                let hash = Self::hash_tuple((old_hash, &to_hash));
                to_hash.clear();
                new_hashes.insert(*bnode, hash);
                partition.entry(hash).or_default().push(*bnode);
            }
            if partition.len() == partition_len {
                let mut partition: Vec<_> = partition.into_iter().collect();
                partition.sort_by(|(h1, b1), (h2, b2)| (b1.len(), h1).cmp(&(b2.len(), h2)));
                return (hashes, partition);
            }
            hashes = new_hashes;
            partition_len = partition.len();
            partition.clear();
        }
    }

    fn hash_named_node(&self, node: InternedNamedNode) -> u64 {
        Self::hash_tuple(node.decode_from(&self.interner))
    }

    fn hash_subject(
        &self,
        node: &InternedSubject,
        bnodes_hash: &HashMap<InternedBlankNode, u64>,
    ) -> u64 {
        #[cfg(feature = "rdf-star")]
        if let InternedSubject::Triple(triple) = node {
            return self.hash_triple(triple, bnodes_hash);
        }
        if let InternedSubject::BlankNode(bnode) = node {
            bnodes_hash[bnode]
        } else {
            Self::hash_tuple(node.decode_from(&self.interner))
        }
    }

    fn hash_term(&self, term: &InternedTerm, bnodes_hash: &HashMap<InternedBlankNode, u64>) -> u64 {
        #[cfg(feature = "rdf-star")]
        if let InternedTerm::Triple(triple) = term {
            return self.hash_triple(triple, bnodes_hash);
        }
        if let InternedTerm::BlankNode(bnode) = term {
            bnodes_hash[bnode]
        } else {
            Self::hash_tuple(term.decode_from(&self.interner))
        }
    }

    fn hash_graph_name(
        &self,
        graph_name: &InternedGraphName,
        bnodes_hash: &HashMap<InternedBlankNode, u64>,
    ) -> u64 {
        if let InternedGraphName::BlankNode(bnode) = graph_name {
            bnodes_hash[bnode]
        } else {
            Self::hash_tuple(graph_name.decode_from(&self.interner))
        }
    }

    #[cfg(feature = "rdf-star")]
    fn hash_triple(
        &self,
        triple: &InternedTriple,
        bnodes_hash: &HashMap<InternedBlankNode, u64>,
    ) -> u64 {
        Self::hash_tuple((
            self.hash_subject(&triple.subject, bnodes_hash),
            self.hash_named_node(triple.predicate),
            self.hash_term(&triple.object, bnodes_hash),
        ))
    }

    fn hash_tuple(v: impl Hash) -> u64 {
        let mut hasher = DefaultHasher::new();
        v.hash(&mut hasher);
        hasher.finish()
    }

    fn distinguish(
        &mut self,
        hash: &HashMap<InternedBlankNode, u64>,
        partition: &[(u64, Vec<InternedBlankNode>)],
    ) -> Vec<(
        InternedSubject,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    )> {
        let b_prime = partition
            .iter()
            .find_map(|(_, b)| if b.len() > 1 { Some(b) } else { None });
        if let Some(b_prime) = b_prime {
            b_prime
                .iter()
                .map(|b| {
                    let mut hash_prime = hash.clone();
                    hash_prime.insert(*b, Self::hash_tuple((hash_prime[b], 22)));
                    let (hash_prime_prime, partition_prime) = self.hash_bnodes(hash_prime);
                    self.distinguish(&hash_prime_prime, &partition_prime)
                })
                .fold(None, |a, b| {
                    Some(if let Some(a) = a {
                        if a <= b {
                            a
                        } else {
                            b
                        }
                    } else {
                        b
                    })
                })
                .unwrap_or_default()
        } else {
            self.label(hash)
        }
    }

    #[allow(clippy::needless_collect)]
    fn label(
        &mut self,
        hashes: &HashMap<InternedBlankNode, u64>,
    ) -> Vec<(
        InternedSubject,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    )> {
        let old_quads: Vec<_> = self.spog.iter().cloned().collect();
        let mut quads: Vec<_> = old_quads
            .into_iter()
            .map(|(s, p, o, g)| {
                (
                    if let InternedSubject::BlankNode(bnode) = s {
                        InternedSubject::BlankNode(self.map_bnode(bnode, hashes))
                    } else {
                        #[cfg(feature = "rdf-star")]
                        {
                            if let InternedSubject::Triple(triple) = s {
                                InternedSubject::Triple(Box::new(InternedTriple::encoded_into(
                                    self.label_triple(&triple, hashes).as_ref(),
                                    &mut self.interner,
                                )))
                            } else {
                                s
                            }
                        }
                        #[cfg(not(feature = "rdf-star"))]
                        {
                            s
                        }
                    },
                    p,
                    if let InternedTerm::BlankNode(bnode) = o {
                        InternedTerm::BlankNode(self.map_bnode(bnode, hashes))
                    } else {
                        #[cfg(feature = "rdf-star")]
                        {
                            if let InternedTerm::Triple(triple) = o {
                                InternedTerm::Triple(Box::new(InternedTriple::encoded_into(
                                    self.label_triple(&triple, hashes).as_ref(),
                                    &mut self.interner,
                                )))
                            } else {
                                o
                            }
                        }
                        #[cfg(not(feature = "rdf-star"))]
                        {
                            o
                        }
                    },
                    if let InternedGraphName::BlankNode(bnode) = g {
                        InternedGraphName::BlankNode(self.map_bnode(bnode, hashes))
                    } else {
                        g
                    },
                )
            })
            .collect();
        quads.sort();
        quads
    }

    #[cfg(feature = "rdf-star")]
    fn label_triple(
        &mut self,
        triple: &InternedTriple,
        hashes: &HashMap<InternedBlankNode, u64>,
    ) -> Triple {
        Triple {
            subject: if let InternedSubject::BlankNode(bnode) = &triple.subject {
                Self::gen_bnode(*bnode, hashes).into()
            } else if let InternedSubject::Triple(t) = &triple.subject {
                self.label_triple(t, hashes).into()
            } else {
                triple.subject.decode_from(&self.interner).into_owned()
            },
            predicate: triple.predicate.decode_from(&self.interner).into_owned(),
            object: if let InternedTerm::BlankNode(bnode) = &triple.object {
                Self::gen_bnode(*bnode, hashes).into()
            } else if let InternedTerm::Triple(t) = &triple.object {
                self.label_triple(t, hashes).into()
            } else {
                triple.object.decode_from(&self.interner).into_owned()
            },
        }
    }

    fn map_bnode(
        &mut self,
        old_bnode: InternedBlankNode,
        hashes: &HashMap<InternedBlankNode, u64>,
    ) -> InternedBlankNode {
        InternedBlankNode::encoded_into(
            Self::gen_bnode(old_bnode, hashes).as_ref(),
            &mut self.interner,
        )
    }

    fn gen_bnode(
        old_bnode: InternedBlankNode,
        hashes: &HashMap<InternedBlankNode, u64>,
    ) -> BlankNode {
        BlankNode::new_from_unique_id(hashes[&old_bnode].into())
    }
}

impl PartialEq for Dataset {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        for q in self {
            if !other.contains(q) {
                return false;
            }
        }
        true
    }
}

impl Eq for Dataset {}

impl<'a> IntoIterator for &'a Dataset {
    type Item = QuadRef<'a>;
    type IntoIter = Iter<'a>;

    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

impl FromIterator<Quad> for Dataset {
    fn from_iter<I: IntoIterator<Item = Quad>>(iter: I) -> Self {
        let mut g = Self::new();
        g.extend(iter);
        g
    }
}

impl<'a, T: Into<QuadRef<'a>>> FromIterator<T> for Dataset {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut g = Self::new();
        g.extend(iter);
        g
    }
}

impl Extend<Quad> for Dataset {
    fn extend<I: IntoIterator<Item = Quad>>(&mut self, iter: I) {
        for t in iter {
            self.insert(&t);
        }
    }
}

impl<'a, T: Into<QuadRef<'a>>> Extend<T> for Dataset {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for t in iter {
            self.insert(t);
        }
    }
}

impl fmt::Display for Dataset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self {
            writeln!(f, "{t}")?;
        }
        Ok(())
    }
}

/// A read-only view on an [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) contained in a [`Dataset`].
///
/// It is built using the [`Dataset::graph`] method.
///
/// Usage example:
/// ```
/// use oxrdf::*;
///
/// let mut dataset = Dataset::default();
/// let ex = NamedNodeRef::new("http://example.com")?;
/// dataset.insert(QuadRef::new(ex, ex, ex, ex));
///
/// let results: Vec<_> = dataset.graph(ex).iter().collect();
/// assert_eq!(vec![TripleRef::new(ex, ex, ex)], results);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone, Debug)]
pub struct GraphView<'a> {
    dataset: &'a Dataset,
    graph_name: InternedGraphName,
}

impl<'a> GraphView<'a> {
    /// Returns all the triples contained by the graph.
    pub fn iter(&self) -> GraphViewIter<'a> {
        let iter = self.dataset.gspo.range(
            &(
                self.graph_name.clone(),
                InternedSubject::first(),
                InternedNamedNode::first(),
                InternedTerm::first(),
            )
                ..&(
                    self.graph_name.next(),
                    InternedSubject::first(),
                    InternedNamedNode::first(),
                    InternedTerm::first(),
                ),
        );
        GraphViewIter {
            dataset: self.dataset,
            inner: iter,
        }
    }

    pub fn triples_for_subject<'b>(
        &self,
        subject: impl Into<SubjectRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.triples_for_interned_subject(self.dataset.encoded_subject(subject))
    }

    pub(super) fn triples_for_interned_subject(
        &self,
        subject: Option<InternedSubject>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        let subject = subject.unwrap_or_else(InternedSubject::impossible);
        let ds = self.dataset;
        self.dataset
            .gspo
            .range(
                &(
                    self.graph_name.clone(),
                    subject.clone(),
                    InternedNamedNode::first(),
                    InternedTerm::first(),
                )
                    ..&(
                        self.graph_name.clone(),
                        subject.next(),
                        InternedNamedNode::first(),
                        InternedTerm::first(),
                    ),
            )
            .map(move |q| {
                let (_, s, p, o) = q;
                ds.decode_spo((s, p, o))
            })
    }

    pub fn objects_for_subject_predicate<'b>(
        &self,
        subject: impl Into<SubjectRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TermRef<'a>> + 'a {
        self.objects_for_interned_subject_predicate(
            self.dataset.encoded_subject(subject),
            self.dataset.encoded_named_node(predicate),
        )
    }

    pub(super) fn objects_for_interned_subject_predicate(
        &self,
        subject: Option<InternedSubject>,
        predicate: Option<InternedNamedNode>,
    ) -> impl Iterator<Item = TermRef<'a>> + 'a {
        let subject = subject.unwrap_or_else(InternedSubject::impossible);
        let predicate = predicate.unwrap_or_else(InternedNamedNode::impossible);
        let ds = self.dataset;
        self.dataset
            .gspo
            .range(
                &(
                    self.graph_name.clone(),
                    subject.clone(),
                    predicate,
                    InternedTerm::first(),
                )
                    ..&(
                        self.graph_name.clone(),
                        subject,
                        predicate.next(),
                        InternedTerm::first(),
                    ),
            )
            .map(move |q| q.3.decode_from(&ds.interner))
    }

    pub fn object_for_subject_predicate<'b>(
        &self,
        subject: impl Into<SubjectRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> Option<TermRef<'a>> {
        self.objects_for_subject_predicate(subject, predicate)
            .next()
    }

    pub fn predicates_for_subject_object<'b>(
        &self,
        subject: impl Into<SubjectRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = NamedNodeRef<'a>> + 'a {
        self.predicates_for_interned_subject_object(
            self.dataset.encoded_subject(subject),
            self.dataset.encoded_term(object),
        )
    }

    pub(super) fn predicates_for_interned_subject_object(
        &self,
        subject: Option<InternedSubject>,
        object: Option<InternedTerm>,
    ) -> impl Iterator<Item = NamedNodeRef<'a>> + 'a {
        let subject = subject.unwrap_or_else(InternedSubject::impossible);
        let object = object.unwrap_or_else(InternedTerm::impossible);
        let ds = self.dataset;
        self.dataset
            .gosp
            .range(
                &(
                    self.graph_name.clone(),
                    object.clone(),
                    subject.clone(),
                    InternedNamedNode::first(),
                )
                    ..&(
                        self.graph_name.clone(),
                        object,
                        subject.next(),
                        InternedNamedNode::first(),
                    ),
            )
            .map(move |q| q.3.decode_from(&ds.interner))
    }

    pub fn triples_for_predicate<'b>(
        &self,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.triples_for_interned_predicate(self.dataset.encoded_named_node(predicate))
    }

    pub(super) fn triples_for_interned_predicate(
        &self,
        predicate: Option<InternedNamedNode>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        let predicate = predicate.unwrap_or_else(InternedNamedNode::impossible);
        let ds = self.dataset;
        self.dataset
            .gpos
            .range(
                &(
                    self.graph_name.clone(),
                    predicate,
                    InternedTerm::first(),
                    InternedSubject::first(),
                )
                    ..&(
                        self.graph_name.clone(),
                        predicate.next(),
                        InternedTerm::first(),
                        InternedSubject::first(),
                    ),
            )
            .map(move |(_, p, o, s)| ds.decode_spo((s, p, o)))
    }

    pub fn subjects_for_predicate_object<'b>(
        &self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = SubjectRef<'a>> + 'a {
        self.subjects_for_interned_predicate_object(
            self.dataset.encoded_named_node(predicate),
            self.dataset.encoded_term(object),
        )
    }

    pub(super) fn subjects_for_interned_predicate_object(
        &self,
        predicate: Option<InternedNamedNode>,
        object: Option<InternedTerm>,
    ) -> impl Iterator<Item = SubjectRef<'a>> + 'a {
        let predicate = predicate.unwrap_or_else(InternedNamedNode::impossible);
        let object = object.unwrap_or_else(InternedTerm::impossible);
        let ds = self.dataset;
        self.dataset
            .gpos
            .range(
                &(
                    self.graph_name.clone(),
                    predicate,
                    object.clone(),
                    InternedSubject::first(),
                )
                    ..&(
                        self.graph_name.clone(),
                        predicate,
                        object.next(),
                        InternedSubject::first(),
                    ),
            )
            .map(move |q| q.3.decode_from(&ds.interner))
    }

    pub fn subject_for_predicate_object<'b>(
        &self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> Option<SubjectRef<'a>> {
        self.subjects_for_predicate_object(predicate, object).next()
    }

    pub fn triples_for_object<'b>(
        &self,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.triples_for_interned_object(self.dataset.encoded_term(object))
    }

    pub(super) fn triples_for_interned_object(
        &self,
        object: Option<InternedTerm>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        let object = object.unwrap_or_else(InternedTerm::impossible);
        let ds = self.dataset;
        self.dataset
            .gosp
            .range(
                &(
                    self.graph_name.clone(),
                    object.clone(),
                    InternedSubject::first(),
                    InternedNamedNode::first(),
                )
                    ..&(
                        self.graph_name.clone(),
                        object.next(),
                        InternedSubject::first(),
                        InternedNamedNode::first(),
                    ),
            )
            .map(move |(_, o, s, p)| ds.decode_spo((s, p, o)))
    }

    /// Checks if the graph contains the given triple.
    pub fn contains<'b>(&self, triple: impl Into<TripleRef<'b>>) -> bool {
        if let Some(triple) = self.encoded_triple(triple.into()) {
            self.dataset.gspo.contains(&(
                self.graph_name.clone(),
                triple.subject,
                triple.predicate,
                triple.object,
            ))
        } else {
            false
        }
    }

    /// Returns the number of triples in this graph.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Checks if this graph contains a triple.
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    fn encoded_triple(&self, triple: TripleRef<'_>) -> Option<InternedTriple> {
        Some(InternedTriple {
            subject: self.dataset.encoded_subject(triple.subject)?,
            predicate: self.dataset.encoded_named_node(triple.predicate)?,
            object: self.dataset.encoded_term(triple.object)?,
        })
    }
}

impl<'a> IntoIterator for GraphView<'a> {
    type Item = TripleRef<'a>;
    type IntoIter = GraphViewIter<'a>;

    fn into_iter(self) -> GraphViewIter<'a> {
        self.iter()
    }
}

impl<'a, 'b> IntoIterator for &'b GraphView<'a> {
    type Item = TripleRef<'a>;
    type IntoIter = GraphViewIter<'a>;

    fn into_iter(self) -> GraphViewIter<'a> {
        self.iter()
    }
}

impl<'a> fmt::Display for GraphView<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self {
            writeln!(f, "{t}")?;
        }
        Ok(())
    }
}

/// A read/write view on an [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) contained in a [`Dataset`].
///
/// It is built using the [`Dataset::graph_mut`] method.
///
/// Usage example:
/// ```
/// use oxrdf::*;
///
/// let mut dataset = Dataset::default();
/// let ex = NamedNodeRef::new("http://example.com")?;
///
/// // We edit and query the dataset http://example.com graph
/// {
///     let mut graph = dataset.graph_mut(ex);
///     graph.insert(TripleRef::new(ex, ex, ex));
///     let results: Vec<_> = graph.iter().collect();
///     assert_eq!(vec![TripleRef::new(ex, ex, ex)], results);
/// }
///
/// // We have also changes the dataset itself
/// let results: Vec<_> = dataset.iter().collect();
/// assert_eq!(vec![QuadRef::new(ex, ex, ex, ex)], results);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Debug)]
pub struct GraphViewMut<'a> {
    dataset: &'a mut Dataset,
    graph_name: InternedGraphName,
}

impl<'a> GraphViewMut<'a> {
    fn read(&self) -> GraphView<'_> {
        GraphView {
            dataset: self.dataset,
            graph_name: self.graph_name.clone(),
        }
    }

    /// Adds a triple to the graph.
    pub fn insert<'b>(&mut self, triple: impl Into<TripleRef<'b>>) -> bool {
        let triple = self.encode_triple(triple.into());
        self.dataset.insert_encoded((
            triple.subject,
            triple.predicate,
            triple.object,
            self.graph_name.clone(),
        ))
    }

    /// Removes a concrete triple from the graph.
    pub fn remove<'b>(&mut self, triple: impl Into<TripleRef<'b>>) -> bool {
        if let Some(triple) = self.read().encoded_triple(triple.into()) {
            self.dataset.remove_encoded((
                triple.subject,
                triple.predicate,
                triple.object,
                self.graph_name.clone(),
            ))
        } else {
            false
        }
    }

    fn encode_triple(&mut self, triple: TripleRef<'_>) -> InternedTriple {
        InternedTriple {
            subject: InternedSubject::encoded_into(triple.subject, &mut self.dataset.interner),
            predicate: InternedNamedNode::encoded_into(
                triple.predicate,
                &mut self.dataset.interner,
            ),
            object: InternedTerm::encoded_into(triple.object, &mut self.dataset.interner),
        }
    }

    /// Returns all the triples contained by the graph
    pub fn iter(&'a self) -> GraphViewIter<'a> {
        self.read().iter()
    }

    pub fn triples_for_subject<'b>(
        &'a self,
        subject: impl Into<SubjectRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.read()
            .triples_for_interned_subject(self.dataset.encoded_subject(subject))
    }

    pub fn objects_for_subject_predicate<'b>(
        &'a self,
        subject: impl Into<SubjectRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TermRef<'a>> + 'a {
        self.read().objects_for_interned_subject_predicate(
            self.dataset.encoded_subject(subject),
            self.dataset.encoded_named_node(predicate),
        )
    }

    pub fn object_for_subject_predicate<'b>(
        &'a self,
        subject: impl Into<SubjectRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> Option<TermRef<'a>> {
        self.read().object_for_subject_predicate(subject, predicate)
    }

    pub fn predicates_for_subject_object<'b>(
        &'a self,
        subject: impl Into<SubjectRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = NamedNodeRef<'a>> + 'a {
        self.read().predicates_for_interned_subject_object(
            self.dataset.encoded_subject(subject),
            self.dataset.encoded_term(object),
        )
    }

    pub fn triples_for_predicate<'b>(
        &'a self,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.read()
            .triples_for_interned_predicate(self.dataset.encoded_named_node(predicate))
    }

    pub fn subjects_for_predicate_object<'b>(
        &'a self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = SubjectRef<'a>> + 'a {
        self.read().subjects_for_interned_predicate_object(
            self.dataset.encoded_named_node(predicate),
            self.dataset.encoded_term(object),
        )
    }

    pub fn subject_for_predicate_object<'b>(
        &'a self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> Option<SubjectRef<'a>> {
        self.read().subject_for_predicate_object(predicate, object)
    }

    pub fn triples_for_object<'b>(
        &'a self,
        object: TermRef<'b>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.read()
            .triples_for_interned_object(self.dataset.encoded_term(object))
    }

    /// Checks if the graph contains the given triple.
    pub fn contains<'b>(&self, triple: impl Into<TripleRef<'b>>) -> bool {
        self.read().contains(triple)
    }

    /// Returns the number of triples in this graph.
    pub fn len(&self) -> usize {
        self.read().len()
    }

    /// Checks if this graph contains a triple.
    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }
}

impl<'a> Extend<Triple> for GraphViewMut<'a> {
    fn extend<I: IntoIterator<Item = Triple>>(&mut self, iter: I) {
        for t in iter {
            self.insert(&t);
        }
    }
}

impl<'a, 'b, T: Into<TripleRef<'b>>> Extend<T> for GraphViewMut<'a> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for t in iter {
            self.insert(t);
        }
    }
}

impl<'a> IntoIterator for &'a GraphViewMut<'a> {
    type Item = TripleRef<'a>;
    type IntoIter = GraphViewIter<'a>;

    fn into_iter(self) -> GraphViewIter<'a> {
        self.iter()
    }
}

impl<'a> fmt::Display for GraphViewMut<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self {
            writeln!(f, "{t}")?;
        }
        Ok(())
    }
}

/// Iterator returned by [`Dataset::iter`].
pub struct Iter<'a> {
    dataset: &'a Dataset,
    inner: std::collections::btree_set::Iter<
        'a,
        (
            InternedSubject,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
    >,
}

impl<'a> Iterator for Iter<'a> {
    type Item = QuadRef<'a>;

    fn next(&mut self) -> Option<QuadRef<'a>> {
        self.inner
            .next()
            .map(|(s, p, o, g)| self.dataset.decode_spog((s, p, o, g)))
    }
}

/// Iterator returned by [`GraphView::iter`].
pub struct GraphViewIter<'a> {
    dataset: &'a Dataset,
    inner: std::collections::btree_set::Range<
        'a,
        (
            InternedGraphName,
            InternedSubject,
            InternedNamedNode,
            InternedTerm,
        ),
    >,
}

impl<'a> Iterator for GraphViewIter<'a> {
    type Item = TripleRef<'a>;

    fn next(&mut self) -> Option<TripleRef<'a>> {
        self.inner
            .next()
            .map(|(_, s, p, o)| self.dataset.decode_spo((s, p, o)))
    }
}
