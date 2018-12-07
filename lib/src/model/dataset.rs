use crate::model::*;
use crate::Result;

/// Trait for [RDF graphs](https://www.w3.org/TR/rdf11-concepts/#dfn-graph)
///
/// This crate currently provides a simple stand-alone in memory implementation of the `Graph` trait.
///
/// Usage example:
/// ```
/// use rudf::model::*;
/// use rudf::store::MemoryGraph;
/// use std::str::FromStr;
///
/// let graph = MemoryGraph::default();
/// let ex = NamedNode::from_str("http://example.com").unwrap();
/// let triple = Triple::new(ex.clone(), ex.clone(), ex.clone());
/// graph.insert(&triple);
/// let results: Vec<Triple> = graph.triples_for_subject(&ex.into()).unwrap().map(|t| t.unwrap()).collect();
/// assert_eq!(vec![triple], results);
/// ```
pub trait Graph {
    type TriplesIterator: Iterator<Item = Result<Triple>>;
    type TriplesForSubjectIterator: Iterator<Item = Result<Triple>>;
    type ObjectsForSubjectPredicateIterator: Iterator<Item = Result<Term>>;
    type PredicatesForSubjectObjectIterator: Iterator<Item = Result<NamedNode>>;
    type TriplesForPredicateIterator: Iterator<Item = Result<Triple>>;
    type SubjectsForPredicateObjectIterator: Iterator<Item = Result<NamedOrBlankNode>>;
    type TriplesForObjectIterator: Iterator<Item = Result<Triple>>;

    /// Returns all triples contained by the graph
    fn iter(&self) -> Result<Self::TriplesIterator> {
        self.triples()
    }

    /// Returns all triples contained by the graph
    fn triples(&self) -> Result<Self::TriplesIterator>;

    fn triples_for_subject(
        &self,
        subject: &NamedOrBlankNode,
    ) -> Result<Self::TriplesForSubjectIterator>;

    fn objects_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> Result<Self::ObjectsForSubjectPredicateIterator>;

    fn object_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> Result<Option<Term>> {
        //TODO use transpose when stable
        match self
            .objects_for_subject_predicate(subject, predicate)?
            .nth(0)
        {
            Some(object) => Ok(Some(object?)),
            None => Ok(None),
        }
    }

    fn predicates_for_subject_object(
        &self,
        subject: &NamedOrBlankNode,
        object: &Term,
    ) -> Result<Self::PredicatesForSubjectObjectIterator>;

    fn triples_for_predicate(
        &self,
        predicate: &NamedNode,
    ) -> Result<Self::TriplesForPredicateIterator>;

    fn subjects_for_predicate_object(
        &self,
        predicate: &NamedNode,
        object: &Term,
    ) -> Result<Self::SubjectsForPredicateObjectIterator>;

    fn triples_for_object(&self, object: &Term) -> Result<Self::TriplesForObjectIterator>;

    /// Checks if the graph contains the given triple
    fn contains(&self, triple: &Triple) -> Result<bool>;

    /// Adds a triple to the graph
    fn insert(&self, triple: &Triple) -> Result<()>;

    /// Removes a concrete triple from the graph
    fn remove(&self, triple: &Triple) -> Result<()>;

    /// Returns the number of triples in this graph
    fn len(&self) -> Result<usize>;

    /// Checks if this graph contains a triple
    fn is_empty(&self) -> Result<bool>;
}

/// Trait for [RDF named graphs](https://www.w3.org/TR/rdf11-concepts/#dfn-named-graph) i.e. RDF graphs identified by an IRI
pub trait NamedGraph: Graph {
    fn name(&self) -> &NamedOrBlankNode;
}

