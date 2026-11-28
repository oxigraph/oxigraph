//! [In-memory implementation](Dataset) of [RDF datasets](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
//!
//! Usage example:
//! ```
//! use oxrdf::*;
//!
//! let mut dataset = Dataset::default();
//!
//! // insertion
//! let ex = NamedNode::new("http://example.com")?;
//! let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone());
//! dataset.insert(quad.clone());
//!
//! // simple filter
//! let results = dataset
//!     .quads_for_subject(&ex.clone().into())
//!     .collect::<Vec<_>>();
//! assert_eq!(vec![quad], results);
//!
//! // direct access to a dataset graph
//! let results = dataset.graph(&ex.clone().into()).iter().collect::<Vec<_>>();
//! assert_eq!(vec![Triple::new(ex.clone(), ex.clone(), ex)], results);
//!
//! // Print
//! assert_eq!(
//!     dataset.to_string(),
//!     "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n"
//! );
//! # Result::<_, Box<dyn std::error::Error>>::Ok(())
//! ```
//!
//! See also [`Graph`](crate::Graph) if you only care about plain triples.

use crate::{BlankNode, GraphName, NamedNode, NamedOrBlankNode, Quad, Term, Triple};
use oxstr::{OxString, oxformat};
#[cfg(feature = "rdfc-10")]
use sha2::{Digest, Sha256, Sha384};
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet, btree_set};
use std::fmt;
use std::hash::{BuildHasher, DefaultHasher, Hash, Hasher};
use std::mem::take;
use std::num::NonZeroUsize;
use std::ops::RangeInclusive;

// Component numbers are S, P, O, G. Every binding mask has an index prefix.
const ORDERS: [[usize; 4]; 6] = [
    [3, 0, 1, 2],
    [3, 1, 2, 0],
    [3, 2, 0, 1],
    [0, 1, 2, 3],
    [1, 2, 0, 3],
    [2, 0, 1, 3],
];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
struct TermId(u32);

impl TermId {
    const MIN: Self = Self(0); // Also the default graph; never a dictionary ID.
    const MAX: Self = Self(u32::MAX); // Reserved exclusively for range bounds.
}

