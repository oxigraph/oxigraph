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
//! let results = dataset.quads_for_subject(&ex).collect::<Vec<_>>();
//! assert_eq!(vec![quad], results);
//!
//! // direct access to a dataset graph
//! let results = dataset.graph(&ex).iter().collect::<Vec<_>>();
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

use crate::{
    BlankNode, BlankNodeRef, GraphName, GraphNameRef, NamedNode, NamedNodeRef, NamedOrBlankNode,
    NamedOrBlankNodeRef, Quad, QuadRef, Term, TermRef, Triple, TripleRef,
};
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
/// let results: Vec<_> = dataset.quads_for_subject(&ex).collect();
/// assert_eq!(vec![quad], results);
///
/// // direct access to a dataset graph
/// let results: Vec<_> = dataset.graph(&ex).iter().collect();
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
    /// let results: Vec<_> = dataset.graph(&ex).iter().collect();
    /// assert_eq!(vec![Triple::new(ex.clone(), ex.clone(), ex)], results);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn graph<'a, 'b>(&'a self, graph_name: impl Into<GraphNameRef<'b>>) -> GraphView<'a> {
        GraphView {
            dataset: self,
            graph_name: self.graph_id(graph_name.into()),
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
    pub fn graph_mut(&mut self, graph_name: impl Into<GraphName>) -> GraphViewMut<'_> {
        GraphViewMut {
            dataset: self,
            graph_name: graph_name.into(),
        }
    }

    /// Returns all the quads contained by the dataset.
    pub fn iter(&self) -> Iter<'_> {
        Iter {
            dataset: self,
            inner: self.indexes[3].iter(),
        }
    }

    pub fn quads_for_subject<'a, 'b, S: Into<NamedOrBlankNodeRef<'b>>>(
        &'a self,
        subject: S,
    ) -> impl Iterator<Item = Quad> + 'a + use<'a, S> {
        self.quads_for_pattern(Some(subject.into()), None, None, None)
    }

    pub fn quads_for_predicate<'a, 'b, P: Into<NamedNodeRef<'b>>>(
        &'a self,
        predicate: P,
    ) -> impl Iterator<Item = Quad> + 'a + use<'a, P> {
        self.quads_for_pattern(None, Some(predicate.into()), None, None)
    }

    pub fn quads_for_object<'a, 'b, O: Into<TermRef<'b>>>(
        &'a self,
        object: O,
    ) -> impl Iterator<Item = Quad> + 'a + use<'a, O> {
        self.quads_for_pattern(None, None, Some(object.into()), None)
    }

    pub fn quads_for_graph_name<'a, 'b, G: Into<GraphNameRef<'b>>>(
        &'a self,
        graph_name: G,
    ) -> impl Iterator<Item = Quad> + 'a + use<'a, G> {
        self.quads_for_pattern(None, None, None, Some(graph_name.into()))
    }

    /// Retrieves quads with a filter on each quad component
    pub fn quads_for_pattern<'a>(
        &'a self,
        subject: Option<NamedOrBlankNodeRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
        graph_name: Option<GraphNameRef<'_>>,
    ) -> impl Iterator<Item = Quad> + use<'a> {
        self.matching_ids(self.encode_pattern(subject, predicate, object, graph_name))
            .map(|ids| self.decode(ids))
    }

    fn matching_ids(&self, ids: Option<[Option<TermId>; 4]>) -> Matches<'_> {
        let mut result = Matches {
            order: 3,
            inner: None,
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
        result.inner = Some(self.indexes[order].range(prefix_range(&prefix[..bound])));
        result
    }

    /// Checks if the dataset contains the given quad
    pub fn contains<'a>(&self, quad: impl Into<QuadRef<'a>>) -> bool {
        self.encoded_quad(quad.into())
            .is_some_and(|q| self.indexes[3].contains(&q))
    }

    /// Returns the number of quads in this dataset.
    pub fn len(&self) -> usize {
        self.indexes[3].len()
    }

    /// Checks if this dataset contains a quad.
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
    pub fn remove<'a>(&mut self, quad: impl Into<QuadRef<'a>>) -> bool {
        let Some(ids) = self.encoded_quad(quad.into()) else {
            return false;
        };
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

    fn encode_pattern<'a>(
        &self,
        subject: Option<NamedOrBlankNodeRef<'a>>,
        predicate: Option<NamedNodeRef<'a>>,
        object: Option<TermRef<'a>>,
        graph_name: Option<GraphNameRef<'a>>,
    ) -> Option<[Option<TermId>; 4]> {
        let find = |term| self.dictionary.find(term).map(Some);
        Some([
            subject.map_or(Some(None), |s| find(s.into()))?,
            predicate.map_or(Some(None), |p| find(p.into()))?,
            object.map_or(Some(None), find)?,
            graph_name.map_or(Some(None), |g| match g {
                GraphNameRef::DefaultGraph => Some(Some(TermId::MIN)),
                GraphNameRef::NamedNode(n) => find(n.into()),
                GraphNameRef::BlankNode(n) => find(n.into()),
            })?,
        ])
    }

    fn encoded_quad(&self, quad: QuadRef<'_>) -> Option<[TermId; 4]> {
        Some([
            self.dictionary.find(quad.subject.into())?,
            self.dictionary.find(quad.predicate.into())?,
            self.dictionary.find(quad.object)?,
            match quad.graph_name {
                GraphNameRef::DefaultGraph => TermId::MIN,
                GraphNameRef::NamedNode(n) => self.dictionary.find(n.into())?,
                GraphNameRef::BlankNode(n) => self.dictionary.find(n.into())?,
            },
        ])
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

    fn graph_id(&self, graph: GraphNameRef<'_>) -> Option<TermId> {
        match graph {
            GraphNameRef::DefaultGraph => Some(TermId::MIN),
            GraphNameRef::NamedNode(n) => self.dictionary.find(n.into()),
            GraphNameRef::BlankNode(n) => self.dictionary.find(n.into()),
        }
    }

    fn decode(&self, ids: [TermId; 4]) -> Quad {
        Quad {
            subject: self.subject(ids[0]),
            predicate: self.predicate(ids[1]),
            object: self.dictionary.term(ids[2]).clone(),
            graph_name: if ids[3] == TermId::MIN {
                GraphName::DefaultGraph
            } else {
                self.subject(ids[3]).into()
            },
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
            quad.subject = map_subject(quad.subject, &mapping);
            quad.object = map_term(quad.object, &mapping);
            if let GraphName::BlankNode(node) = &quad.graph_name {
                quad.graph_name = mapping[node].clone().into();
            }
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

    fn hash(&self, term: TermRef<'_>) -> u64 {
        self.by_hash.hasher().hash_one(term)
    }

    fn find(&self, term: TermRef<'_>) -> Option<TermId> {
        self.by_hash
            .get(&self.hash(term))?
            .iter()
            .copied()
            .find(|id| self.term(*id).as_ref() == term)
    }

    fn acquire(&mut self, term: Term) -> TermId {
        #[expect(clippy::expect_used)]
        if let Some(id) = self.find(term.as_ref()) {
            let entry = self.entries[(id.0 - 1) as usize].as_mut().unwrap();
            entry.references = entry
                .references
                .checked_add(1)
                .expect("term reference count overflow");
            return id;
        }
        let hash = self.hash(term.as_ref());
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
        let hash = self.hash(entry.term.as_ref());
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
    Canonicalization {
        quads: quads
            .iter()
            .map(|q| {
                (
                    q.subject.as_ref(),
                    q.predicate.as_ref(),
                    q.object.as_ref(),
                    q.graph_name.as_ref(),
                )
            })
            .collect(),
    }
    .canonicalize_blank_nodes(algorithm)
    .into_iter()
    .map(|(from, to)| (from.into_owned(), to))
    .collect()
}

struct Canonicalization<'a> {
    quads: Vec<QuadTerms<'a>>,
}

impl<'a> Canonicalization<'a> {
    fn canonicalize_blank_nodes(
        &self,
        algorithm: CanonicalizationAlgorithm,
    ) -> HashMap<BlankNodeRef<'a>, BlankNode> {
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
        for quad in &self.quads {
            if let NamedOrBlankNodeRef::BlankNode(bnode) = quad.0 {
                Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                    bnode,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
            if let TermRef::BlankNode(bnode) = &quad.2 {
                Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                    *bnode,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
            #[cfg(feature = "rdf-12")]
            if let TermRef::Triple(t) = &quad.2 {
                Self::add_quad_to_blank_node_to_quads_map_based_on_triple(
                    t,
                    quad,
                    &mut canonicalization_state.blank_node_to_quads_map,
                );
            }
            if let GraphNameRef::BlankNode(bnode) = &quad.3 {
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
            let hash = Self::hash_first_degree_quads(&canonicalization_state, *n, hash_algorithm);
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
        quad: &QuadTerms<'a>,
        blank_node_to_quads_map: &mut QuadsPerBlankNode<'a>,
    ) {
        if let NamedOrBlankNodeRef::BlankNode(bnode) = triple.subject.as_ref() {
            Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                bnode,
                quad,
                blank_node_to_quads_map,
            );
        }
        if let TermRef::BlankNode(bnode) = &triple.object.as_ref() {
            Self::add_quad_to_blank_node_to_quads_map_for_blank_node(
                *bnode,
                quad,
                blank_node_to_quads_map,
            );
        } else if let TermRef::Triple(t) = &triple.object.as_ref() {
            Self::add_quad_to_blank_node_to_quads_map_based_on_triple(
                t,
                quad,
                blank_node_to_quads_map,
            );
        }
    }

    fn add_quad_to_blank_node_to_quads_map_for_blank_node(
        bnode: BlankNodeRef<'a>,
        quad: &QuadTerms<'a>,
        blank_node_to_quads_map: &mut QuadsPerBlankNode<'a>,
    ) {
        let entry = blank_node_to_quads_map.entry(bnode).or_default();
        if !entry.ends_with(&[*quad]) {
            entry.push(*quad);
        }
    }

    /// RDFC [Issue Identifier Algorithm](https://www.w3.org/TR/rdf-canon/#issue-identifier)
    fn issue_identifier(
        issuer: &mut IdentifierIssuer<'a>,
        blank_node: BlankNodeRef<'a>,
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
        reference_blank_node_identifier: BlankNodeRef<'a>,
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
            let subject = Self::hash_first_degree_quads_decode_named_or_blank_node(
                subject,
                reference_blank_node_identifier,
            );
            let predicate = predicate.into_owned();
            let object =
                Self::hash_first_degree_quads_decode_term(object, reference_blank_node_identifier);
            let graph_name = Self::hash_first_degree_quads_decode_graph_name(
                graph_name,
                reference_blank_node_identifier,
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
        term: &NamedOrBlankNodeRef<'a>,
        reference_blank_node_identifier: BlankNodeRef<'a>,
    ) -> NamedOrBlankNode {
        match term {
            NamedOrBlankNodeRef::NamedNode(t) => t.into_owned().into(),
            NamedOrBlankNodeRef::BlankNode(t) => {
                BlankNodeRef::new_unchecked(if *t == reference_blank_node_identifier {
                    "a"
                } else {
                    "z"
                })
                .into()
            }
        }
    }

    fn hash_first_degree_quads_decode_term(
        term: &TermRef<'a>,
        reference_blank_node_identifier: BlankNodeRef<'a>,
    ) -> Term {
        match term {
            TermRef::NamedNode(t) => t.into_owned().into(),
            TermRef::BlankNode(t) => {
                BlankNodeRef::new_unchecked(if *t == reference_blank_node_identifier {
                    "a"
                } else {
                    "z"
                })
                .into()
            }
            TermRef::Literal(t) => t.into_owned().into(),
            #[cfg(feature = "rdf-12")]
            TermRef::Triple(t) => Triple::new(
                Self::hash_first_degree_quads_decode_named_or_blank_node(
                    &t.subject.as_ref(),
                    reference_blank_node_identifier,
                ),
                t.predicate.clone(),
                Self::hash_first_degree_quads_decode_term(
                    &t.object.as_ref(),
                    reference_blank_node_identifier,
                ),
            )
            .into(),
        }
    }

    fn hash_first_degree_quads_decode_graph_name(
        term: &GraphNameRef<'a>,
        reference_blank_node_identifier: BlankNodeRef<'a>,
    ) -> GraphName {
        match term {
            GraphNameRef::NamedNode(t) => t.into_owned().into(),
            GraphNameRef::BlankNode(t) => {
                BlankNodeRef::new_unchecked(if *t == reference_blank_node_identifier {
                    "a"
                } else {
                    "z"
                })
                .into()
            }
            GraphNameRef::DefaultGraph => GraphName::DefaultGraph,
        }
    }

    /// RDFC [Hash Related Blank Node](https://www.w3.org/TR/rdf-canon/#hash-related-blank-node)
    fn hash_related_blank_node(
        canonicalization_state: &CanonicalizationState<'a>,
        related: BlankNodeRef<'a>,
        quad: &QuadTerms<'a>,
        issuer: &IdentifierIssuer<'a>,
        position: &str,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) -> String {
        // 1)
        let mut input = position.to_owned();
        // 2)
        if position != "g" {
            input.push('<');
            input.push_str(quad.1.into_owned().as_str());
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
        identifier: BlankNodeRef<'a>,
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
            if let NamedOrBlankNodeRef::BlankNode(component) = quad.0 {
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
            if let TermRef::BlankNode(component) = quad.2 {
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
            if let TermRef::Triple(t) = &quad.2 {
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
            if let GraphNameRef::BlankNode(component) = quad.3 {
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
        identifier: BlankNodeRef<'a>,
        quad: &QuadTerms<'a>,
        issuer: &IdentifierIssuer<'a>,
        h_n: &mut BTreeMap<String, HashSet<BlankNodeRef<'a>>>,
        hash_algorithm: Option<CanonicalizationHashAlgorithm>,
    ) {
        if let NamedOrBlankNodeRef::BlankNode(component) = triple.subject.as_ref() {
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
        if let TermRef::BlankNode(component) = &triple.object.as_ref() {
            Self::hash_related_blank_node_on_possible_component(
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
        canonicalization_state: &CanonicalizationState<'a>,
        component: BlankNodeRef<'a>,
        identifier: BlankNodeRef<'a>,
        quad: &QuadTerms<'a>,
        issuer: &IdentifierIssuer<'a>,
        position: &str,
        h_n: &mut BTreeMap<String, HashSet<BlankNodeRef<'a>>>,
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

fn map_subject(
    subject: NamedOrBlankNode,
    mapping: &HashMap<BlankNode, BlankNode>,
) -> NamedOrBlankNode {
    match subject {
        NamedOrBlankNode::BlankNode(node) => mapping[&node].clone().into(),
        node @ NamedOrBlankNode::NamedNode(_) => node,
    }
}

fn map_term(term: Term, mapping: &HashMap<BlankNode, BlankNode>) -> Term {
    match term {
        Term::BlankNode(node) => mapping[&node].clone().into(),
        #[cfg(feature = "rdf-12")]
        Term::Triple(mut triple) => {
            triple.subject = map_subject(triple.subject, mapping);
            triple.object = map_term(triple.object, mapping);
            Term::Triple(triple)
        }
        other => other,
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
/// let results: Vec<_> = dataset.graph(&ex).iter().collect();
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
    pub fn iter(&self) -> GraphViewIter<'a> {
        GraphViewIter {
            dataset: self.dataset,
            inner: self.matching(None, None, None),
        }
    }

    pub fn triples_for_subject<'b, S: Into<NamedOrBlankNodeRef<'b>>>(
        &self,
        subject: S,
    ) -> impl Iterator<Item = Triple> + 'a + use<'a, S> {
        self.triples_for_pattern(Some(subject.into()), None, None)
    }

    pub fn objects_for_subject_predicate<
        'b,
        S: Into<NamedOrBlankNodeRef<'b>>,
        P: Into<NamedNodeRef<'b>>,
    >(
        &self,
        subject: S,
        predicate: P,
    ) -> impl Iterator<Item = Term> + 'a + use<'a, S, P> {
        let dataset = self.dataset;
        self.matching(Some(subject.into()), Some(predicate.into()), None)
            .map(move |ids| dataset.dictionary.term(ids[2]).clone())
    }

    pub fn object_for_subject_predicate<'b>(
        &self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> Option<Term> {
        self.objects_for_subject_predicate(subject, predicate)
            .next()
    }

    pub fn predicates_for_subject_object<
        'b,
        S: Into<NamedOrBlankNodeRef<'b>>,
        O: Into<TermRef<'b>>,
    >(
        &self,
        subject: S,
        object: O,
    ) -> impl Iterator<Item = NamedNode> + 'a + use<'a, S, O> {
        let dataset = self.dataset;
        self.matching(Some(subject.into()), None, Some(object.into()))
            .map(move |ids| dataset.predicate(ids[1]))
    }

    pub fn triples_for_predicate<'b, P: Into<NamedNodeRef<'b>>>(
        &self,
        predicate: P,
    ) -> impl Iterator<Item = Triple> + 'a + use<'a, P> {
        self.triples_for_pattern(None, Some(predicate.into()), None)
    }

    pub fn subjects_for_predicate_object<'b, P: Into<NamedNodeRef<'b>>, O: Into<TermRef<'b>>>(
        &self,
        predicate: P,
        object: O,
    ) -> impl Iterator<Item = NamedOrBlankNode> + 'a + use<'a, P, O> {
        let dataset = self.dataset;
        self.matching(None, Some(predicate.into()), Some(object.into()))
            .map(move |ids| dataset.subject(ids[0]))
    }

    pub fn subject_for_predicate_object<'b>(
        &self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> Option<NamedOrBlankNode> {
        self.subjects_for_predicate_object(predicate, object).next()
    }

    pub fn triples_for_object<'b, O: Into<TermRef<'b>>>(
        &self,
        object: O,
    ) -> impl Iterator<Item = Triple> + 'a + use<'a, O> {
        self.triples_for_pattern(None, None, Some(object.into()))
    }

    /// Retrieves triples with a filter on each triple component
    pub fn triples_for_pattern(
        &self,
        subject: Option<NamedOrBlankNodeRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
    ) -> impl Iterator<Item = Triple> + use<'a> {
        let dataset = self.dataset;
        self.matching(subject, predicate, object)
            .map(move |ids| Triple {
                subject: dataset.subject(ids[0]),
                predicate: dataset.predicate(ids[1]),
                object: dataset.dictionary.term(ids[2]).clone(),
            })
    }

    /// Checks if the graph contains the given triple.
    pub fn contains<'b>(&self, triple: impl Into<TripleRef<'b>>) -> bool {
        let triple = triple.into();
        self.matching(
            Some(triple.subject),
            Some(triple.predicate),
            Some(triple.object),
        )
        .next()
        .is_some()
    }

    /// Returns the number of triples in this graph.
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Checks if this graph contains a triple.
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    fn matching(
        &self,
        subject: Option<NamedOrBlankNodeRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
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

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &GraphView<'a> {
    type Item = Triple;
    type IntoIter = GraphViewIter<'a>;

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
        let triple = triple.into();
        self.dataset.insert(Quad {
            subject: triple.subject,
            predicate: triple.predicate,
            object: triple.object,
            graph_name: self.graph_name.clone(),
        })
    }

    /// Removes a concrete triple from the graph.
    pub fn remove<'a>(&mut self, triple: impl Into<TripleRef<'a>>) -> bool {
        let triple = triple.into();
        self.dataset.remove(QuadRef {
            subject: triple.subject,
            predicate: triple.predicate,
            object: triple.object,
            graph_name: self.graph_name.as_ref(),
        })
    }

    /// Returns all the triples contained by the graph
    pub fn iter(&self) -> GraphViewIter<'_> {
        self.read().iter()
    }

    pub fn triples_for_subject<'a, 'b, S: Into<NamedOrBlankNodeRef<'b>>>(
        &'a self,
        subject: S,
    ) -> impl Iterator<Item = Triple> + 'a + use<'a, S> {
        self.read().triples_for_subject(subject)
    }

    pub fn objects_for_subject_predicate<
        'a,
        'b,
        S: Into<NamedOrBlankNodeRef<'b>>,
        P: Into<NamedNodeRef<'b>>,
    >(
        &'a self,
        subject: S,
        predicate: P,
    ) -> impl Iterator<Item = Term> + 'a + use<'a, S, P> {
        self.read()
            .objects_for_subject_predicate(subject, predicate)
    }

    pub fn object_for_subject_predicate<'b>(
        &self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> Option<Term> {
        self.read().object_for_subject_predicate(subject, predicate)
    }

    pub fn predicates_for_subject_object<
        'a,
        'b,
        S: Into<NamedOrBlankNodeRef<'b>>,
        O: Into<TermRef<'b>>,
    >(
        &'a self,
        subject: S,
        object: O,
    ) -> impl Iterator<Item = NamedNode> + 'a + use<'a, S, O> {
        self.read().predicates_for_subject_object(subject, object)
    }

    pub fn triples_for_predicate<'a, 'b, P: Into<NamedNodeRef<'b>>>(
        &'a self,
        predicate: P,
    ) -> impl Iterator<Item = Triple> + 'a + use<'a, P> {
        self.read().triples_for_predicate(predicate)
    }

    pub fn subjects_for_predicate_object<
        'a,
        'b,
        P: Into<NamedNodeRef<'b>>,
        O: Into<TermRef<'b>>,
    >(
        &'a self,
        predicate: P,
        object: O,
    ) -> impl Iterator<Item = NamedOrBlankNode> + 'a + use<'a, P, O> {
        self.read().subjects_for_predicate_object(predicate, object)
    }

    pub fn subject_for_predicate_object<'b>(
        &self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> Option<NamedOrBlankNode> {
        self.read().subject_for_predicate_object(predicate, object)
    }

    pub fn triples_for_object<'a, 'b, O: Into<TermRef<'b>>>(
        &'a self,
        object: O,
    ) -> impl Iterator<Item = Triple> + 'a + use<'a, O> {
        self.read().triples_for_object(object)
    }

    pub fn triples_for_pattern<'a>(
        &'a self,
        subject: Option<NamedOrBlankNodeRef<'_>>,
        predicate: Option<NamedNodeRef<'_>>,
        object: Option<TermRef<'_>>,
    ) -> impl Iterator<Item = Triple> + use<'a> {
        self.read().triples_for_pattern(subject, predicate, object)
    }

    /// Checks if the graph contains the given triple.
    pub fn contains<'a>(&self, triple: impl Into<TripleRef<'a>>) -> bool {
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

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

struct Matches<'a> {
    order: usize,
    inner: Option<btree_set::Range<'a, [TermId; 4]>>,
}

impl Iterator for Matches<'_> {
    type Item = [TermId; 4];
    fn next(&mut self) -> Option<Self::Item> {
        let indexed = self.inner.as_mut()?.next()?;
        let mut ids = [TermId::MIN; 4];
        for (position, component) in ORDERS[self.order].iter().enumerate() {
            ids[*component] = indexed[position];
        }
        Some(ids)
    }
}

/// Iterator returned by [`Dataset::iter`].
pub struct Iter<'a> {
    dataset: &'a Dataset,
    inner: btree_set::Iter<'a, [TermId; 4]>,
}
impl Iterator for Iter<'_> {
    type Item = Quad;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|ids| self.dataset.decode(*ids))
    }
}

