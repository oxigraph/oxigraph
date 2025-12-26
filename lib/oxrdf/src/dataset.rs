//! [In-memory implementation](Dataset) of [RDF datasets](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
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
//!
//! // Print
//! assert_eq!(
//!     dataset.to_string(),
//!     "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n"
//! );
//! # Result::<_, Box<dyn std::error::Error>>::Ok(())
//! ```
//!
//! See also [`Graph`] if you only care about plain triples.

use crate::interning::*;
use crate::*;
#[cfg(feature = "rdfc-10")]
use sha2::{Digest, Sha256, Sha384};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::mem::take;

/// An in-memory [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
///
/// It can accommodate a fairly large number of quads (in the few millions).
///
/// <div class="warning">It interns the strings and does not do any garbage collection yet:
/// if you insert and remove a lot of different terms, memory will grow without any reduction.</div>
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
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Debug, Default, Clone)]
pub struct Dataset {
    interner: Interner,
    gspo: BTreeSet<(
        InternedGraphName,
        InternedNamedOrBlankNode,
        InternedNamedNode,
        InternedTerm,
    )>,
    gpos: BTreeSet<(
        InternedGraphName,
        InternedNamedNode,
        InternedTerm,
        InternedNamedOrBlankNode,
    )>,
    gosp: BTreeSet<(
        InternedGraphName,
        InternedTerm,
        InternedNamedOrBlankNode,
        InternedNamedNode,
    )>,
    spog: BTreeSet<(
        InternedNamedOrBlankNode,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    )>,
    posg: BTreeSet<(
        InternedNamedNode,
        InternedTerm,
        InternedNamedOrBlankNode,
        InternedGraphName,
    )>,
    ospg: BTreeSet<(
        InternedTerm,
        InternedNamedOrBlankNode,
        InternedNamedNode,
        InternedGraphName,
    )>,
}

impl Dataset {
    /// Creates a new dataset
    pub fn new() -> Self {
        Self::default()
    }

    /// Provides a read-only view on an [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) contained in this dataset.
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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