/// An in-memory [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
///
/// It can accommodate a fairly large number of quads (in the few millions).
///
/// Usage example:
/// ```
/// use oxrdf::*;
///
/// let mut dataset = Dataset::default();
///
/// // insertion
/// let ex = NamedNode::new("http://example.com")?;
/// let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone());
/// dataset.insert(quad.clone());
///
/// // simple filter
/// let results: Vec<_> = dataset.quads_for_subject(&ex.clone().into()).collect();
/// assert_eq!(vec![quad], results);
///
/// // direct access to a dataset graph
/// let results: Vec<_> = dataset.graph(&ex.clone().into()).iter().collect();
/// assert_eq!(vec![Triple::new(ex.clone(), ex.clone(), ex)], results);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct Dataset {
    dictionary: Dictionary,
    indexes: [BTreeSet<[TermId; 4]>; 6],
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
    /// let ex = NamedNode::new("http://example.com")?;
    /// dataset.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone()));
    ///
    /// let results: Vec<_> = dataset.graph(&ex.clone().into()).iter().collect();
    /// assert_eq!(vec![Triple::new(ex.clone(), ex.clone(), ex)], results);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn graph(&self, graph_name: &GraphName) -> GraphView<'_> {
        GraphView {
            dataset: self,
            graph_name: self.graph_id(graph_name),
        }
    }

    /// Provides a read/write view on an [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) contained in this dataset.
    ///
    /// ```
    /// use oxrdf::*;
    ///
    /// let mut dataset = Dataset::default();
    /// let ex = NamedNode::new("http://example.com")?;
    ///
    /// // We edit and query the dataset http://example.com graph
    /// {
    ///     let mut graph = dataset.graph_mut(ex.clone());
    ///     graph.insert(Triple::new(ex.clone(), ex.clone(), ex.clone()));
    ///     let results: Vec<_> = graph.iter().collect();
    ///     assert_eq!(
    ///         vec![Triple::new(ex.clone(), ex.clone(), ex.clone())],
    ///         results
    ///     );
    /// }
    ///
    /// // We have also changes the dataset itself
    /// let results: Vec<_> = dataset.iter().collect();
    /// assert_eq!(
    ///     vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone())],
    ///     results
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn graph_mut(&mut self, graph_name: impl Into<GraphName>) -> GraphViewMut<'_> {
        GraphViewMut {
            dataset: self,
            graph_name: graph_name.into(),
        }
    }

    /// Returns all the quads contained by the dataset.
    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            dataset: self,
            inner: self.indexes[3].iter(),
        }
    }

    pub fn quads_for_subject<'a>(
        &'a self,
        subject: &NamedOrBlankNode,
    ) -> impl Iterator<Item = Quad> + use<'a> {
        self.quads_for_pattern(Some(subject), None, None, None)
    }

    pub fn quads_for_predicate<'a>(
        &'a self,
        predicate: &NamedNode,
    ) -> impl Iterator<Item = Quad> + use<'a> {
        self.quads_for_pattern(None, Some(predicate), None, None)
    }

    pub fn quads_for_object<'a>(&'a self, object: &Term) -> impl Iterator<Item = Quad> + use<'a> {
        self.quads_for_pattern(None, None, Some(object), None)
    }

    pub fn quads_for_graph_name<'a>(
        &'a self,
        graph_name: &GraphName,
    ) -> impl Iterator<Item = Quad> + use<'a> {
        self.quads_for_pattern(None, None, None, Some(graph_name))
    }

    /// Retrieves quads with a filter on each quad component
    pub fn quads_for_pattern<'a>(
        &'a self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<&GraphName>,
    ) -> impl Iterator<Item = Quad> + use<'a> {
        self.matching_ids(self.encode_pattern(subject, predicate, object, graph_name))
            .map(|ids| self.decode(ids))
    }

    fn matching_ids(&self, ids: Option<[Option<TermId>; 4]>) -> Matches<'_> {
        let mut result = Matches {
            order: 3,
            inner: btree_set::Range::default(),
        };
        let Some(ids) = ids else {
            return result;
        };
        let bound = ids.iter().filter(|id| id.is_some()).count();
        let order = ORDERS
            .iter()
            .position(|order| {
                order[..bound]
                    .iter()
                    .all(|component| ids[*component].is_some())
            })
            .unwrap();
        let prefix = ORDERS[order].map(|component| ids[component].unwrap_or(TermId::MIN));
        result.order = order;
        result.inner = self.indexes[order].range(prefix_range(&prefix[..bound]));
        result
    }

    /// Checks if the dataset contains the given quad
    pub fn contains(&self, quad: &Quad) -> bool {
        self.encode_quad(quad.as_triple(), &quad.graph_name)
            .is_some_and(|q| self.indexes[3].contains(&q))
    }

    /// Returns the number of quads in this dataset.
    #[inline]
    pub fn len(&self) -> usize {
        self.indexes[3].len()
    }

    /// Checks if this dataset contains a quad.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.indexes[3].is_empty()
    }

    /// Adds a quad to the dataset.
    ///
    /// Panics if more than `u32::MAX - 1` distinct term are in the dataset.
    pub fn insert(&mut self, quad: impl Into<Quad>) -> bool {
        let quad = quad.into();
        let ids = [
            self.dictionary.acquire(quad.subject.into()),
            self.dictionary.acquire(quad.predicate.into()),
            self.dictionary.acquire(quad.object),
            match quad.graph_name {
                GraphName::DefaultGraph => TermId::MIN,
                GraphName::NamedNode(n) => self.dictionary.acquire(n.into()),
                GraphName::BlankNode(n) => self.dictionary.acquire(n.into()),
            },
        ];
        if !self.indexes[3].insert(ids) {
            for id in ids {
                self.dictionary.release(id);
            }
            return false;
        }
        for (order, index) in self.indexes.iter_mut().enumerate() {
            index.insert(reorder(ids, order));
        }
        true
    }

    /// Removes a concrete quad from the dataset.
    pub fn remove(&mut self, quad: &Quad) -> bool {
        let Some(ids) = self.encode_quad(quad.as_triple(), &quad.graph_name) else {
            return false;
        };
        self.remove_ids(ids)
    }

    fn remove_ids(&mut self, ids: [TermId; 4]) -> bool {
        if !self.indexes[3].remove(&ids) {
            return false;
        }
        for order in [0, 1, 2, 4, 5] {
            self.indexes[order].remove(&reorder(ids, order));
        }
        // Only recycle IDs after every index has stopped referring to them.
        for id in ids {
            self.dictionary.release(id);
        }
        true
    }

    /// Clears the dataset.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    fn encode_pattern(
        &self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
        graph_name: Option<&GraphName>,
    ) -> Option<[Option<TermId>; 4]> {
        Some([
            subject.map_or(Some(None), |s| self.subject_id(s).map(Some))?,
            predicate.map_or(Some(None), |p| self.dictionary.find_named_node(p).map(Some))?,
            object.map_or(Some(None), |o| self.dictionary.find(o).map(Some))?,
            graph_name.map_or(Some(None), |g| self.graph_id(g).map(Some))?,
        ])
    }

    fn encode_quad(&self, triple: &Triple, graph_name: &GraphName) -> Option<[TermId; 4]> {
        Some([
            self.subject_id(&triple.subject)?,
            self.dictionary.find_named_node(&triple.predicate)?,
            self.dictionary.find(&triple.object)?,
            self.graph_id(graph_name)?,
        ])
    }

    fn subject_id(&self, subject: &NamedOrBlankNode) -> Option<TermId> {
        match subject {
            NamedOrBlankNode::NamedNode(n) => self.dictionary.find_named_node(n),
            NamedOrBlankNode::BlankNode(n) => self.dictionary.find_blank_node(n),
        }
    }

    fn subject(&self, id: TermId) -> NamedOrBlankNode {
        match self.dictionary.term(id) {
            Term::NamedNode(n) => NamedOrBlankNode::NamedNode(n.clone()),
            Term::BlankNode(n) => NamedOrBlankNode::BlankNode(n.clone()),
            Term::Literal(_) => unreachable!("quad subjects are named or blank nodes"),
            #[cfg(feature = "rdf-12")]
            Term::Triple(_) => unreachable!("quad subjects are named or blank nodes"),
        }
    }

    fn predicate(&self, id: TermId) -> NamedNode {
        let Term::NamedNode(n) = self.dictionary.term(id) else {
            unreachable!("quad predicates are named nodes")
        };
        n.clone()
    }

    #[inline]
    fn graph_id(&self, graph: &GraphName) -> Option<TermId> {
        match graph {
            GraphName::DefaultGraph => Some(TermId::MIN),
            GraphName::NamedNode(n) => self.dictionary.find_named_node(n),
            GraphName::BlankNode(n) => self.dictionary.find_blank_node(n),
        }
    }

    fn decode(&self, ids: [TermId; 4]) -> Quad {
        let graph_name = if ids[3] == TermId::MIN {
            GraphName::DefaultGraph
        } else {
            self.subject(ids[3]).into()
        };
        self.decode_triple(ids).in_graph(graph_name)
    }

    fn decode_triple(&self, ids: [TermId; 4]) -> Triple {
        Triple {
            subject: self.subject(ids[0]),
            predicate: self.predicate(ids[1]),
            object: self.dictionary.term(ids[2]).clone(),
        }
    }

    /// Canonicalizes the dataset by renaming blank nodes.
    ///
    /// Usage example ([Dataset isomorphism](https://www.w3.org/TR/rdf11-concepts/#dfn-dataset-isomorphism)):
    /// ```
    /// use oxrdf::dataset::CanonicalizationAlgorithm;
    /// use oxrdf::*;
    ///
    /// let iri = NamedNode::new("http://example.com")?;
    ///
    /// let mut graph1 = Graph::new();
    /// let bnode1 = BlankNode::default();
    /// graph1.insert(Triple::new(iri.clone(), iri.clone(), bnode1.clone()));
    /// graph1.insert(Triple::new(bnode1, iri.clone(), iri.clone()));
    ///
    /// let mut graph2 = Graph::new();
    /// let bnode2 = BlankNode::default();
    /// graph2.insert(Triple::new(iri.clone(), iri.clone(), bnode2.clone()));
    /// graph2.insert(Triple::new(bnode2, iri.clone(), iri));
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
        let quads: Vec<_> = self.iter().collect();
        let mapping = canonicalize_blank_nodes(&quads, algorithm);
        self.clear();
        self.extend(quads.into_iter().map(|mut quad| {
            rename_quad_blank_nodes(&mut quad, &|node| mapping[node].clone());
            quad
        }));
    }

    /// Returns a map between the current dataset blank node and the canonicalized blank node
    /// to create a canonical dataset.
    ///
    /// See also [`canonicalize`](Self::canonicalize).
    pub fn canonicalize_blank_nodes(
        &self,
        algorithm: CanonicalizationAlgorithm,
    ) -> HashMap<BlankNode, BlankNode> {
        canonicalize_blank_nodes(&self.iter().collect::<Vec<_>>(), algorithm)
    }
}