/// Iterator returned by [`GraphView::iter`].
pub struct GraphViewIter<'a> {
    dataset: &'a Dataset,
    inner: Matches<'a>,
}

impl Iterator for GraphViewIter<'_> {
    type Item = Triple;
    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|ids| Triple {
            subject: self.dataset.subject(ids[0]),
            predicate: self.dataset.predicate(ids[1]),
            object: self.dataset.dictionary.term(ids[2]).clone(),
        })
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

type QuadTerms<'a> = (
    NamedOrBlankNodeRef<'a>,
    NamedNodeRef<'a>,
    TermRef<'a>,
    GraphNameRef<'a>,
);
type QuadsPerBlankNode<'a> = HashMap<BlankNodeRef<'a>, Vec<QuadTerms<'a>>>;

/// A RDFC [canonicalization state](https://www.w3.org/TR/rdf-canon/#canon-state)
struct CanonicalizationState<'a> {
    blank_node_to_quads_map: QuadsPerBlankNode<'a>,
    hash_to_blank_nodes_map: BTreeMap<String, Vec<BlankNodeRef<'a>>>,
    canonical_issuer: IdentifierIssuer<'a>,
}

/// A RDFC [identifier issuer](https://www.w3.org/TR/rdf-canon/#dfn-identifier-issuer)
#[derive(Clone)]
struct IdentifierIssuer<'a> {
    identifier_prefix: &'static str,
    identifier_counter: u32,
    issued_identifier_map: HashMap<BlankNodeRef<'a>, BlankNode>,
    issued_identifier_order: Vec<BlankNodeRef<'a>>, /* hack to know the insertion order in the hash map */
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
            Literal::new_simple_literal("a").into(),
            Literal::new_language_tagged_literal_unchecked("a", "en").into(),
            Literal::new_typed_literal("a", NamedNode::new_unchecked("http://example.com/type"))
                .into(),
        ];
        #[cfg(feature = "rdf-12")]
        let objects = {
            let mut rdf12_objects = objects;
            rdf12_objects.push(
                Literal::new_directional_language_tagged_literal_unchecked(
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
                assert_eq!(dataset.dictionary.find(entry.term.as_ref()), Some(*id));
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
                    (mask & 1 != 0).then_some(query.subject.as_ref()),
                    (mask & 2 != 0).then_some(query.predicate.as_ref()),
                    (mask & 4 != 0).then_some(query.object.as_ref()),
                    (mask & 8 != 0).then_some(query.graph_name.as_ref()),
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
            Literal::new_simple_literal("missing"),
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
                    dataset.insert(query.as_ref()),
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
                Literal::new_simple_literal("shared value".to_owned()),
                NamedNode::new_unchecked(format!("http://example.com/graph/{i}")),
            ));
        }
        let quads: Vec<_> = dataset.iter().collect();
        for q in &quads {
            assert_eq!(
                q.subject.as_ref().to_string(),
                quads[0].subject.as_ref().to_string()
            );
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