    /// Provides a read/write view on an [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) contained in this dataset.
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
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
    ) -> impl Iterator<Item = QuadRef<'a>> + 'a {
        let subject = self
            .encoded_named_or_blank_node(subject)
            .unwrap_or_else(InternedNamedOrBlankNode::impossible);
        self.interned_quads_for_subject(&subject)
            .map(move |q| self.decode_spog(q))
    }

    fn interned_quads_for_subject<'a>(
        &'a self,
        subject: &InternedNamedOrBlankNode,
    ) -> impl Iterator<
        Item = (
            &'a InternedNamedOrBlankNode,
            &'a InternedNamedNode,
            &'a InternedTerm,
            &'a InternedGraphName,
        ),
    > + use<'a> {
        self.spog
            .range(
                &(
                    *subject,
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
            &InternedNamedOrBlankNode,
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
                    InternedNamedOrBlankNode::first(),
                    InternedGraphName::first(),
                )
                    ..&(
                        predicate.next(),
                        InternedTerm::first(),
                        InternedNamedOrBlankNode::first(),
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

    fn interned_quads_for_object<'a>(
        &'a self,
        object: &InternedTerm,
    ) -> impl Iterator<
        Item = (
            &'a InternedNamedOrBlankNode,
            &'a InternedNamedNode,
            &'a InternedTerm,
            &'a InternedGraphName,
        ),
    > + use<'a> {
        self.ospg
            .range(
                &(
                    object.clone(),
                    InternedNamedOrBlankNode::first(),
                    InternedNamedNode::first(),
                    InternedGraphName::first(),
                )
                    ..&(
                        object.next(),
                        InternedNamedOrBlankNode::first(),
                        InternedNamedNode::first(),
                        InternedGraphName::first(),
                    ),
            )
            .map(|(o, s, p, g)| (s, p, o, g))
    }

    pub fn quads_for_graph_name<'a, 'b>(
        &'a self,
        graph_name: impl Into<GraphNameRef<'b>>,
    ) -> impl Iterator<Item = QuadRef<'a>> + 'a {
        let graph_name = self
            .encoded_graph_name(graph_name)
            .unwrap_or_else(InternedGraphName::impossible);

        self.interned_quads_for_graph_name(&graph_name)
            .map(move |q| self.decode_spog(q))
    }

    fn interned_quads_for_graph_name<'a>(
        &'a self,
        graph_name: &InternedGraphName,
    ) -> impl Iterator<
        Item = (
            &'a InternedNamedOrBlankNode,
            &'a InternedNamedNode,
            &'a InternedTerm,
            &'a InternedGraphName,
        ),
    > + use<'a> {
        self.gspo
            .range(
                &(
                    *graph_name,
                    InternedNamedOrBlankNode::first(),
                    InternedNamedNode::first(),
                    InternedTerm::first(),
                )
                    ..&(
                        graph_name.next(),
                        InternedNamedOrBlankNode::first(),
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
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
    ) -> bool {
        let (s, p, o, g) = quad;
        self.gspo.insert((g, s, p, o.clone()));
        self.gpos.insert((g, p, o.clone(), s));
        self.gosp.insert((g, o.clone(), s, p));
        self.spog.insert((s, p, o.clone(), g));
        self.posg.insert((p, o.clone(), s, g));
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
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
    ) -> bool {
        let (s, p, o, g) = quad;
        self.gspo.remove(&(g, s, p, o.clone()));
        self.gpos.remove(&(g, p, o.clone(), s));
        self.gosp.remove(&(g, o.clone(), s, p));
        self.spog.remove(&(s, p, o.clone(), g));
        self.posg.remove(&(p, o.clone(), s, g));
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

    /// Computes the union of two datasets (self ⊔ other).
    ///
    /// Returns a new dataset containing all quads from both datasets.
    /// Uses deterministic BTreeSet iteration for reproducible results.
    ///
    /// ```
    /// use oxrdf::*;
    ///
    /// let mut ds1 = Dataset::new();
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// ds1.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph));
    ///
    /// let mut ds2 = Dataset::new();
    /// let ex2 = NamedNodeRef::new("http://example.com/2")?;
    /// ds2.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));
    ///
    /// let union = ds1.union(&ds2);
    /// assert_eq!(union.len(), 2);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn union(&self, other: &Self) -> Self {
        let mut result = self.clone();
        for quad in other.iter() {
            result.insert(quad);
        }
        result
    }

    /// Computes the set difference (self \ other).
    ///
    /// Returns a new dataset containing quads in self but not in other.
    /// Essential for computing Δ⁻ in ΔGate protocol.
    ///
    /// ```
    /// use oxrdf::*;
    ///
    /// let mut ds1 = Dataset::new();
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// ds1.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph));
    ///
    /// let mut ds2 = Dataset::new();
    /// ds2.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph));
    ///
    /// let diff = ds1.difference(&ds2);
    /// assert!(diff.is_empty());
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn difference(&self, other: &Self) -> Self {
        let mut result = Self::new();
        for quad in self.iter() {
            if !other.contains(quad) {
                result.insert(quad);
            }
        }
        result
    }

    /// Computes the intersection of two datasets (self ∩ other).
    ///
    /// Returns a new dataset containing only quads present in both datasets.
    ///
    /// ```
    /// use oxrdf::*;
    ///
    /// let mut ds1 = Dataset::new();
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// ds1.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph));
    ///
    /// let mut ds2 = Dataset::new();
    /// ds2.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph));
    ///
    /// let intersection = ds1.intersection(&ds2);
    /// assert_eq!(intersection.len(), 1);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn intersection(&self, other: &Self) -> Self {
        let mut result = Self::new();
        for quad in self.iter() {
            if other.contains(quad) {
                result.insert(quad);
            }
        }
        result
    }

    /// Computes the symmetric difference (self Δ other).
    ///
    /// Returns a new dataset containing quads in either dataset but not in both.
    ///
    /// ```
    /// use oxrdf::*;
    ///
    /// let mut ds1 = Dataset::new();
    /// let ex1 = NamedNodeRef::new("http://example.com/1")?;
    /// ds1.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));
    ///
    /// let mut ds2 = Dataset::new();
    /// let ex2 = NamedNodeRef::new("http://example.com/2")?;
    /// ds2.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));
    ///
    /// let sym_diff = ds1.symmetric_difference(&ds2);
    /// assert_eq!(sym_diff.len(), 2);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn symmetric_difference(&self, other: &Self) -> Self {
        let mut result = Self::new();
        for quad in self.iter() {
            if !other.contains(quad) {
                result.insert(quad);
            }
        }
        for quad in other.iter() {
            if !self.contains(quad) {
                result.insert(quad);
            }
        }
        result
    }

    /// Computes the delta/diff between two datasets for ΔGate protocol.
    ///
    /// Returns (additions, removals) where:
    /// - additions (Δ⁺) = quads in `target` but not in `self`
    /// - removals (Δ⁻) = quads in `self` but not in `target`
    ///
    /// This is the core operation for ΔGate delta computation.
    ///
    /// ```
    /// use oxrdf::*;
    ///
    /// let mut before = Dataset::new();
    /// let ex1 = NamedNodeRef::new("http://example.com/1")?;
    /// before.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));
    ///
    /// let mut after = Dataset::new();
    /// let ex2 = NamedNodeRef::new("http://example.com/2")?;
    /// after.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));
    ///
    /// let (additions, removals) = before.diff(&after);
    /// assert_eq!(additions.len(), 1); // ex2 added
    /// assert_eq!(removals.len(), 1);  // ex1 removed
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn diff(&self, target: &Self) -> (Self, Self) {
        let additions = target.difference(self);
        let removals = self.difference(target);
        (additions, removals)
    }

    /// Applies a delta to this dataset for ΔGate protocol.
    ///
    /// Applies additions (Δ⁺) and removals (Δ⁻) to transform this dataset.
    /// This is the inverse operation of `diff`.
    ///
    /// ```
    /// use oxrdf::*;
    ///
    /// let mut ds = Dataset::new();
    /// let ex1 = NamedNodeRef::new("http://example.com/1")?;
    /// ds.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));
    ///
    /// let mut additions = Dataset::new();
    /// let ex2 = NamedNodeRef::new("http://example.com/2")?;
    /// additions.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));
    ///
    /// let mut removals = Dataset::new();
    /// removals.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));
    ///
    /// ds.apply_diff(&additions, &removals);
    /// assert_eq!(ds.len(), 1);
    /// assert!(ds.contains(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph)));
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn apply_diff(&mut self, additions: &Self, removals: &Self) {
        // First remove
        for quad in removals.iter() {
            self.remove(quad);
        }
        // Then add
        for quad in additions.iter() {
            self.insert(quad);
        }
    }

    fn encode_quad(
        &mut self,
        quad: QuadRef<'_>,
    ) -> (
        InternedNamedOrBlankNode,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    ) {
        (
            InternedNamedOrBlankNode::encoded_into(quad.subject, &mut self.interner),
            InternedNamedNode::encoded_into(quad.predicate, &mut self.interner),
            InternedTerm::encoded_into(quad.object, &mut self.interner),
            InternedGraphName::encoded_into(quad.graph_name, &mut self.interner),
        )
    }

    fn encoded_quad(
        &self,
        quad: QuadRef<'_>,
    ) -> Option<(
        InternedNamedOrBlankNode,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    )> {
        Some((
            self.encoded_named_or_blank_node(quad.subject)?,
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

    pub(super) fn encoded_named_or_blank_node<'a>(
        &self,
        node: impl Into<NamedOrBlankNodeRef<'a>>,
    ) -> Option<InternedNamedOrBlankNode> {
        InternedNamedOrBlankNode::encoded_from(node.into(), &self.interner)
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
            &InternedNamedOrBlankNode,
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
        triple: (&InternedNamedOrBlankNode, &InternedNamedNode, &InternedTerm),
    ) -> TripleRef<'_> {
        TripleRef {
            subject: triple.0.decode_from(&self.interner),
            predicate: triple.1.decode_from(&self.interner),
            object: triple.2.decode_from(&self.interner),
        }
    }

    /// Canonicalizes the dataset by renaming blank nodes.
    ///
    /// Usage example ([Dataset isomorphism](https://www.w3.org/TR/rdf11-concepts/#dfn-dataset-isomorphism)):
    /// ```
    /// use oxrdf::dataset::CanonicalizationAlgorithm;
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
    /// graph1.canonicalize(CanonicalizationAlgorithm::Unstable);
    /// graph2.canonicalize(CanonicalizationAlgorithm::Unstable);
    /// assert_eq!(graph1, graph2);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// It supports the [RDF Dataset Canonicalization](https://www.w3.org/TR/rdf-canon/) standard algorithm.
    /// Support requires the `rdfc-10` feature to be enabled.
    ///
    /// <div class="warning">Blank node ids depend on the current shape of the graph. Adding a new quad might change the ids of a lot of blank nodes.
    /// Hence, this canonization might not be suitable for diffs.</div>
    ///
    /// <div class="warning">
    ///     This implementation's worst-case complexity is exponential with respect to the number of blank nodes in the input dataset.
    ///     See [the RDFC specification section about it](https://www.w3.org/TR/rdf-canon/#dataset-poisoning).
    /// </div>
    pub fn canonicalize(&mut self, algorithm: CanonicalizationAlgorithm) {
        let bnode_mapping = self.canonicalize_interned_blank_nodes(algorithm);
        let new_quads = self.map_blank_nodes(&bnode_mapping);
        self.clear();
        for quad in new_quads {
            self.insert_encoded(quad);
        }
    }

    /// Returns a map between the current dataset blank node and the canonicalized blank node
    /// to create a canonical dataset.
    ///
    /// See also [`canonicalize`](Self::canonicalize).
    pub fn canonicalize_blank_nodes(
        &self,
        algorithm: CanonicalizationAlgorithm,
    ) -> HashMap<BlankNodeRef<'_>, BlankNode> {
        self.canonicalize_interned_blank_nodes(algorithm)
            .into_iter()
            .map(|(from, to)| (from.decode_from(&self.interner), to))
            .collect()
    }

    fn canonicalize_interned_blank_nodes(
        &self,
        algorithm: CanonicalizationAlgorithm,
    ) -> HashMap<InternedBlankNode, BlankNode> {
        let hash_algorithm = match algorithm {
            CanonicalizationAlgorithm::Unstable => None,
            #[cfg(feature = "rdfc-10")]
            CanonicalizationAlgorithm::Rdfc10 { hash_algorithm } => Some(hash_algorithm),
        };
        // https://www.w3.org/TR/rdf-canon/#canon-algo-algo
        // 1)
        let mut canonicalization_state = CanonicalizationState {
            blank_node_to_quads_map: QuadsPerBlankNode::new(),
            hash_to_blank_nodes_map: BTreeMap::new(),
            canonical_issuer: IdentifierIssuer::new("c14n"),
        };
        // 2)
        for quad in &self.spog {
            if let InternedNamedOrBlankNode::BlankNode(bnode) = quad.0 {
                Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                    bnode,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
            if let InternedTerm::BlankNode(bnode) = &quad.2 {
                Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                    *bnode,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
            #[cfg(feature = "rdf-12")]
            if let InternedTerm::Triple(t) = &quad.2 {
                Self::add_quad_to_blank_node_to_quads_map_based_on_triple(
                    t,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
            if let InternedGraphName::BlankNode(bnode) = &quad.3 {
                Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                    *bnode,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
        }
        // 3)
        for n in canonicalization_state.blank_node_to_quads_map.keys() {
            // 3.1)
            let hash = self.hash_first_degree_quads(&canonicalization_state, *n, hash_algorithm);
            // 3.2)
            canonicalization_state
                .hash_to_blank_nodes_map
                .entry(hash)
                .or_default()
                .push(*n);
        }
        // 4)
        canonicalization_state.hash_to_blank_nodes_map = canonicalization_state
            .hash_to_blank_nodes_map
            .into_iter()
            .filter(|(_, identifier_list)| {
                match identifier_list.len() {
                    0 => unreachable!(),
                    // 4.1)
                    2.. => true,
                    1 => {
                        // 4.2)
                        Self::issue_identifier(
                            &mut canonicalization_state.canonical_issuer,
                            identifier_list[0],
                        );
                        // 4.3)
                        false
                    }
                }
            })
            .collect::<BTreeMap<_, _>>();
        // 5)
        for (_, identifier_list) in take(&mut canonicalization_state.hash_to_blank_nodes_map) {
            // 5.1)
            let mut hash_path_list = Vec::new();
            // 5.2)
            for n in identifier_list {
                // 5.2.1)
                if canonicalization_state
                    .canonical_issuer
                    .issued_identifier_map
                    .contains_key(&n)
                {
                    continue;
                }
                // 5.2.2)
                let mut temporary_issuer = IdentifierIssuer::new("b");
                // 5.2.3)
                Self::issue_identifier(&mut temporary_issuer, n);
                // 5.2.4)
                hash_path_list.push(self.hash_n_degree_quads(
                    &canonicalization_state,
                    n,
                    &temporary_issuer,
                    hash_algorithm,
                ))
            }
            // 5.3)
            hash_path_list.sort_unstable_by(|(_, hl), (_, hr)| hl.cmp(hr));
            for (result_identifier_issuer, _) in hash_path_list {
                // 5.3.1)
                for existing_identifier in result_identifier_issuer.issued_identifier_order {
                    Self::issue_identifier(
                        &mut canonicalization_state.canonical_issuer,
                        existing_identifier,
                    );
                }
            }
        }
        // 6)
        canonicalization_state
            .canonical_issuer
            .issued_identifier_map
    }

    #[cfg(feature = "rdf-12")]
    fn add_quad_to_blank_node_to_quads_map_based_on_triple<'a>(
        triple: &InternedTriple,
        quad: &'a (
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
        blank_node_to_quads_map: &mut QuadsPerBlankNode<'a>,
    ) {
        if let InternedNamedOrBlankNode::BlankNode(bnode) = triple.subject {
            Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                bnode,
                quad,
                blank_node_to_quads_map,
            );
        }
        if let InternedTerm::BlankNode(bnode) = &triple.object {
            Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                *bnode,
                quad,
                blank_node_to_quads_map,
            );
        } else if let InternedTerm::Triple(t) = &triple.object {
            Self::add_quad_to_blank_node_to_quads_map_based_on_triple(
                t,
                quad,
                blank_node_to_quads_map,
            );
        }
    }

    fn add_quad_to_blank_node_to_quads_map_for_blank_node<'a>(
        bnode: InternedBlankNode,
        quad: &'a (
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
        blank_node_to_quads_map: &mut QuadsPerBlankNode<'a>,
    ) {
        let entry = blank_node_to_quads_map.entry(bnode).or_default();
        if !entry.ends_with(&[quad]) {
            entry.push(quad);
        }
    }

    /// RDFC [Issue Identifier Algorithm](https://www.w3.org/TR/rdf-canon/#issue-identifier)
    fn issue_identifier(issuer: &mut IdentifierIssuer, blank_node: InternedBlankNode) -> BlankNode {
        match issuer.issued_identifier_map.entry(blank_node) {
            // 1)
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                // 2)
                let issued_identifier = BlankNode::new_unchecked(format!(
                    "{}{}",
                    issuer.identifier_prefix, issuer.identifier_counter
                ));
                // 3)
                entry.insert(issued_identifier.clone());
                issuer.issued_identifier_order.push(blank_node);
                // 4)
                issuer.identifier_counter += 1;
                // 5)
                issued_identifier
            }
        }
    }

    /// RDFC [Hash First Degree Quads](https://www.w3.org/TR/rdf-canon/#hash-1d-quads)
    fn hash_first_degree_quads(
        &self,
        canonicalization_state: &CanonicalizationState<'_>,
        reference_blank_node_identifier: InternedBlankNode,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) -> String {
        // 1)
        let mut nquads = Vec::new();
        // 2)
        let quads =
            &canonicalization_state.blank_node_to_quads_map[&reference_blank_node_identifier];
        // 3)
        for (subject, predicate, object, graph_name) in quads {
            // 3.1)
            let subject = self.hash_first_degree_quads_decode_named_or_blank_node(
                subject,
                &reference_blank_node_identifier,
            );
            let predicate = predicate.decode_from(&self.interner);
            let object =
                self.hash_first_degree_quads_decode_term(object, &reference_blank_node_identifier);
            let graph_name = self.hash_first_degree_quads_decode_graph_name(
                graph_name,
                &reference_blank_node_identifier,
            );
            nquads.push(if graph_name.is_default_graph() {
                format!("{subject} {predicate} {object} .\n")
            } else {
                format!("{subject} {predicate} {object} {graph_name} .\n")
            });
        }
        // 3)
        nquads.sort();
        // 4)
        Self::hash_function(&nquads.join(""), hash_algorithm)
    }

    fn hash_first_degree_quads_decode_named_or_blank_node(
        &self,
        term: &InternedNamedOrBlankNode,
        reference_blank_node_identifier: &InternedBlankNode,
    ) -> NamedOrBlankNodeRef<'_> {
        match term {
            InternedNamedOrBlankNode::NamedNode(t) => t.decode_from(&self.interner).into(),
            InternedNamedOrBlankNode::BlankNode(t) => {
                BlankNodeRef::new_unchecked(if t == reference_blank_node_identifier {
                    "a"
                } else {
                    "z"
                })
                .into()
            }
        }
    }

    fn hash_first_degree_quads_decode_term(
        &self,
        term: &InternedTerm,
        reference_blank_node_identifier: &InternedBlankNode,
    ) -> Term {
        match term {
            InternedTerm::NamedNode(t) => t.decode_from(&self.interner).into(),
            InternedTerm::BlankNode(t) => {
                BlankNodeRef::new_unchecked(if t == reference_blank_node_identifier {
                    "a"
                } else {
                    "z"
                })
                .into()
            }
            InternedTerm::Literal(t) => t.decode_from(&self.interner).into(),
            #[cfg(feature = "rdf-12")]
            InternedTerm::Triple(t) => Triple::new(
                self.hash_first_degree_quads_decode_named_or_blank_node(
                    &t.subject,
                    reference_blank_node_identifier,
                ),
                t.predicate.decode_from(&self.interner),
                self.hash_first_degree_quads_decode_term(
                    &t.object,
                    reference_blank_node_identifier,
                ),
            )
            .into(),
        }
    }

    fn hash_first_degree_quads_decode_graph_name(
        &self,
        term: &InternedGraphName,
        reference_blank_node_identifier: &InternedBlankNode,
    ) -> GraphNameRef<'_> {
        match term {
            InternedGraphName::NamedNode(t) => t.decode_from(&self.interner).into(),
            InternedGraphName::BlankNode(t) => {
                BlankNodeRef::new_unchecked(if t == reference_blank_node_identifier {
                    "a"
                } else {
                    "z"
                })
                .into()
            }
            InternedGraphName::DefaultGraph => GraphNameRef::DefaultGraph,
        }
    }

    /// RDFC [Hash Related Blank Node](https://www.w3.org/TR/rdf-canon/#hash-related-blank-node)
    fn hash_related_blank_node(
        &self,
        canonicalization_state: &CanonicalizationState<'_>,
        related: InternedBlankNode,
        quad: &(
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
        issuer: &IdentifierIssuer,
        position: &str,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) -> String {
        // 1)
        let mut input = position.to_owned();
        // 2)
        if position != "g" {
            input.push('<');
            input.push_str(quad.1.decode_from(&self.interner).as_str());
            input.push('>');
        }
        // 3)
        if let Some(id) = canonicalization_state
            .canonical_issuer
            .issued_identifier_map
            .get(&related)
            .or_else(|| issuer.issued_identifier_map.get(&related))
        {
            input.push_str("_:");
            input.push_str(id.as_str());
        } else {
            // 4)
            input.push_str(&self.hash_first_degree_quads(
                canonicalization_state,
                related,
                hash_algorithm,
            ));
        }
        // 5)
        Self::hash_function(&input, hash_algorithm)
    }

    /// RDFC [Hash N Degree Quads](https://www.w3.org/TR/rdf-canon/#hash-nd-quads)
    fn hash_n_degree_quads(
        &self,
        canonicalization_state: &CanonicalizationState<'_>,
        identifier: InternedBlankNode,
        issuer: &IdentifierIssuer,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) -> (IdentifierIssuer, String) {
        let mut issuer = issuer.clone();
        // 1)
        let mut h_n = BTreeMap::<_, HashSet<_>>::new();
        // 2)
        let quads = &canonicalization_state.blank_node_to_quads_map[&identifier];
        // 3)
        for quad in quads {
            // 3.1)
            if let InternedNamedOrBlankNode::BlankNode(component) = quad.0 {
                self.hash_related_blank_node_on_possible_component(
                    canonicalization_state,
                    component,
                    identifier,
                    quad,
                    &issuer,
                    "s",
                    &mut h_n,
                    hash_algorithm,
                );
            }
            if let InternedTerm::BlankNode(component) = quad.2 {
                self.hash_related_blank_node_on_possible_component(
                    canonicalization_state,
                    component,
                    identifier,
                    quad,
                    &issuer,
                    "o",
                    &mut h_n,
                    hash_algorithm,
                );
            }
            #[cfg(feature = "rdf-12")]
            if let InternedTerm::Triple(t) = &quad.2 {
                self.hash_related_blank_node_on_possible_triple(
                    canonicalization_state,
                    t,
                    identifier,
                    quad,
                    &issuer,
                    &mut h_n,
                    hash_algorithm,
                );
            }
            if let InternedGraphName::BlankNode(component) = quad.3 {
                self.hash_related_blank_node_on_possible_component(
                    canonicalization_state,
                    component,
                    identifier,
                    quad,
                    &issuer,
                    "g",
                    &mut h_n,
                    hash_algorithm,
                );
            }
        }
        // 4)
        let mut data_to_hash = String::new();
        // 5)
        for (related_hash, blank_node_list) in h_n {
            // 5.1)
            data_to_hash.push_str(&related_hash);
            // 5.2)
            let mut chosen_path = String::new();
            // 5.3)
            let mut chosen_issuer = IdentifierIssuer::new("");
            // 5.4)
            'perm: for p in generate_permutations(blank_node_list) {
                // 5.4.1)
                let mut issuer_copy = issuer.clone();
                // 5.4.2)
                let mut path = String::new();
                // 5.4.3)
                let mut recursion_list = Vec::new();
                // 5.4.4)
                for related in p {
                    // 5.4.4.1)
                    if let Some(id) = canonicalization_state
                        .canonical_issuer
                        .issued_identifier_map
                        .get(&related)
                    {
                        path.push_str("_:");
                        path.push_str(id.as_str());
                    } else {
                        // 5.4.4.2)
                        // 5.4.4.2.1)
                        if !issuer_copy.issued_identifier_map.contains_key(&related) {
                            recursion_list.push(related);
                        }
                        // 5.4.4.2.2)
                        let id = Self::issue_identifier(&mut issuer_copy, related);
                        path.push_str("_:");
                        path.push_str(id.as_str());
                    }
                    // 5.4.4.3)
                    if !chosen_path.is_empty()
                        && path.len() >= chosen_path.len()
                        && path > chosen_path
                    {
                        continue 'perm;
                    }
                }
                // 5.4.5)
                for related in recursion_list {
                    // 5.4.5.1)
                    let (result_identifier_issuer, result_hash) = self.hash_n_degree_quads(
                        canonicalization_state,
                        related,
                        &issuer_copy,
                        hash_algorithm,
                    );
                    // 5.4.5.2)
                    let id = Self::issue_identifier(&mut issuer_copy, related);
                    path.push_str("_:");
                    path.push_str(id.as_str());
                    // 5.4.5.3)
                    path.push('<');
                    path.push_str(&result_hash);
                    path.push('>');
                    // 5.4.5.4)
                    issuer_copy = result_identifier_issuer;
                    // 5.4.5.5)
                    if !chosen_path.is_empty()
                        && path.len() >= chosen_path.len()
                        && path > chosen_path
                    {
                        continue 'perm;
                    }
                }
                // 5.4.6)
                if chosen_path.is_empty() || path < chosen_path {
                    chosen_path = path;
                    chosen_issuer = issuer_copy;
                }
            }
            // 5.5)
            data_to_hash.push_str(&chosen_path);
            // 5.6)
            issuer = chosen_issuer;
        }
        // 6)
        (issuer, Self::hash_function(&data_to_hash, hash_algorithm))
    }

    #[cfg(feature = "rdf-12")]
    fn hash_related_blank_node_on_possible_triple(
        &self,
        canonicalization_state: &CanonicalizationState<'_>,
        triple: &InternedTriple,
        identifier: InternedBlankNode,
        quad: &(
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
        issuer: &IdentifierIssuer,
        h_n: &mut BTreeMap<String, HashSet<InternedBlankNode>>,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) {
        if let InternedNamedOrBlankNode::BlankNode(component) = triple.subject {
            self.hash_related_blank_node_on_possible_component(
                canonicalization_state,
                component,
                identifier,
                quad,
                issuer,
                "os",
                h_n,
                hash_algorithm,
            );
        }
        if let InternedTerm::BlankNode(component) = &triple.object {
            self.hash_related_blank_node_on_possible_component(
                canonicalization_state,
                *component,
                identifier,
                quad,
                issuer,
                "oo",
                h_n,
                hash_algorithm,
            );
        }
    }

    fn hash_related_blank_node_on_possible_component(
        &self,
        canonicalization_state: &CanonicalizationState<'_>,
        component: InternedBlankNode,
        identifier: InternedBlankNode,
        quad: &(
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
        issuer: &IdentifierIssuer,
        position: &str,
        h_n: &mut BTreeMap<String, HashSet<InternedBlankNode>>,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) {
        if component != identifier {
            // 3.1.1)
            let hash = self.hash_related_blank_node(
                canonicalization_state,
                component,
                quad,
                issuer,
                position,
                hash_algorithm,
            );
            // 3.1.2)
            h_n.entry(hash).or_default().insert(component);
        }
    }

    fn hash_function(input: &str, hash_algorithm: Option<CanonicalizationHashAlgorithm>) -> String {
        match hash_algorithm {
            #[cfg(feature = "rdfc-10")]
            Some(CanonicalizationHashAlgorithm::Sha256) => {
                hex::encode(Sha256::new().chain_update(input).finalize())
            }
            #[cfg(feature = "rdfc-10")]
            Some(CanonicalizationHashAlgorithm::Sha384) => {
                hex::encode(Sha384::new().chain_update(input).finalize())
            }
            None => {
                let mut hasher = DefaultHasher::new();
                input.hash(&mut hasher);
                hasher.finish().to_string()
            }
        }
    }

    #[expect(clippy::needless_collect)]
    fn map_blank_nodes(
        &mut self,
        bnode_mapping: &HashMap<InternedBlankNode, BlankNode>,
    ) -> Vec<(
        InternedNamedOrBlankNode,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    )> {
        let old_quads: Vec<_> = self.spog.iter().cloned().collect();
        old_quads
            .into_iter()
            .map(|(s, p, o, g)| {
                (
                    match s {
                        InternedNamedOrBlankNode::NamedNode(_) => s,
                        InternedNamedOrBlankNode::BlankNode(bnode) => {
                            InternedNamedOrBlankNode::BlankNode(InternedBlankNode::encoded_into(
                                bnode_mapping[&bnode].as_ref(),
                                &mut self.interner,
                            ))
                        }
                    },
                    p,
                    match o {
                        InternedTerm::NamedNode(_) | InternedTerm::Literal(_) => o,
                        InternedTerm::BlankNode(bnode) => {
                            InternedTerm::BlankNode(InternedBlankNode::encoded_into(
                                bnode_mapping[&bnode].as_ref(),
                                &mut self.interner,
                            ))
                        }
                        #[cfg(feature = "rdf-12")]
                        InternedTerm::Triple(triple) => {
                            InternedTerm::Triple(Box::new(InternedTriple::encoded_into(
                                self.map_triple_blank_nodes(&triple, bnode_mapping).as_ref(),
                                &mut self.interner,
                            )))
                        }
                    },
                    match g {
                        InternedGraphName::NamedNode(_) | InternedGraphName::DefaultGraph => g,
                        InternedGraphName::BlankNode(bnode) => {
                            InternedGraphName::BlankNode(InternedBlankNode::encoded_into(
                                bnode_mapping[&bnode].as_ref(),
                                &mut self.interner,
                            ))
                        }
                    },
                )
            })
            .collect()
    }

    #[cfg(feature = "rdf-12")]
    fn map_triple_blank_nodes(
        &mut self,
        triple: &InternedTriple,
        bnode_mapping: &HashMap<InternedBlankNode, BlankNode>,
    ) -> Triple {
        Triple {
            subject: if let InternedNamedOrBlankNode::BlankNode(bnode) = &triple.subject {
                bnode_mapping[bnode].clone().into()
            } else {
                triple.subject.decode_from(&self.interner).into_owned()
            },
            predicate: triple.predicate.decode_from(&self.interner).into_owned(),
            object: if let InternedTerm::BlankNode(bnode) = &triple.object {
                bnode_mapping[bnode].clone().into()
            } else if let InternedTerm::Triple(t) = &triple.object {
                self.map_triple_blank_nodes(t, bnode_mapping).into()
            } else {
                triple.object.decode_from(&self.interner).into_owned()
            },
        }
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

    fn into_iter(self) -> Self::IntoIter {
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
            writeln!(f, "{t} .")?;
        }
        Ok(())
    }
}

/// A read-only view on an [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) contained in a [`Dataset`].
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
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
                self.graph_name,
                InternedNamedOrBlankNode::first(),
                InternedNamedNode::first(),
                InternedTerm::first(),
            )
                ..&(
                    self.graph_name.next(),
                    InternedNamedOrBlankNode::first(),
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
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.triples_for_interned_subject(self.dataset.encoded_named_or_blank_node(subject))
    }

    pub(super) fn triples_for_interned_subject(
        &self,
        subject: Option<InternedNamedOrBlankNode>,
    ) -> impl Iterator<Item = TripleRef<'a>> + use<'a> {
        let subject = subject.unwrap_or_else(InternedNamedOrBlankNode::impossible);
        let ds = self.dataset;
        self.dataset
            .gspo
            .range(
                &(
                    self.graph_name,
                    subject,
                    InternedNamedNode::first(),
                    InternedTerm::first(),
                )
                    ..&(
                        self.graph_name,
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
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TermRef<'a>> + 'a {
        self.objects_for_interned_subject_predicate(
            self.dataset.encoded_named_or_blank_node(subject),
            self.dataset.encoded_named_node(predicate),
        )
    }

    pub(super) fn objects_for_interned_subject_predicate(
        &self,
        subject: Option<InternedNamedOrBlankNode>,
        predicate: Option<InternedNamedNode>,
    ) -> impl Iterator<Item = TermRef<'a>> + use<'a> {
        let subject = subject.unwrap_or_else(InternedNamedOrBlankNode::impossible);
        let predicate = predicate.unwrap_or_else(InternedNamedNode::impossible);
        let ds = self.dataset;
        self.dataset
            .gspo
            .range(
                &(self.graph_name, subject, predicate, InternedTerm::first())
                    ..&(
                        self.graph_name,
                        subject,
                        predicate.next(),
                        InternedTerm::first(),
                    ),
            )
            .map(move |q| q.3.decode_from(&ds.interner))
    }

    pub fn object_for_subject_predicate<'b>(
        &self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> Option<TermRef<'a>> {
        self.objects_for_subject_predicate(subject, predicate)
            .next()
    }

    pub fn predicates_for_subject_object<'b>(
        &self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = NamedNodeRef<'a>> + 'a {
        self.predicates_for_interned_subject_object(
            self.dataset.encoded_named_or_blank_node(subject),
            self.dataset.encoded_term(object),
        )
    }

    pub(super) fn predicates_for_interned_subject_object(
        &self,
        subject: Option<InternedNamedOrBlankNode>,
        object: Option<InternedTerm>,
    ) -> impl Iterator<Item = NamedNodeRef<'a>> + use<'a> {
        let subject = subject.unwrap_or_else(InternedNamedOrBlankNode::impossible);
        let object = object.unwrap_or_else(InternedTerm::impossible);
        let ds = self.dataset;
        self.dataset
            .gosp
            .range(
                &(
                    self.graph_name,
                    object.clone(),
                    subject,
                    InternedNamedNode::first(),
                )
                    ..&(
                        self.graph_name,
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
    ) -> impl Iterator<Item = TripleRef<'a>> + use<'a> {
        let predicate = predicate.unwrap_or_else(InternedNamedNode::impossible);
        let ds = self.dataset;
        self.dataset
            .gpos
            .range(
                &(
                    self.graph_name,
                    predicate,
                    InternedTerm::first(),
                    InternedNamedOrBlankNode::first(),
                )
                    ..&(
                        self.graph_name,
                        predicate.next(),
                        InternedTerm::first(),
                        InternedNamedOrBlankNode::first(),
                    ),
            )
            .map(move |(_, p, o, s)| ds.decode_spo((s, p, o)))
    }

    pub fn subjects_for_predicate_object<'b>(
        &self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = NamedOrBlankNodeRef<'a>> + 'a {
        self.subjects_for_interned_predicate_object(
            self.dataset.encoded_named_node(predicate),
            self.dataset.encoded_term(object),
        )
    }

    pub(super) fn subjects_for_interned_predicate_object(
        &self,
        predicate: Option<InternedNamedNode>,
        object: Option<InternedTerm>,
    ) -> impl Iterator<Item = NamedOrBlankNodeRef<'a>> + use<'a> {
        let predicate = predicate.unwrap_or_else(InternedNamedNode::impossible);
        let object = object.unwrap_or_else(InternedTerm::impossible);
        let ds = self.dataset;
        self.dataset
            .gpos
            .range(
                &(
                    self.graph_name,
                    predicate,
                    object.clone(),
                    InternedNamedOrBlankNode::first(),
                )
                    ..&(
                        self.graph_name,
                        predicate,
                        object.next(),
                        InternedNamedOrBlankNode::first(),
                    ),
            )
            .map(move |q| q.3.decode_from(&ds.interner))
    }

    pub fn subject_for_predicate_object<'b>(
        &self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> Option<NamedOrBlankNodeRef<'a>> {
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
    ) -> impl Iterator<Item = TripleRef<'a>> + use<'a> {
        let object = object.unwrap_or_else(InternedTerm::impossible);
        let ds = self.dataset;
        self.dataset
            .gosp
            .range(
                &(
                    self.graph_name,
                    object.clone(),
                    InternedNamedOrBlankNode::first(),
                    InternedNamedNode::first(),
                )
                    ..&(
                        self.graph_name,
                        object.next(),
                        InternedNamedOrBlankNode::first(),
                        InternedNamedNode::first(),
                    ),
            )
            .map(move |(_, o, s, p)| ds.decode_spo((s, p, o)))
    }

    /// Checks if the graph contains the given triple.
    pub fn contains<'b>(&self, triple: impl Into<TripleRef<'b>>) -> bool {
        if let Some(triple) = self.encoded_triple(triple.into()) {
            self.dataset.gspo.contains(&(
                self.graph_name,
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
            subject: self.dataset.encoded_named_or_blank_node(triple.subject)?,
            predicate: self.dataset.encoded_named_node(triple.predicate)?,
            object: self.dataset.encoded_term(triple.object)?,
        })
    }
}

impl<'a> IntoIterator for GraphView<'a> {
    type Item = TripleRef<'a>;
    type IntoIter = GraphViewIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &GraphView<'a> {
    type Item = TripleRef<'a>;
    type IntoIter = GraphViewIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl fmt::Display for GraphView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for t in self {
            writeln!(f, "{t} .")?;
        }
        Ok(())
    }
}

/// A read/write view on an [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) contained in a [`Dataset`].
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
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
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
            graph_name: self.graph_name,
        }
    }

    /// Adds a triple to the graph.
    pub fn insert<'b>(&mut self, triple: impl Into<TripleRef<'b>>) -> bool {
        let triple = self.encode_triple(triple.into());
        self.dataset.insert_encoded((
            triple.subject,
            triple.predicate,
            triple.object,
            self.graph_name,
        ))
    }

    /// Removes a concrete triple from the graph.
    pub fn remove<'b>(&mut self, triple: impl Into<TripleRef<'b>>) -> bool {
        if let Some(triple) = self.read().encoded_triple(triple.into()) {
            self.dataset.remove_encoded((
                triple.subject,
                triple.predicate,
                triple.object,
                self.graph_name,
            ))
        } else {
            false
        }
    }

    fn encode_triple(&mut self, triple: TripleRef<'_>) -> InternedTriple {
        InternedTriple {
            subject: InternedNamedOrBlankNode::encoded_into(
                triple.subject,
                &mut self.dataset.interner,
            ),
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
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.read()
            .triples_for_interned_subject(self.dataset.encoded_named_or_blank_node(subject))
    }

    pub fn objects_for_subject_predicate<'b>(
        &'a self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TermRef<'a>> + 'a {
        self.read().objects_for_interned_subject_predicate(
            self.dataset.encoded_named_or_blank_node(subject),
            self.dataset.encoded_named_node(predicate),
        )
    }

    pub fn object_for_subject_predicate<'b>(
        &'a self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> Option<TermRef<'a>> {
        self.read().object_for_subject_predicate(subject, predicate)
    }

    pub fn predicates_for_subject_object<'b>(
        &'a self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = NamedNodeRef<'a>> + 'a {
        self.read().predicates_for_interned_subject_object(
            self.dataset.encoded_named_or_blank_node(subject),
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
    ) -> impl Iterator<Item = NamedOrBlankNodeRef<'a>> + 'a {
        self.read().subjects_for_interned_predicate_object(
            self.dataset.encoded_named_node(predicate),
            self.dataset.encoded_term(object),
        )
    }

    pub fn subject_for_predicate_object<'b>(
        &'a self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> Option<NamedOrBlankNodeRef<'a>> {
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

impl Extend<Triple> for GraphViewMut<'_> {
    fn extend<I: IntoIterator<Item = Triple>>(&mut self, iter: I) {
        for t in iter {
            self.insert(&t);
        }
    }
}