#[derive(Clone, Debug)]
struct TermEntry {
    term: Term,
    references: NonZeroUsize,
}

#[derive(Clone, Debug, Default)]
struct Dictionary {
    // Most buckets contain one ID. Collisions are resolved by term equality,
    // not by changing IDs or assuming hashes uniquely identify terms.
    by_hash: HashMap<u64, Vec<TermId>>, // TODO: avoid the nested Vec?
    entries: Vec<Option<TermEntry>>,
    free: Vec<TermId>,
}

impl Dictionary {
    fn term(&self, id: TermId) -> &Term {
        &self.entries[(id.0 - 1) as usize].as_ref().unwrap().term
    }

    fn hash(&self, term: &Term) -> u64 {
        if let Term::NamedNode(node) = term {
            self.hash_named_node(node)
        } else if let Term::BlankNode(node) = term {
            self.hash_blank_node(node)
        } else {
            self.by_hash.hasher().hash_one(term)
        }
    }

    // Look up nodes by their own type without building a temporary Term.
    fn hash_named_node(&self, node: &NamedNode) -> u64 {
        self.by_hash.hasher().hash_one((0_u8, node))
    }

    fn hash_blank_node(&self, node: &BlankNode) -> u64 {
        self.by_hash.hasher().hash_one((1_u8, node))
    }

    fn find(&self, term: &Term) -> Option<TermId> {
        self.find_by(self.hash(term), |candidate| candidate == term)
    }

    fn find_named_node(&self, node: &NamedNode) -> Option<TermId> {
        self.find_by(self.hash_named_node(node), |term| term == node)
    }

    fn find_blank_node(&self, node: &BlankNode) -> Option<TermId> {
        self.find_by(self.hash_blank_node(node), |term| term == node)
    }

    fn find_by(&self, hash: u64, matches: impl Fn(&Term) -> bool) -> Option<TermId> {
        self.by_hash
            .get(&hash)?
            .iter()
            .copied()
            .find(|id| matches(self.term(*id)))
    }

    fn acquire(&mut self, term: Term) -> TermId {
        let hash = self.hash(&term);
        #[expect(clippy::expect_used)]
        if let Some(id) = self.find_by(hash, |candidate| candidate == &term) {
            let entry = self.entries[(id.0 - 1) as usize].as_mut().unwrap();
            entry.references = entry
                .references
                .checked_add(1)
                .expect("term reference count overflow");
            return id;
        }
        let id = self.free.pop().unwrap_or_else(|| {
            let id = TermId(u32::try_from(self.entries.len() + 1).unwrap());
            assert!(id < TermId::MAX, "term dictionary exhausted its IDs");
            self.entries.push(None);
            id
        });
        self.entries[(id.0 - 1) as usize] = Some(TermEntry {
            term,
            references: NonZeroUsize::MIN,
        });
        self.by_hash.entry(hash).or_default().push(id);
        id
    }

    fn release(&mut self, id: TermId) {
        if id == TermId::MIN {
            return;
        } // The default graph has no owned term.
        let slot = &mut self.entries[(id.0 - 1) as usize];
        let entry = slot.as_mut().unwrap();
        if let Some(references) = NonZeroUsize::new(entry.references.get() - 1) {
            entry.references = references;
            return;
        }
        // We have 0 referenvces, we GC
        let entry = slot.take().unwrap();
        let hash = self.hash(&entry.term);
        let bucket = self.by_hash.get_mut(&hash).unwrap();
        bucket.swap_remove(
            bucket
                .iter()
                .position(|candidate| *candidate == id)
                .unwrap(),
        );
        if bucket.is_empty() {
            self.by_hash.remove(&hash);
        }
        self.free.push(id);
    }
}

fn reorder(quad: [TermId; 4], order: usize) -> [TermId; 4] {
    ORDERS[order].map(|component| quad[component])
}

fn prefix_range(prefix: &[TermId]) -> RangeInclusive<[TermId; 4]> {
    let mut lower = [TermId::MIN; 4];
    let mut upper = [TermId::MAX; 4];
    lower[..prefix.len()].copy_from_slice(prefix);
    upper[..prefix.len()].copy_from_slice(prefix);
    lower..=upper
}

fn canonicalize_blank_nodes(
    quads: &[Quad],
    algorithm: CanonicalizationAlgorithm,
) -> HashMap<BlankNode, BlankNode> {
    Canonicalization { quads }
        .canonicalize_blank_nodes(algorithm)
        .into_iter()
        .map(|(from, to)| (from.clone(), to))
        .collect()
}

struct Canonicalization<'a> {
    quads: &'a [Quad],
}

