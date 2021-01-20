//! [In-memory implementation](super::Dataset) of [RDF datasets](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
//!
//! Usage example:
//! ```
//! use oxigraph::model::*;
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

use crate::io::{
    DatasetFormat, DatasetParser, DatasetSerializer, GraphFormat, GraphParser, GraphSerializer,
};
use crate::model::interning::*;
use crate::model::NamedOrBlankNodeRef;
use crate::model::*;
use lasso::Rodeo;
use std::collections::hash_map::DefaultHasher;
use std::collections::BTreeSet;
use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::io::{BufRead, Write};
use std::iter::FromIterator;
use std::{fmt, io};

/// An in-memory [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset).
///
/// It can accomodate a fairly large number of quads (in the few millions).
/// Beware: it interns the string and does not do any garbage collection yet:
/// if you insert and remove a lot of different terms, memory will grow without any reduction.
///
/// Usage example:
/// ```
/// use oxigraph::model::*;
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
    interner: Rodeo,
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

    /// Provides a read-only view on a [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) contained in this dataset.
    ///
    /// ```
    /// use oxigraph::model::*;
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

    /// Provides a read/write view on a [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) contained in this dataset.
    ///
    /// ```
    /// use oxigraph::model::*;
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

    /// Returns all the quads contained by the dataset
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
        self.interned_quads_for_subject(subject)
            .map(move |q| self.decode_spog(q))
    }

    fn interned_quads_for_subject(
        &self,
        subject: InternedNamedOrBlankNode,
    ) -> impl Iterator<
        Item = (
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
    > + '_ {
        self.spog
            .range(
                &(
                    subject,
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
            .copied()
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
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
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
            .copied()
            .map(|(p, o, s, g)| (s, p, o, g))
    }

    pub fn quads_for_object<'a, 'b>(
        &'a self,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = QuadRef<'a>> + 'a {
        let object = self
            .encoded_term(object)
            .unwrap_or_else(InternedTerm::impossible);

        self.interned_quads_for_object(object)
            .map(move |q| self.decode_spog(q))
    }

    fn interned_quads_for_object(
        &self,
        object: InternedTerm,
    ) -> impl Iterator<
        Item = (
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
    > + '_ {
        self.ospg
            .range(
                &(
                    object,
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
            .copied()
            .map(|(o, s, p, g)| (s, p, o, g))
    }

    fn interned_quads_for_graph_name(
        &self,
        graph_name: InternedGraphName,
    ) -> impl Iterator<
        Item = (
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
    > + '_ {
        self.gspo
            .range(
                &(
                    graph_name,
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
            .copied()
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

    /// Returns the number of quads in this dataset
    pub fn len(&self) -> usize {
        self.gspo.len()
    }

    /// Checks if this dataset contains a quad
    pub fn is_empty(&self) -> bool {
        self.gspo.is_empty()
    }

    /// Adds a quad to the dataset
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
        self.gspo.insert((g, s, p, o));
        self.gpos.insert((g, p, o, s));
        self.gosp.insert((g, o, s, p));
        self.spog.insert((s, p, o, g));
        self.posg.insert((p, o, s, g));
        self.ospg.insert((o, s, p, g))
    }

    /// Removes a concrete quad from the dataset
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
        self.gspo.remove(&(g, s, p, o));
        self.gpos.remove(&(g, p, o, s));
        self.gosp.remove(&(g, o, s, p));
        self.spog.remove(&(s, p, o, g));
        self.posg.remove(&(p, o, s, g));
        self.ospg.remove(&(o, s, p, g))
    }

    /// Clears the dataset
    pub fn clear(&mut self) {
        self.gspo.clear();
        self.gpos.clear();
        self.gosp.clear();
        self.spog.clear();
        self.posg.clear();
        self.ospg.clear();
    }

    /// Loads a file into the dataset.
    ///
    /// To load a specific graph use [`GraphViewMut::load`].
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::io::DatasetFormat;
    ///
    /// let mut dataset = Dataset::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com>  <http://example.com> .";
    /// dataset.load(file.as_ref(), DatasetFormat::NQuads, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(dataset.contains(QuadRef::new(ex, ex, ex, ex)));
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// Warning: This functions inserts the quads during the parsing.
    /// If the parsing fails in the middle of the file, the quads read before stay in the dataset.
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    pub fn load(
        &mut self,
        reader: impl BufRead,
        format: DatasetFormat,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        let mut parser = DatasetParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        }
        for t in parser.read_quads(reader)? {
            self.insert(&t?);
        }
        Ok(())
    }

    /// Dumps the dataset into a file.
    ///
    /// To dump a specific graph use [`GraphView::dump`].
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::DatasetFormat;
    /// use oxigraph::model::Dataset;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let mut store = Dataset::new();
    /// store.load(file, DatasetFormat::NQuads,None)?;
    ///
    /// let mut buffer = Vec::new();
    /// store.dump(&mut buffer, DatasetFormat::NQuads)?;
    /// assert_eq!(file, buffer.as_slice());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn dump(&self, writer: impl Write, format: DatasetFormat) -> Result<(), io::Error> {
        let mut writer = DatasetSerializer::from_format(format).quad_writer(writer)?;
        for t in self {
            writer.write(t)?;
        }
        writer.finish()
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
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
            InternedGraphName,
        ),
    ) -> QuadRef<'_> {
        QuadRef {
            subject: quad.0.decode_from(&self.interner),
            predicate: quad.1.decode_from(&self.interner),
            object: quad.2.decode_from(&self.interner),
            graph_name: quad.3.decode_from(&self.interner),
        }
    }

    /// Applies on the dataset the canonicalization process described in
    /// [Canonical Forms for Isomorphic and Equivalent RDF Graphs: Algorithms for Leaning and Labelling Blank Nodes, Aidan Hogan, 2017](http://aidanhogan.com/docs/rdf-canonicalisation.pdf)
    ///
    /// Warning: This implementation worst-case complexity is in O(b!) with b the number of blank nodes in the input graphs.
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
            if let InternedNamedOrBlankNode::BlankNode(bnode) = s {
                bnodes.insert(*bnode);
            }
            if let InternedTerm::BlankNode(bnode) = o {
                bnodes.insert(*bnode);
            }
            if let InternedGraphName::BlankNode(bnode) = g {
                bnodes.insert(*bnode);
            }
        }
        bnodes
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
                    self.interned_quads_for_subject(InternedNamedOrBlankNode::BlankNode(*bnode))
                {
                    to_hash.push((
                        self.hash_named_node(p),
                        self.hash_term(o, &hashes),
                        self.hash_graph_name(g, &hashes),
                        0,
                    ));
                }
                for (s, p, _, g) in self.interned_quads_for_object(InternedTerm::BlankNode(*bnode))
                {
                    to_hash.push((
                        self.hash_named_or_blank_node(s, &hashes),
                        self.hash_named_node(p),
                        self.hash_graph_name(g, &hashes),
                        1,
                    ));
                }
                for (s, p, o, _) in
                    self.interned_quads_for_graph_name(InternedGraphName::BlankNode(*bnode))
                {
                    to_hash.push((
                        self.hash_named_or_blank_node(s, &hashes),
                        self.hash_named_node(p),
                        self.hash_term(o, &hashes),
                        2,
                    ));
                }
                to_hash.sort_unstable();
                let hash = self.hash_tuple((old_hash, &to_hash));
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
        self.hash_tuple(node.decode_from(&self.interner))
    }

    fn hash_named_or_blank_node(
        &self,
        node: InternedNamedOrBlankNode,
        bnodes_hash: &HashMap<InternedBlankNode, u64>,
    ) -> u64 {
        if let InternedNamedOrBlankNode::BlankNode(bnode) = node {
            *bnodes_hash.get(&bnode).unwrap()
        } else {
            self.hash_tuple(node.decode_from(&self.interner))
        }
    }

    fn hash_term(&self, term: InternedTerm, bnodes_hash: &HashMap<InternedBlankNode, u64>) -> u64 {
        if let InternedTerm::BlankNode(bnode) = term {
            *bnodes_hash.get(&bnode).unwrap()
        } else {
            self.hash_tuple(term.decode_from(&self.interner))
        }
    }

    fn hash_graph_name(
        &self,
        graph_name: InternedGraphName,
        bnodes_hash: &HashMap<InternedBlankNode, u64>,
    ) -> u64 {
        if let InternedGraphName::BlankNode(bnode) = graph_name {
            *bnodes_hash.get(&bnode).unwrap()
        } else {
            self.hash_tuple(graph_name.decode_from(&self.interner))
        }
    }

    fn hash_tuple(&self, v: impl Hash) -> u64 {
        let mut hasher = DefaultHasher::new();
        v.hash(&mut hasher);
        hasher.finish()
    }

    fn distinguish(
        &mut self,
        hash: &HashMap<InternedBlankNode, u64>,
        partition: &[(u64, Vec<InternedBlankNode>)],
    ) -> Vec<(
        InternedNamedOrBlankNode,
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
                    hash_prime.insert(*b, self.hash_tuple((hash_prime[b], 22)));
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
                .unwrap_or_else(Vec::new)
        } else {
            self.label(hash)
        }
    }

    fn label(
        &mut self,
        hashes: &HashMap<InternedBlankNode, u64>,
    ) -> Vec<(
        InternedNamedOrBlankNode,
        InternedNamedNode,
        InternedTerm,
        InternedGraphName,
    )> {
        let old_quads: Vec<_> = self.spog.iter().copied().collect();
        let mut quads: Vec<_> = old_quads
            .into_iter()
            .map(|(s, p, o, g)| {
                (
                    if let InternedNamedOrBlankNode::BlankNode(bnode) = s {
                        InternedNamedOrBlankNode::BlankNode(self.map_bnode(bnode, hashes))
                    } else {
                        s
                    },
                    p,
                    if let InternedTerm::BlankNode(bnode) = o {
                        InternedTerm::BlankNode(self.map_bnode(bnode, hashes))
                    } else {
                        o
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

    fn map_bnode(
        &mut self,
        old_bnode: InternedBlankNode,
        hashes: &HashMap<InternedBlankNode, u64>,
    ) -> InternedBlankNode {
        InternedBlankNode::encoded_into(
            BlankNode::new_from_unique_id(*hashes.get(&old_bnode).unwrap()).as_ref(),
            &mut self.interner,
        )
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
        let mut g = Dataset::new();
        g.extend(iter);
        g
    }
}

impl<'a, T: Into<QuadRef<'a>>> FromIterator<T> for Dataset {
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> Self {
        let mut g = Dataset::new();
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
            writeln!(f, "{}", t)?;
        }
        Ok(())
    }
}

/// A read-only view on a [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) contained in a [`Dataset`].
///
/// It is built using the [`Dataset::graph`] method.
///
/// Usage example:
/// ```
/// use oxigraph::model::*;
///
/// let mut dataset = Dataset::default();
/// let ex = NamedNodeRef::new("http://example.com")?;
/// dataset.insert(QuadRef::new(ex, ex, ex, ex));
///
/// let results: Vec<_> = dataset.graph(ex).iter().collect();
/// assert_eq!(vec![TripleRef::new(ex, ex, ex)], results);
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone, Copy, Debug)]
pub struct GraphView<'a> {
    dataset: &'a Dataset,
    graph_name: InternedGraphName,
}

