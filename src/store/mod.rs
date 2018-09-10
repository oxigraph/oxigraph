pub mod isomorphism;
pub mod memory;
mod numeric_encoder;
pub mod rocksdb;
mod store;

use errors::*;
use model::*;

pub trait Graph {
    type TriplesIterator: Iterator<Item = Result<Triple>>;
    type TriplesForSubjectIterator: Iterator<Item = Result<Triple>>;
    type TriplesForSubjectPredicateIterator: Iterator<Item = Result<Triple>>;
    type TriplesForSubjectObjectIterator: Iterator<Item = Result<Triple>>;
    type TriplesForPredicateIterator: Iterator<Item = Result<Triple>>;
    type TriplesForPredicateObjectIterator: Iterator<Item = Result<Triple>>;
    type TriplesForObjectIterator: Iterator<Item = Result<Triple>>;

    fn iter(&self) -> Result<Self::TriplesIterator> {
        self.triples()
    }

    fn triples(&self) -> Result<Self::TriplesIterator>;

    fn triples_for_subject(
        &self,
        subject: &NamedOrBlankNode,
    ) -> Result<Self::TriplesForSubjectIterator>;

    fn triples_for_subject_predicate(
        &self,
        subject: &NamedOrBlankNode,
        predicate: &NamedNode,
    ) -> Result<Self::TriplesForSubjectPredicateIterator>;

    fn triples_for_subject_object(
        &self,
        subject: &NamedOrBlankNode,
        object: &Term,
    ) -> Result<Self::TriplesForSubjectObjectIterator>;

    fn triples_for_predicate(
        &self,
        predicate: &NamedNode,
    ) -> Result<Self::TriplesForPredicateIterator>;

    fn triples_for_predicate_object(
        &self,
        predicate: &NamedNode,
        object: &Term,
    ) -> Result<Self::TriplesForPredicateObjectIterator>;

    fn triples_for_object(&self, object: &Term) -> Result<Self::TriplesForObjectIterator>;

    fn contains(&self, triple: &Triple) -> Result<bool>;

    fn insert(&self, triple: &Triple) -> Result<()>;

    fn remove(&self, triple: &Triple) -> Result<()>;

    fn len(&self) -> Result<usize>;

    fn is_empty(&self) -> Result<bool>;
}

pub trait NamedGraph: Graph {
    fn name(&self) -> &NamedOrBlankNode;
}

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

    fn named_graph(&self, name: &NamedOrBlankNode) -> Result<Self::NamedGraph>;

    fn default_graph(&self) -> Self::DefaultGraph;

    fn union_graph(&self) -> Self::UnionGraph;

    fn iter(&self) -> Result<Self::QuadsIterator> {
        self.quads()
    }

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

    fn contains(&self, quad: &Quad) -> Result<bool>;

    fn insert(&self, quad: &Quad) -> Result<()>;

    fn remove(&self, quad: &Quad) -> Result<()>;

    fn len(&self) -> Result<usize>;

    fn is_empty(&self) -> Result<bool>;
}