impl<'a> Canonicalization<'a> {
    fn canonicalize_blank_nodes(
        &self,
        algorithm: CanonicalizationAlgorithm,
    ) -> HashMap<&'a BlankNode, BlankNode> {
        let hash_algorithm = match algorithm {
            CanonicalizationAlgorithm::Unstable | CanonicalizationAlgorithm::UnstableHashedIds => {
                None
            }
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
        for quad in self.quads {
            if let NamedOrBlankNode::BlankNode(bnode) = &quad.subject {
                Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                    bnode,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
            if let Term::BlankNode(bnode) = &quad.object {
                Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                    bnode,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
            #[cfg(feature = "rdf-12")]
            if let Term::Triple(t) = &quad.object {
                Self::add_quad_to_blank_node_to_quads_map_based_on_triple(
                    t,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
            if let GraphName::BlankNode(bnode) = &quad.graph_name {
                Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                    bnode,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
        }
        // 3)
        for n in canonicalization_state.blank_node_to_quads_map.keys() {
            // 3.1)
            let hash = Self::hash_first_degree_quads(&canonicalization_state, n, hash_algorithm);
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
            .filter(|(hash, identifier_list)| {
                match identifier_list.len() {
                    0 => unreachable!(),
                    // 4.1)
                    2.. => true,
                    1 => {
                        // 4.2)
                        Self::issue_identifier(
                            &mut canonicalization_state.canonical_issuer,
                            identifier_list[0],
                            algorithm,
                            hash,
                        );
                        // 4.3)
                        false
                    }
                }
            })
            .collect::<BTreeMap<_, _>>();
        // 5)
        for (hash, identifier_list) in take(&mut canonicalization_state.hash_to_blank_nodes_map) {
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
                Self::issue_identifier(&mut temporary_issuer, n, algorithm, &hash);
                // 5.2.4)
                hash_path_list.push(Self::hash_n_degree_quads(
                    &canonicalization_state,
                    n,
                    &temporary_issuer,
                    algorithm,
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
                        algorithm,
                        &hash,
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
    fn add_quad_to_blank_node_to_quads_map_based_on_triple(
        triple: &'a Triple,
        quad: &'a Quad,
        blank_node_to_quads_map: &mut QuadsPerBlankNode<'a>,
    ) {
        if let NamedOrBlankNode::BlankNode(bnode) = &triple.subject {
            Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                bnode,
                quad,
                blank_node_to_quads_map,
            );
        }
        if let Term::BlankNode(bnode) = &triple.object {
            Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                bnode,
                quad,
                blank_node_to_quads_map,
            );
        } else if let Term::Triple(t) = &triple.object {
            Self::add_quad_to_blank_node_to_quads_map_based_on_triple(
                t,
                quad,
                blank_node_to_quads_map,
            );
        }
    }

    fn add_quad_to_blank_node_to_quads_map_for_blank_node(
        bnode: &'a BlankNode,
        quad: &'a Quad,
        blank_node_to_quads_map: &mut QuadsPerBlankNode<'a>,
    ) {
        let entry = blank_node_to_quads_map.entry(bnode).or_default();
        if !entry.ends_with(&[quad]) {
            entry.push(quad);
        }
    }

    /// RDFC [Issue Identifier Algorithm](https://www.w3.org/TR/rdf-canon/#issue-identifier)
    fn issue_identifier(
        issuer: &mut IdentifierIssuer<'a>,
        blank_node: &'a BlankNode,
        algorithm: CanonicalizationAlgorithm,
        hash: &str,
    ) -> BlankNode {
        match issuer.issued_identifier_map.entry(blank_node) {
            // 1)
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                if algorithm == CanonicalizationAlgorithm::UnstableHashedIds {
                    let mut issued_identifier = BlankNode::new_unchecked(OxString::new_owned(hash));
                    let mut i = 0;
                    while !issuer
                        .already_issued_identifiers
                        .insert(issued_identifier.clone())
                    {
                        i += 1;
                        issued_identifier = BlankNode::new_unchecked(oxformat!("{hash}{i}"));
                    }
                    entry.insert(issued_identifier.clone());
                    issuer.issued_identifier_order.push(blank_node);
                    issued_identifier
                } else {
                    // 2)
                    let issued_identifier = BlankNode::new_unchecked(oxformat!(
                        "{}{}",
                        issuer.identifier_prefix,
                        issuer.identifier_counter
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
    }

    /// RDFC [Hash First Degree Quads](https://www.w3.org/TR/rdf-canon/#hash-1d-quads)
    fn hash_first_degree_quads(
        canonicalization_state: &CanonicalizationState<'a>,
        reference_blank_node_identifier: &'a BlankNode,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) -> String {
        // 1)
        let mut nquads = Vec::new();
        // 2)
        let quads =
            &canonicalization_state.blank_node_to_quads_map[&reference_blank_node_identifier];
        // 3)
        for &quad in quads {
            // 3.1)
            let mut quad = quad.clone();
            rename_quad_blank_nodes(&mut quad, &|node| {
                BlankNode::new_unchecked(if node == reference_blank_node_identifier {
                    "a"
                } else {
                    "z"
                })
            });
            nquads.push(format!("{quad} .\n"));
        }
        // 3)
        nquads.sort();
        // 4)
        Self::hash_function(&nquads.join(""), hash_algorithm)
    }

    /// RDFC [Hash Related Blank Node](https://www.w3.org/TR/rdf-canon/#hash-related-blank-node)
    fn hash_related_blank_node(
        canonicalization_state: &CanonicalizationState<'a>,
        related: &'a BlankNode,
        quad: &'a Quad,
        issuer: &IdentifierIssuer<'a>,
        position: &str,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) -> String {
        // 1)
        let mut input = position.to_owned();
        // 2)
        if position != "g" {
            input.push('<');
            input.push_str(quad.predicate.as_str());
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
            input.push_str(&Self::hash_first_degree_quads(
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
        canonicalization_state: &CanonicalizationState<'a>,
        identifier: &'a BlankNode,
        issuer: &IdentifierIssuer<'a>,
        algorithm: CanonicalizationAlgorithm,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) -> (IdentifierIssuer<'a>, String) {
        let mut issuer = issuer.clone();
        // 1)
        let mut h_n = BTreeMap::<_, HashSet<_>>::new();
        // 2)
        let quads = &canonicalization_state.blank_node_to_quads_map[&identifier];
        // 3)
        for quad in quads {
            // 3.1)
            if let NamedOrBlankNode::BlankNode(component) = &quad.subject {
                Self::hash_related_blank_node_on_possible_component(
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
            if let Term::BlankNode(component) = &quad.object {
                Self::hash_related_blank_node_on_possible_component(
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
            if let Term::Triple(t) = &quad.object {
                Self::hash_related_blank_node_on_possible_triple(
                    canonicalization_state,
                    t,
                    identifier,
                    quad,
                    &issuer,
                    &mut h_n,
                    hash_algorithm,
                );
            }
            if let GraphName::BlankNode(component) = &quad.graph_name {
                Self::hash_related_blank_node_on_possible_component(
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
                        let id = Self::issue_identifier(
                            &mut issuer_copy,
                            related,
                            algorithm,
                            &related_hash,
                        );
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
                    let (result_identifier_issuer, result_hash) = Self::hash_n_degree_quads(
                        canonicalization_state,
                        related,
                        &issuer_copy,
                        algorithm,
                        hash_algorithm,
                    );
                    // 5.4.5.2)
                    let id =
                        Self::issue_identifier(&mut issuer_copy, related, algorithm, &result_hash);
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
        canonicalization_state: &CanonicalizationState<'a>,
        triple: &'a Triple,
        identifier: &'a BlankNode,
        quad: &'a Quad,
        issuer: &IdentifierIssuer<'a>,
        h_n: &mut BTreeMap<String, HashSet<&'a BlankNode>>,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) {
        if let NamedOrBlankNode::BlankNode(component) = &triple.subject {
            Self::hash_related_blank_node_on_possible_component(
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
        if let Term::BlankNode(component) = &triple.object {
            Self::hash_related_blank_node_on_possible_component(
                canonicalization_state,
                component,
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
        canonicalization_state: &CanonicalizationState<'a>,
        component: &'a BlankNode,
        identifier: &'a BlankNode,
        quad: &'a Quad,
        issuer: &IdentifierIssuer<'a>,
        position: &str,
        h_n: &mut BTreeMap<String, HashSet<&'a BlankNode>>,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) {
        if component != identifier {
            // 3.1.1)
            let hash = Self::hash_related_blank_node(
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
}

fn rename_quad_blank_nodes(quad: &mut Quad, rename: &impl Fn(&BlankNode) -> BlankNode) {
    rename_triple_blank_nodes(&mut quad.subject, &mut quad.object, rename);
    if let GraphName::BlankNode(node) = &mut quad.graph_name {
        *node = rename(node);
    }
}

fn rename_triple_blank_nodes(
    subject: &mut NamedOrBlankNode,
    object: &mut Term,
    rename: &impl Fn(&BlankNode) -> BlankNode,
) {
    if let NamedOrBlankNode::BlankNode(node) = subject {
        *node = rename(node);
    }
    if let Term::BlankNode(node) = object {
        *node = rename(node);
    }
    #[cfg(feature = "rdf-12")]
    if let Term::Triple(triple) = object {
        rename_triple_blank_nodes(&mut triple.subject, &mut triple.object, rename);
    }
}

impl PartialEq for Dataset {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len() && self.iter().all(|q| other.contains(&q))
    }
}
impl Eq for Dataset {}

impl<'a> IntoIterator for &'a Dataset {
    type Item = Quad;
    type IntoIter = Iter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<Q: Into<Quad>> FromIterator<Q> for Dataset {
    fn from_iter<I: IntoIterator<Item = Q>>(iter: I) -> Self {
        let mut result = Self::new();
        result.extend(iter);
        result
    }
}

impl<Q: Into<Quad>> Extend<Q> for Dataset {
    fn extend<I: IntoIterator<Item = Q>>(&mut self, iter: I) {
        for quad in iter {
            self.insert(quad);
        }
    }
}

impl fmt::Display for Dataset {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for quad in self {
            writeln!(f, "{quad} .")?;
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
/// let ex = NamedNode::new("http://example.com")?;
/// dataset.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), ex.clone()));
///
/// let results: Vec<_> = dataset.graph(&ex.clone().into()).iter().collect();
/// assert_eq!(vec![Triple::new(ex.clone(), ex.clone(), ex)], results);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone, Debug)]
pub struct GraphView<'a> {
    dataset: &'a Dataset,
    graph_name: Option<TermId>,
}

impl<'a> GraphView<'a> {
    /// Returns all the triples contained by the graph.
    #[inline]
    pub fn iter(&self) -> GraphViewIter<'a> {
        GraphViewIter {
            dataset: self.dataset,
            inner: self.matching(None, None, None),
        }
    }

    pub fn triples_for_subject(
        &self,
        subject: &NamedOrBlankNode,
    ) -> impl Iterator<Item = Triple> + use<'a> {
        self.triples_for_pattern(Some(subject), None, None)
    }

    pub fn objects_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> impl Iterator<Item = Term> + use<'a> {
        let dataset = self.dataset;
        self.matching(Some(subject), Some(predicate), None)
            .map(move |ids| dataset.dictionary.term(ids[2]).clone())
    }

    pub fn object_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> Option<Term> {
        self.objects_for_subject_predicate(subject, predicate)
            .next()
    }

    pub fn predicates_for_subject_object(
        &self,
        subject: &NamedOrBlankNode,
        object: &Term,
    ) -> impl Iterator<Item = NamedNode> + use<'a> {
        let dataset = self.dataset;
        self.matching(Some(subject), None, Some(object))
            .map(move |ids| dataset.predicate(ids[1]))
    }

    pub fn triples_for_predicate(
        &self,
        predicate: &NamedNode,
    ) -> impl Iterator<Item = Triple> + use<'a> {
        self.triples_for_pattern(None, Some(predicate), None)
    }

    pub fn subjects_for_predicate_object(
        &self,
        predicate: &NamedNode,
        object: &Term,
    ) -> impl Iterator<Item = NamedOrBlankNode> + use<'a> {
        let dataset = self.dataset;
        self.matching(None, Some(predicate), Some(object))
            .map(move |ids| dataset.subject(ids[0]))
    }

    pub fn subject_for_predicate_object(
        &self,
        predicate: &NamedNode,
        object: &Term,
    ) -> Option<NamedOrBlankNode> {
        self.subjects_for_predicate_object(predicate, object).next()
    }

    pub fn triples_for_object(&self, object: &Term) -> impl Iterator<Item = Triple> + use<'a> {
        self.triples_for_pattern(None, None, Some(object))
    }

    /// Retrieves triples with a filter on each triple component
    pub fn triples_for_pattern(
        &self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
    ) -> impl Iterator<Item = Triple> + use<'a> {
        GraphViewIter {
            dataset: self.dataset,
            inner: self.matching(subject, predicate, object),
        }
    }

    /// Checks if the graph contains the given triple.
    pub fn contains(&self, triple: &Triple) -> bool {
        self.matching(
            Some(&triple.subject),
            Some(&triple.predicate),
            Some(&triple.object),
        )
        .next()
        .is_some()
    }

    /// Returns the number of triples in this graph.
    #[inline]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Checks if this graph contains a triple.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.matching(None, None, None).next().is_none()
    }

    fn matching(
        &self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
    ) -> Matches<'a> {
        let ids = self.graph_name.and_then(|graph| {
            let mut ids = self
                .dataset
                .encode_pattern(subject, predicate, object, None)?;
            ids[3] = Some(graph);
            Some(ids)
        });
        self.dataset.matching_ids(ids)
    }
}

impl fmt::Display for GraphView<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for triple in self {
            writeln!(f, "{triple} .")?;
        }
        Ok(())
    }
}

impl<'a> IntoIterator for GraphView<'a> {
    type Item = Triple;
    type IntoIter = GraphViewIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &GraphView<'a> {
    type Item = Triple;
    type IntoIter = GraphViewIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
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
/// let ex = NamedNode::new("http://example.com")?;
///
/// // We edit and query the dataset http://example.com graph
/// {
///     let mut graph = dataset.graph_mut(ex.clone());
///     graph.insert(Triple::new(ex.clone(), ex.clone(), ex.clone()));
///     let results: Vec<_> = graph.iter().collect();
///     assert_eq!(
///         vec![Triple::new(ex.clone(), ex.clone(), ex.clone())],
///         results
///     );
/// }
///
/// // We have also changes the dataset itself
/// let results: Vec<_> = dataset.iter().collect();
/// assert_eq!(
///     vec![Quad::new(ex.clone(), ex.clone(), ex.clone(), ex)],
///     results
/// );
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Debug)]
pub struct GraphViewMut<'a> {
    dataset: &'a mut Dataset,
    graph_name: GraphName,
}
impl GraphViewMut<'_> {
    fn read(&self) -> GraphView<'_> {
        self.dataset.graph(&self.graph_name)
    }

    /// Adds a triple to the graph.
    pub fn insert(&mut self, triple: impl Into<Triple>) -> bool {
        self.dataset
            .insert(triple.into().in_graph(self.graph_name.clone()))
    }

    /// Removes a concrete triple from the graph.
    pub fn remove(&mut self, triple: &Triple) -> bool {
        let Some(ids) = self.dataset.encode_quad(triple, &self.graph_name) else {
            return false;
        };
        self.dataset.remove_ids(ids)
    }

    /// Returns all the triples contained by the graph
    #[inline]
    pub fn iter(&self) -> GraphViewIter<'_> {
        self.read().iter()
    }

    pub fn triples_for_subject<'a>(
        &'a self,
        subject: &NamedOrBlankNode,
    ) -> impl Iterator<Item = Triple> + use<'a> {
        self.read().triples_for_subject(subject)
    }

    pub fn objects_for_subject_predicate<'a>(
        &'a self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> impl Iterator<Item = Term> + use<'a> {
        self.read()
            .objects_for_subject_predicate(subject, predicate)
    }

    pub fn object_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> Option<Term> {
        self.read().object_for_subject_predicate(subject, predicate)
    }

    pub fn predicates_for_subject_object<'a>(
        &'a self,
        subject: &NamedOrBlankNode,
        object: &Term,
    ) -> impl Iterator<Item = NamedNode> + use<'a> {
        self.read().predicates_for_subject_object(subject, object)
    }

    pub fn triples_for_predicate<'a>(
        &'a self,
        predicate: &NamedNode,
    ) -> impl Iterator<Item = Triple> + use<'a> {
        self.read().triples_for_predicate(predicate)
    }

    pub fn subjects_for_predicate_object<'a>(
        &'a self,
        predicate: &NamedNode,
        object: &Term,
    ) -> impl Iterator<Item = NamedOrBlankNode> + use<'a> {
        self.read().subjects_for_predicate_object(predicate, object)
    }

    pub fn subject_for_predicate_object(
        &self,
        predicate: &NamedNode,
        object: &Term,
    ) -> Option<NamedOrBlankNode> {
        self.read().subject_for_predicate_object(predicate, object)
    }

    pub fn triples_for_object<'a>(
        &'a self,
        object: &Term,
    ) -> impl Iterator<Item = Triple> + use<'a> {
        self.read().triples_for_object(object)
    }

    pub fn triples_for_pattern<'a>(
        &'a self,
        subject: Option<&NamedOrBlankNode>,
        predicate: Option<&NamedNode>,
        object: Option<&Term>,
    ) -> impl Iterator<Item = Triple> + use<'a> {
        self.read().triples_for_pattern(subject, predicate, object)
    }

    /// Checks if the graph contains the given triple.
    pub fn contains(&self, triple: &Triple) -> bool {
        self.read().contains(triple)
    }

    /// Returns the number of triples in this graph.
    #[inline]
    pub fn len(&self) -> usize {
        self.read().len()
    }

    /// Checks if this graph contains a triple.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }
}

impl<T: Into<Triple>> Extend<T> for GraphViewMut<'_> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        for triple in iter {
            self.insert(triple);
        }
    }
}