impl<'a> GraphView<'a> {
    /// Returns all the triples contained by the graph
    pub fn iter(self) -> GraphViewIter<'a> {
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
            graph: self,
            inner: iter,
        }
    }

    pub fn triples_for_subject<'b>(
        self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.triples_for_interned_subject(self.dataset.encoded_named_or_blank_node(subject))
    }

    pub(super) fn triples_for_interned_subject(
        self,
        subject: Option<InternedNamedOrBlankNode>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        let subject = subject.unwrap_or_else(InternedNamedOrBlankNode::impossible);
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
            .map(move |q| self.decode_gspo(*q))
    }

    pub fn objects_for_subject_predicate<'b>(
        self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TermRef<'a>> + 'a {
        self.objects_for_interned_subject_predicate(
            self.dataset.encoded_named_or_blank_node(subject),
            self.dataset.encoded_named_node(predicate),
        )
    }

    pub fn objects_for_interned_subject_predicate(
        self,
        subject: Option<InternedNamedOrBlankNode>,
        predicate: Option<InternedNamedNode>,
    ) -> impl Iterator<Item = TermRef<'a>> + 'a {
        let subject = subject.unwrap_or_else(InternedNamedOrBlankNode::impossible);
        let predicate = predicate.unwrap_or_else(InternedNamedNode::impossible);
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
            .map(move |q| q.3.decode_from(&self.dataset.interner))
    }

    pub fn object_for_subject_predicate<'b>(
        self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> Option<TermRef<'a>> {
        self.objects_for_subject_predicate(subject, predicate)
            .next()
    }

    pub fn predicates_for_subject_object<'b>(
        self,
        subject: impl Into<NamedOrBlankNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = NamedNodeRef<'a>> + 'a {
        self.predicates_for_interned_subject_object(
            self.dataset.encoded_named_or_blank_node(subject),
            self.dataset.encoded_term(object),
        )
    }

    pub(super) fn predicates_for_interned_subject_object(
        self,
        subject: Option<InternedNamedOrBlankNode>,
        object: Option<InternedTerm>,
    ) -> impl Iterator<Item = NamedNodeRef<'a>> + 'a {
        let subject = subject.unwrap_or_else(InternedNamedOrBlankNode::impossible);
        let object = object.unwrap_or_else(InternedTerm::impossible);
        self.dataset
            .gosp
            .range(
                &(self.graph_name, object, subject, InternedNamedNode::first())
                    ..&(
                        self.graph_name,
                        object,
                        subject.next(),
                        InternedNamedNode::first(),
                    ),
            )
            .map(move |q| q.3.decode_from(&self.dataset.interner))
    }

    pub fn triples_for_predicate<'b>(
        self,
        predicate: impl Into<NamedNodeRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.triples_for_interned_predicate(self.dataset.encoded_named_node(predicate))
    }

    pub(super) fn triples_for_interned_predicate(
        self,
        predicate: Option<InternedNamedNode>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        let predicate = predicate.unwrap_or_else(InternedNamedNode::impossible);
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
            .map(move |q| self.decode_gpos(*q))
    }

    pub fn subjects_for_predicate_object<'b>(
        self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = NamedOrBlankNodeRef<'a>> + 'a {
        self.subjects_for_interned_predicate_object(
            self.dataset.encoded_named_node(predicate),
            self.dataset.encoded_term(object),
        )
    }

    pub(super) fn subjects_for_interned_predicate_object(
        self,
        predicate: Option<InternedNamedNode>,
        object: Option<InternedTerm>,
    ) -> impl Iterator<Item = NamedOrBlankNodeRef<'a>> + 'a {
        let predicate = predicate.unwrap_or_else(InternedNamedNode::impossible);
        let object = object.unwrap_or_else(InternedTerm::impossible);
        self.dataset
            .gpos
            .range(
                &(
                    self.graph_name,
                    predicate,
                    object,
                    InternedNamedOrBlankNode::first(),
                )
                    ..&(
                        self.graph_name,
                        predicate,
                        object.next(),
                        InternedNamedOrBlankNode::first(),
                    ),
            )
            .map(move |q| q.3.decode_from(&self.dataset.interner))
    }

    pub fn subject_for_predicate_object<'b>(
        self,
        predicate: impl Into<NamedNodeRef<'b>>,
        object: impl Into<TermRef<'b>>,
    ) -> Option<NamedOrBlankNodeRef<'a>> {
        self.subjects_for_predicate_object(predicate, object).next()
    }

    pub fn triples_for_object<'b>(
        self,
        object: impl Into<TermRef<'b>>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        self.triples_for_interned_object(self.dataset.encoded_term(object))
    }

    pub fn triples_for_interned_object(
        self,
        object: Option<InternedTerm>,
    ) -> impl Iterator<Item = TripleRef<'a>> + 'a {
        let object = object.unwrap_or_else(InternedTerm::impossible);
        self.dataset
            .gosp
            .range(
                &(
                    self.graph_name,
                    object,
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
            .map(move |q| self.decode_gosp(*q))
    }

    /// Checks if the graph contains the given triple
    pub fn contains<'b>(&self, triple: impl Into<TripleRef<'b>>) -> bool {
        if let Some((s, p, o)) = self.encoded_triple(triple.into()) {
            self.dataset.gspo.contains(&(self.graph_name, s, p, o))
        } else {
            false
        }
    }

    /// Returns the number of triples in this graph
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Checks if this graph contains a triple
    pub fn is_empty(&self) -> bool {
        self.iter().next().is_none()
    }

    /// Dumps the graph into a file.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::Graph;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let mut store = Graph::new();
    /// store.load(file, GraphFormat::NTriples,None)?;
    ///
    /// let mut buffer = Vec::new();
    /// store.dump(&mut buffer, GraphFormat::NTriples)?;
    /// assert_eq!(file, buffer.as_slice());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn dump(self, writer: impl Write, format: GraphFormat) -> Result<(), io::Error> {
        let mut writer = GraphSerializer::from_format(format).triple_writer(writer)?;
        for t in self {
            writer.write(t)?;
        }
        writer.finish()
    }

    fn encoded_triple(
        &self,
        triple: TripleRef<'_>,
    ) -> Option<(InternedNamedOrBlankNode, InternedNamedNode, InternedTerm)> {
        Some((
            self.dataset.encoded_named_or_blank_node(triple.subject)?,
            self.dataset.encoded_named_node(triple.predicate)?,
            self.dataset.encoded_term(triple.object)?,
        ))
    }

    fn decode_gspo(
        self,
        quad: (
            InternedGraphName,
            InternedNamedOrBlankNode,
            InternedNamedNode,
            InternedTerm,
        ),
    ) -> TripleRef<'a> {
        TripleRef {
            subject: quad.1.decode_from(&self.dataset.interner),
            predicate: quad.2.decode_from(&self.dataset.interner),
            object: quad.3.decode_from(&self.dataset.interner),
        }
    }

    fn decode_gpos(
        self,
        quad: (
            InternedGraphName,
            InternedNamedNode,
            InternedTerm,
            InternedNamedOrBlankNode,
        ),
    ) -> TripleRef<'a> {
        self.decode_gspo((quad.0, quad.3, quad.1, quad.2))
    }

    fn decode_gosp(
        self,
        quad: (
            InternedGraphName,
            InternedTerm,
            InternedNamedOrBlankNode,
            InternedNamedNode,
        ),
    ) -> TripleRef<'a> {
        self.decode_gspo((quad.0, quad.2, quad.3, quad.1))
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
            writeln!(f, "{}", t)?;
        }
        Ok(())
    }
}