/// Trait for [RDF datasets](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
///
/// This crate currently provides two implementation of the `Dataset` traits:
/// * One in memory: `rudf::store::MemoryDataset`
/// * One disk-based using [RocksDB](https://rocksdb.org/): `rudf::store::RocksDbDataset`
///
/// Usage example with `rudf::store::MemoryDataset`:
/// ```
/// use rudf::model::*;
/// use rudf::store::MemoryDataset;
/// use std::str::FromStr;
///
/// let dataset = MemoryDataset::default();
/// let default_graph = dataset.default_graph();
/// let ex = NamedNode::from_str("http://example.com").unwrap();
/// let triple = Triple::new(ex.clone(), ex.clone(), ex.clone());
/// default_graph.insert(&triple);
/// let results: Vec<Quad> = dataset.quads_for_subject(&ex.into()).unwrap().map(|t| t.unwrap()).collect();
/// assert_eq!(vec![triple.in_graph(None)], results);
/// ```
///
/// The implementation backed by RocksDB if disabled by default and requires the `"rocksdb"` feature to be activated.
/// A `RocksDbDataset` could be built using `RocksDbDataset::open` and works just like its in-memory equivalent:
/// ```ignore
/// use rudf::store::RocksDbDataset;
/// let dataset = RocksDbDataset::open("foo.db");
/// ```
pub trait Dataset {
    type NamedGraph: NamedGraph;
    type DefaultGraph: Graph;
    type UnionGraph: Graph;
    type QuadsIterator: Iterator<Item = Result<Quad>>;
    type QuadsForSubjectIterator: Iterator<Item = Result<Quad>>;
    type QuadsForSubjectPredicateIterator: Iterator<Item = Result<Quad>>;
    type QuadsForSubjectPredicateObjectIterator: Iterator<Item = Result<Quad>>;
    type QuadsForSubjectObjectIterator: Iterator<Item = Result<Quad>>;
    type QuadsForPredicateIterator: Iterator<Item = Result<Quad>>;
    type QuadsForPredicateObjectIterator: Iterator<Item = Result<Quad>>;
    type QuadsForObjectIterator: Iterator<Item = Result<Quad>>;

    /// Returns an object for a [named graph](https://www.w3.org/TR/rdf11-concepts/#dfn-named-graph) of this dataset.
    ///
    /// This named graph may be empty if no triple is in the graph yet.
    fn named_graph(&self, name: &NamedOrBlankNode) -> Result<Self::NamedGraph>;

    /// Returns an object for the [default graph](https://www.w3.org/TR/rdf11-concepts/#dfn-default-graph) of this dataset
    fn default_graph(&self) -> Self::DefaultGraph;

    /// Returns a graph that is the union of all graphs contained in this dataset, including the default graph
    fn union_graph(&self) -> Self::UnionGraph;

    /// Returns all quads contained by the graph
    fn iter(&self) -> Result<Self::QuadsIterator> {
        self.quads()
    }

    /// Returns all quads contained by the graph
    fn quads(&self) -> Result<Self::QuadsIterator>;

    fn quads_for_subject(
        &self,
        subject: &NamedOrBlankNode,
    ) -> Result<Self::QuadsForSubjectIterator>;

    fn quads_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> Result<Self::QuadsForSubjectPredicateIterator>;

    fn quads_for_subject_predicate_object(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
        object: &Term,
    ) -> Result<Self::QuadsForSubjectPredicateObjectIterator>;

    fn quads_for_subject_object(
        &self,
        subject: &NamedOrBlankNode,
        object: &Term,
    ) -> Result<Self::QuadsForSubjectObjectIterator>;

    fn quads_for_predicate(&self, predicate: &NamedNode)
        -> Result<Self::QuadsForPredicateIterator>;

    fn quads_for_predicate_object(
        &self,
        predicate: &NamedNode,
        object: &Term,
    ) -> Result<Self::QuadsForPredicateObjectIterator>;

    fn quads_for_object(&self, object: &Term) -> Result<Self::QuadsForObjectIterator>;

    /// Checks if this dataset contains a given quad
    fn contains(&self, quad: &Quad) -> Result<bool>;

    /// Adds a quad to this dataset
    fn insert(&self, quad: &Quad) -> Result<()>;

    /// Removes a quad from this dataset
    fn remove(&self, quad: &Quad) -> Result<()>;

    /// Returns the number of quads in this dataset
    fn len(&self) -> Result<usize>;

    /// Checks if this dataset contains a quad
    fn is_empty(&self) -> Result<bool>;
}