impl fmt::Display for GraphViewMut<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.read().fmt(f)
    }
}

impl<'a> IntoIterator for &'a GraphViewMut<'_> {
    type Item = Triple;
    type IntoIter = GraphViewIter<'a>;

    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

struct Matches<'a> {
    order: usize,
    inner: btree_set::Range<'a, [TermId; 4]>,
}

impl Iterator for Matches<'_> {
    type Item = [TermId; 4];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let indexed = self.inner.next()?;
        let mut ids = [TermId::MIN; 4];
        for (position, component) in ORDERS[self.order].iter().enumerate() {
            ids[*component] = indexed[position];
        }
        Some(ids)
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.count()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// Iterator returned by [`Dataset::iter`].
pub struct Iter<'a> {
    dataset: &'a Dataset,
    inner: btree_set::Iter<'a, [TermId; 4]>,
}
impl Iterator for Iter<'_> {
    type Item = Quad;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|ids| self.dataset.decode(*ids))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl ExactSizeIterator for Iter<'_> {}

/// Iterator returned by [`GraphView::iter`].
pub struct GraphViewIter<'a> {
    dataset: &'a Dataset,
    inner: Matches<'a>,
}

impl Iterator for GraphViewIter<'_> {
    type Item = Triple;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|ids| self.dataset.decode_triple(ids))
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.count()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// An algorithm used to canonicalize graph and datasets.
///
/// See [`Graph::canonicalize`](crate::Graph::canonicalize) and [`Dataset::canonicalize`].
#[derive(Default, Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CanonicalizationAlgorithm {
    /// The algorithm preferred by OxRDF.
    ///
    /// <div class="warning">The canonicalization algorithm is not stable and canonical blank node ids might change between versions.</div>
    #[default]
    Unstable,
    /// The algorithm preferred by OxRDF but outputting ids based on hashes.
    ///
    /// This enables to use the blank node ids for diffing, additions or deletions of triples affect less blank node ids.
    ///
    /// <div class="warning">The canonicalization algorithm is not stable and canonical blank node ids might change between versions.</div>
    UnstableHashedIds,
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
/// See [`Graph::canonicalize`](crate::Graph::canonicalize) and [`Dataset::canonicalize`].
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[non_exhaustive]
pub enum CanonicalizationHashAlgorithm {
    #[cfg(feature = "rdfc-10")]
    Sha256,
    #[cfg(feature = "rdfc-10")]
    Sha384,
}