/// A read/write view on a [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) contained in a [`Dataset`].
///
/// It is built using the [`Dataset::graph_mut`] method.
///
/// Usage example:
/// ```
/// use oxigraph::model::*;
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
            graph_name: self.graph_name,
        }
    }

    /// Adds a triple to the graph
    pub fn insert<'b>(&mut self, triple: impl Into<TripleRef<'b>>) -> bool {
        let (s, p, o) = self.encode_triple(triple.into());
        self.dataset.insert_encoded((s, p, o, self.graph_name))
    }

    /// Removes a concrete triple from the graph
    pub fn remove<'b>(&mut self, triple: impl Into<TripleRef<'b>>) -> bool {
        if let Some((s, p, o)) = self.read().encoded_triple(triple.into()) {
            self.dataset.remove_encoded((s, p, o, self.graph_name))
        } else {
            false
        }
    }

    /// Loads a file into the graph.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::io::GraphFormat;
    ///
    /// let mut graph = Graph::new();
    ///
    /// // insertion
    /// let file = b"<http://example.com> <http://example.com> <http://example.com> .";
    /// graph.load(file.as_ref(), GraphFormat::NTriples, None)?;
    ///
    /// // we inspect the store contents
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// assert!(graph.contains(TripleRef::new(ex, ex, ex)));
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    ///
    /// Warning: This functions inserts the triples during the parsing.
    /// If the parsing fails in the middle of the file, the triples read before stay in the graph.
    ///
    /// Errors related to parameter validation like the base IRI use the [`InvalidInput`](std::io::ErrorKind::InvalidInput) error kind.
    /// Errors related to a bad syntax in the loaded file use the [`InvalidData`](std::io::ErrorKind::InvalidData) or [`UnexpectedEof`](std::io::ErrorKind::UnexpectedEof) error kinds.
    pub fn load(
        &mut self,
        reader: impl BufRead,
        format: GraphFormat,
        base_iri: Option<&str>,
    ) -> Result<(), io::Error> {
        let mut parser = GraphParser::from_format(format);
        if let Some(base_iri) = base_iri {
            parser = parser
                .with_base_iri(base_iri)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        }
        for t in parser.read_triples(reader)? {
            self.insert(&t?);
        }
        Ok(())
    }

    fn encode_triple(
        &mut self,
        triple: TripleRef<'_>,
    ) -> (InternedNamedOrBlankNode, InternedNamedNode, InternedTerm) {
        (
            InternedNamedOrBlankNode::encoded_into(triple.subject, &mut self.dataset.interner),
            InternedNamedNode::encoded_into(triple.predicate, &mut self.dataset.interner),
            InternedTerm::encoded_into(triple.object, &mut self.dataset.interner),
        )
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

    /// Checks if the graph contains the given triple
    pub fn contains<'b>(&self, triple: impl Into<TripleRef<'b>>) -> bool {
        self.read().contains(triple)
    }

    /// Returns the number of triples in this graph
    pub fn len(&self) -> usize {
        self.read().len()
    }

    /// Checks if this graph contains a triple
    pub fn is_empty(&self) -> bool {
        self.read().is_empty()
    }

    /// Dumps the graph into a file.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::io::GraphFormat;
    /// use oxigraph::model::Graph;
    ///
    /// let file = "<http://example.com> <http://example.com> <http://example.com> .\n".as_bytes();
    ///
    /// let mut store = Graph::new();
    /// store.load(file, GraphFormat::NTriples,None)?;
    ///
    /// let mut buffer = Vec::new();
    /// store.dump(&mut buffer, GraphFormat::NTriples)?;
    /// assert_eq!(file, buffer.as_slice());
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn dump(self, writer: impl Write, format: GraphFormat) -> Result<(), io::Error> {
        self.read().dump(writer, format)
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
            writeln!(f, "{}", t)?;
        }
        Ok(())
    }
}

/// Iterator returned by [`Dataset::iter`]
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

    fn next(&mut self) -> Option<QuadRef<'a>> {
        self.inner.next().map(|q| self.dataset.decode_spog(*q))
    }
}

/// Iterator returned by [`GraphView::iter`]
pub struct GraphViewIter<'a> {
    graph: GraphView<'a>,
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

    fn next(&mut self) -> Option<TripleRef<'a>> {
        self.inner.next().map(|t| self.graph.decode_gspo(*t))
    }
}