impl<'b, T: Into<TripleRef<'b>>> Extend<T> for GraphViewMut<'_> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for t in iter {
            self.insert(t);
        }
    }
}

impl<'a> IntoIterator for &'a GraphViewMut<'a> {
    type Item = TripleRef<'a>;
    type IntoIter = GraphViewIter<'a>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl fmt::Display for GraphViewMut<'_> {
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
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
    >,
}

impl<'a> Iterator for Iter<'a> {
    type Item = QuadRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
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
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
        ),
    >,
}

impl<'a> Iterator for GraphViewIter<'a> {
    type Item = TripleRef<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner
            .next()
            .map(|(_, s, p, o)| self.dataset.decode_spo((s, p, o)))
    }
}

type QuadsPerBlankNode<'a> = HashMap<
    InternedBlankNode,
    Vec<&'a (
        InternedNamedOrBlankNode,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    )>,
>;

/// An algorithm used to canonicalize graph and datasets.
///
/// See [`Graph::canonicalize`] and [`Dataset::canonicalize`].
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CanonicalizationAlgorithm {
    /// The algorithm preferred by OxRDF.
    ///
    /// <div class="warning">The canonicalization algorithm is not stable and canonical blank node ids might change between versions.</div>
    #[default]
    Unstable,
    /// The [RDF Canonicalization algorithm version 1.0](https://www.w3.org/TR/rdf-canon/#dfn-rdfc-1-0) parametrized with its used [`CanonicalizationHashAlgorithm`](hash algorithm).
    ///
    /// <div class="warning">Note that the algorithm does not support RDF 1.2, this implementation behavior on triple terms is not part of the standard and might change.</div>
    #[cfg(feature = "rdfc-10")]
    Rdfc10 {
        hash_algorithm: CanonicalizationHashAlgorithm,
    },
}