type QuadsPerBlankNode<'a> = HashMap<&'a BlankNode, Vec<&'a Quad>>;

/// A RDFC [canonicalization state](https://www.w3.org/TR/rdf-canon/#canon-state)
struct CanonicalizationState<'a> {
    blank_node_to_quads_map: QuadsPerBlankNode<'a>,
    hash_to_blank_nodes_map: BTreeMap<String, Vec<&'a BlankNode>>,
    canonical_issuer: IdentifierIssuer<'a>,
}

/// A RDFC [identifier issuer](https://www.w3.org/TR/rdf-canon/#dfn-identifier-issuer)
#[derive(Clone)]
struct IdentifierIssuer<'a> {
    identifier_prefix: &'static str,
    identifier_counter: u32,
    issued_identifier_map: HashMap<&'a BlankNode, BlankNode>,
    issued_identifier_order: Vec<&'a BlankNode>, /* hack to know the insertion order in the hash map */
    already_issued_identifiers: HashSet<BlankNode>,
}

impl IdentifierIssuer<'_> {
    fn new(identifier_prefix: &'static str) -> Self {
        Self {
            identifier_prefix,
            identifier_counter: 0,
            issued_identifier_map: HashMap::new(),
            issued_identifier_order: Vec::new(),
            already_issued_identifiers: HashSet::new(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Literal, NamedOrBlankNode};
    use std::collections::HashSet;

    fn data() -> Vec<Quad> {
        let subjects: Vec<NamedOrBlankNode> = vec![
            NamedNode::new_unchecked("http://example.com/a").into(),
            NamedNode::new_unchecked("http://example.com/b").into(),
            BlankNode::new_unchecked("a").into(),
        ];
        let predicates = [
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/q"),
        ];
        let objects: Vec<Term> = vec![
            NamedNode::new_unchecked("http://example.com/a").into(),
            BlankNode::new_unchecked("a").into(),
            Literal::from("a").into(),
            Literal::new_language_tagged_unchecked("a", "en").into(),
            Literal::new_typed_unchecked("a", NamedNode::new_unchecked("http://example.com/type"))
                .into(),
        ];
        #[cfg(feature = "rdf-12")]
        let objects = {
            let mut rdf12_objects = objects;
            rdf12_objects.push(
                Literal::new_directional_language_tagged_unchecked(
                    "a",
                    "en",
                    crate::BaseDirection::Ltr,
                )
                .into(),
            );
            rdf12_objects.push(
                Triple::new(
                    subjects[0].clone(),
                    predicates[0].clone(),
                    BlankNode::new_unchecked("nested"),
                )
                .into(),
            );
            rdf12_objects
        };
        let graphs = [
            GraphName::DefaultGraph,
            NamedNode::new_unchecked("http://example.com/a").into(),
            BlankNode::new_unchecked("a").into(),
        ];
        let mut quads = Vec::new();
        for subject in &subjects {
            for predicate in &predicates {
                for object in &objects {
                    for graph in &graphs {
                        quads.push(Quad::new(
                            subject.clone(),
                            predicate.clone(),
                            object.clone(),
                            graph.clone(),
                        ));
                    }
                }
            }
        }
        quads
    }

    fn check(dataset: &Dataset, expected: &HashSet<Quad>, queries: &[Quad]) {
        assert_eq!(dataset.len(), expected.len());
        assert_eq!(dataset.iter().collect::<HashSet<_>>(), *expected);
        let mut references = HashMap::new();
        for ids in &dataset.indexes[3] {
            for id in ids {
                if *id != TermId::MIN {
                    *references.entry(*id).or_insert(0) += 1;
                }
            }
        }
        assert_eq!(
            references.len(),
            dataset
                .dictionary
                .by_hash
                .values()
                .map(Vec::len)
                .sum::<usize>()
        );
        for bucket in dataset.dictionary.by_hash.values() {
            for id in bucket {
                let entry = dataset.dictionary.entries[(id.0 - 1) as usize]
                    .as_ref()
                    .unwrap();
                assert_eq!(entry.references.get(), references[id]);
                assert_eq!(dataset.dictionary.find(&entry.term), Some(*id));
            }
        }
        for order in 0..6 {
            assert_eq!(dataset.indexes[order].len(), expected.len());
            let expected_ids: BTreeSet<_> = dataset.indexes[3]
                .iter()
                .map(|ids| reorder(*ids, order))
                .collect();
            assert_eq!(dataset.indexes[order], expected_ids);
        }
        for query in queries {
            for mask in 0_u8..16 {
                let expected: HashSet<_> = expected
                    .iter()
                    .filter(|quad| {
                        (mask & 1 == 0 || query.subject == quad.subject)
                            && (mask & 2 == 0 || query.predicate == quad.predicate)
                            && (mask & 4 == 0 || query.object == quad.object)
                            && (mask & 8 == 0 || query.graph_name == quad.graph_name)
                    })
                    .cloned()
                    .collect();
                let iter = dataset.quads_for_pattern(
                    (mask & 1 != 0).then_some(&query.subject),
                    (mask & 2 != 0).then_some(&query.predicate),
                    (mask & 4 != 0).then_some(&query.object),
                    (mask & 8 != 0).then_some(&query.graph_name),
                );
                let actual: Vec<_> = iter.collect();
                assert_eq!(actual.len(), expected.len());
                assert_eq!(
                    actual.into_iter().collect::<HashSet<_>>(),
                    expected,
                    "mask {mask}"
                );
            }
        }
    }

    #[test]
    fn all_patterns_with_churn() {
        let quads = data();
        let missing = Quad::new(
            NamedNode::new_unchecked("http://missing/s"),
            NamedNode::new_unchecked("http://missing/p"),
            Literal::from("missing"),
            NamedNode::new_unchecked("http://missing/g"),
        );
        let queries = [
            quads[0].clone(),
            quads[quads.len() / 2].clone(),
            quads[quads.len() - 1].clone(),
            missing,
        ];
        let mut dataset = Dataset::new();
        let mut expected = HashSet::new();
        let mut random = 42_u64;
        for _ in 0..600 {
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let query = &quads[((random >> 32) as usize) % quads.len()];
            if random.trailing_zeros() >= 2 {
                assert_eq!(dataset.remove(query), expected.remove(query));
            } else {
                assert_eq!(
                    dataset.insert(query.clone()),
                    expected.insert(query.clone())
                );
            }
            check(&dataset, &expected, &queries);
        }
        dataset.extend(quads.iter().cloned());
        expected.extend(quads.iter().cloned());
        check(&dataset.clone(), &expected, &quads);
        for quad in &quads {
            assert!(dataset.remove(quad));
            expected.remove(quad);
            check(&dataset, &expected, &queries);
        }
        assert!(dataset.dictionary.by_hash.is_empty());
        assert!(dataset.dictionary.entries.iter().all(Option::is_none));
        assert!(dataset.indexes.iter().all(BTreeSet::is_empty));
        dataset.extend(quads.iter().cloned());
        check(&dataset, &quads.iter().cloned().collect(), &queries);
        dataset.clear();
        assert!(dataset.dictionary.entries.is_empty());
        assert!(dataset.dictionary.free.is_empty());
        assert!(dataset.dictionary.by_hash.is_empty());
    }

    #[test]
    fn graph_views() {
        let quads = data();
        let mut dataset: Dataset = quads.iter().cloned().collect();
        for graph in [
            GraphName::DefaultGraph,
            quads[1].graph_name.clone(),
            quads[2].graph_name.clone(),
        ] {
            let view = dataset.graph(&graph);
            let reference: Vec<_> = quads
                .iter()
                .filter(|q| q.graph_name == graph)
                .map(|q| Triple::new(q.subject.clone(), q.predicate.clone(), q.object.clone()))
                .collect();
            assert_eq!(view.len(), reference.len());
            assert_eq!(
                view.iter().collect::<HashSet<_>>(),
                reference.iter().cloned().collect()
            );
            for quad in &quads {
                let triple = Triple::new(
                    quad.subject.clone(),
                    quad.predicate.clone(),
                    quad.object.clone(),
                );
                assert_eq!(view.contains(&triple), reference.contains(&triple));
                assert_eq!(
                    view.objects_for_subject_predicate(&quad.subject, &quad.predicate)
                        .collect::<HashSet<_>>(),
                    reference
                        .iter()
                        .filter(|t| t.subject == quad.subject && t.predicate == quad.predicate)
                        .map(|t| t.object.clone())
                        .collect()
                );
                assert_eq!(
                    view.predicates_for_subject_object(&quad.subject, &quad.object)
                        .collect::<HashSet<_>>(),
                    reference
                        .iter()
                        .filter(|t| t.subject == quad.subject && t.object == quad.object)
                        .map(|t| t.predicate.clone())
                        .collect()
                );
                assert_eq!(
                    view.subjects_for_predicate_object(&quad.predicate, &quad.object)
                        .collect::<HashSet<_>>(),
                    reference
                        .iter()
                        .filter(|t| t.predicate == quad.predicate && t.object == quad.object)
                        .map(|t| t.subject.clone())
                        .collect()
                );
            }
        }
        let graph_name = NamedNode::new_unchecked("http://example.com/new");
        let triple = Triple::new(
            BlankNode::new_unchecked("new"),
            graph_name.clone(),
            graph_name.clone(),
        );
        let mut view = dataset.graph_mut(graph_name);
        assert!(view.is_empty());
        assert!(view.insert(triple.clone()));
        assert!(!view.insert(triple.clone()));
        assert_eq!(view.iter().collect::<Vec<_>>(), vec![triple.clone()]);
        assert!(view.remove(&triple));
        assert!(view.is_empty());
    }

    #[test]
    fn term_storage_is_shared_within_each_component() {
        let mut dataset = Dataset::new();
        let subject = "http://example.com/shared";
        for i in 0..10 {
            dataset.insert(Quad::new(
                NamedNode::new_unchecked(subject.to_owned()),
                NamedNode::new_unchecked("http://example.com/p".to_owned()),
                Literal::from("shared value".to_owned()),
                NamedNode::new_unchecked(format!("http://example.com/graph/{i}")),
            ));
        }
        let quads: Vec<_> = dataset.iter().collect();
        for q in &quads {
            assert_eq!(q.subject.to_string(), quads[0].subject.to_string());
            let NamedOrBlankNode::NamedNode(s) = &q.subject else {
                unreachable!()
            };
            let NamedOrBlankNode::NamedNode(first) = &quads[0].subject else {
                unreachable!()
            };
            assert_eq!(s.as_str().as_ptr(), first.as_str().as_ptr());
            assert_eq!(
                q.predicate.as_str().as_ptr(),
                quads[0].predicate.as_str().as_ptr()
            );
            let Term::Literal(o) = &q.object else {
                unreachable!()
            };
            let Term::Literal(first) = &quads[0].object else {
                unreachable!()
            };
            assert_eq!(o.value().as_ptr(), first.value().as_ptr());
        }
    }
    #[cfg(feature = "rdfc-10")]
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
        dataset.insert(Quad::new(
            p.clone(),
            q.clone(),
            e0.clone(),
            GraphName::DefaultGraph,
        ));
        dataset.insert(Quad::new(
            p.clone(),
            q.clone(),
            e1.clone(),
            GraphName::DefaultGraph,
        ));
        dataset.insert(Quad::new(
            e0.clone(),
            p.clone(),
            e2.clone(),
            GraphName::DefaultGraph,
        ));
        dataset.insert(Quad::new(
            e1.clone(),
            p.clone(),
            e3.clone(),
            GraphName::DefaultGraph,
        ));
        dataset.insert(Quad::new(
            e2.clone(),
            r.clone(),
            e3.clone(),
            GraphName::DefaultGraph,
        ));
        dataset.canonicalize(CanonicalizationAlgorithm::Rdfc10 {
            hash_algorithm: CanonicalizationHashAlgorithm::Sha256,
        });

        let mut expected = Dataset::new();
        let c14n0 = BlankNode::new_unchecked("c14n0");
        let c14n1 = BlankNode::new_unchecked("c14n1");
        let c14n2 = BlankNode::new_unchecked("c14n2");
        let c14n3 = BlankNode::new_unchecked("c14n3");
        expected.insert(Quad::new(
            p.clone(),
            q.clone(),
            c14n2.clone(),
            GraphName::DefaultGraph,
        ));
        expected.insert(Quad::new(
            p.clone(),
            q.clone(),
            c14n3.clone(),
            GraphName::DefaultGraph,
        ));
        expected.insert(Quad::new(
            c14n0.clone(),
            r.clone(),
            c14n1.clone(),
            GraphName::DefaultGraph,
        ));
        expected.insert(Quad::new(
            c14n2.clone(),
            p.clone(),
            c14n1.clone(),
            GraphName::DefaultGraph,
        ));
        expected.insert(Quad::new(
            c14n3.clone(),
            p.clone(),
            c14n0.clone(),
            GraphName::DefaultGraph,
        ));
        assert_eq!(dataset, expected);
    }
}