/// The hash function to use to canonicalize graph and datasets.
///
/// See [`Graph::canonicalize`] and [`Dataset::canonicalize`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CanonicalizationHashAlgorithm {
    #[cfg(feature = "rdfc-10")]
    Sha256,
    #[cfg(feature = "rdfc-10")]
    Sha384,
}

/// A RDFC [canonicalization state](https://www.w3.org/TR/rdf-canon/#canon-state)
struct CanonicalizationState<'a> {
    blank_node_to_quads_map: QuadsPerBlankNode<'a>,
    hash_to_blank_nodes_map: BTreeMap<String, Vec<InternedBlankNode>>,
    canonical_issuer: IdentifierIssuer,
}

/// A RDFC [identifier issuer](https://www.w3.org/TR/rdf-canon/#dfn-identifier-issuer)
#[derive(Clone)]
struct IdentifierIssuer {
    identifier_prefix: &'static str,
    identifier_counter: u32,
    issued_identifier_map: HashMap<InternedBlankNode, BlankNode>,
    issued_identifier_order: Vec<InternedBlankNode>, /* hack to know the insertion order in the hash map */
}

impl IdentifierIssuer {
    fn new(identifier_prefix: &'static str) -> Self {
        Self {
            identifier_prefix,
            identifier_counter: 0,
            issued_identifier_map: HashMap::new(),
            issued_identifier_order: Vec::new(),
        }
    }
}

fn generate_permutations<T: Copy>(items: impl IntoIterator<Item = T>) -> Vec<Vec<T>> {
    let mut current_output = vec![Vec::new()];
    for (i, next) in items.into_iter().enumerate() {
        let mut new_output = Vec::with_capacity(current_output.len() * (i + 1));
        for mut permutation in current_output {
            permutation.push(next);
            for j in 0..=i {
                let mut new_permutation = permutation.clone();
                new_permutation.swap(i, j);
                new_output.push(new_permutation);
            }
        }
        current_output = new_output;
    }
    current_output
}

#[cfg(feature = "rdfc-10")]
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canon() {
        let p = NamedNode::new_unchecked("http://example.com/#p");
        let q = NamedNode::new_unchecked("http://example.com/#q");
        let r = NamedNode::new_unchecked("http://example.com/#r");

        let mut dataset = Dataset::new();
        let e0 = BlankNode::new_unchecked("e0");
        let e1 = BlankNode::new_unchecked("e1");
        let e2 = BlankNode::new_unchecked("e2");
        let e3 = BlankNode::new_unchecked("e3");
        dataset.insert(QuadRef::new(&p, &q, &e0, GraphNameRef::DefaultGraph));
        dataset.insert(QuadRef::new(&p, &q, &e1, GraphNameRef::DefaultGraph));
        dataset.insert(QuadRef::new(&e0, &p, &e2, GraphNameRef::DefaultGraph));
        dataset.insert(QuadRef::new(&e1, &p, &e3, GraphNameRef::DefaultGraph));
        dataset.insert(QuadRef::new(&e2, &r, &e3, GraphNameRef::DefaultGraph));
        dataset.canonicalize(CanonicalizationAlgorithm::Rdfc10 {
            hash_algorithm: CanonicalizationHashAlgorithm::Sha256,
        });

        let mut expected = Dataset::new();
        let c14n0 = BlankNode::new_unchecked("c14n0");
        let c14n1 = BlankNode::new_unchecked("c14n1");
        let c14n2 = BlankNode::new_unchecked("c14n2");
        let c14n3 = BlankNode::new_unchecked("c14n3");
        expected.insert(QuadRef::new(&p, &q, &c14n2, GraphNameRef::DefaultGraph));
        expected.insert(QuadRef::new(&p, &q, &c14n3, GraphNameRef::DefaultGraph));
        expected.insert(QuadRef::new(&c14n0, &r, &c14n1, GraphNameRef::DefaultGraph));
        expected.insert(QuadRef::new(&c14n2, &p, &c14n1, GraphNameRef::DefaultGraph));
        expected.insert(QuadRef::new(&c14n3, &p, &c14n0, GraphNameRef::DefaultGraph));
        assert_eq!(dataset, expected);
    }
}
