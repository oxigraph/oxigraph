use std::cmp::Ordering;
use crate::jelly::rdf::rdf_literal::LiteralKind;
use crate::jelly::rdf::{RdfIri, RdfLiteral};
use crate::jelly::rdf::rdf_quad::{Graph, Object, Predicate, Subject};

fn compare_jelly_iris(x: &RdfIri, y: &RdfIri) -> Option<Ordering> {
    x.prefix_id.partial_cmp(&y.prefix_id)
        .map(|ord| match ord {
            Ordering::Equal => x.name_id.partial_cmp(&y.name_id),
            _ => Some(ord),
        }).flatten()
}

fn compare_jelly_literal_kinds(x: &LiteralKind, y: &LiteralKind) -> Option<Ordering> {
    match (x, y) {
        (LiteralKind::Datatype(datatype_x), LiteralKind::Datatype(datatype_y)) => datatype_x.partial_cmp(&datatype_y),
        (LiteralKind::Langtag(tag_x), LiteralKind::Langtag(tag_y)) => tag_x.partial_cmp(&tag_y),
        _ => None,
    }
}

fn compare_jelly_literals(x: &RdfLiteral, y: &RdfLiteral) -> Option<Ordering> {
    x.lex.partial_cmp(&y.lex)
        .map(|ord| match ord {
            Ordering::Equal => match (&x.literalKind, &y.literalKind) {
                (Some(x), Some(y)) => compare_jelly_literal_kinds(x, y),
                _ => None,
            }
            _ => Some(ord),
        }).flatten()
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SortableSubject(pub(crate) Subject);

impl PartialOrd for SortableSubject {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (&self.0, &other.0) {
            (Subject::SIri(x), Subject::SIri(y)) => compare_jelly_iris(x, y),
            (Subject::SBnode(x), Subject::SBnode(y)) => x.partial_cmp(&y),
            _ => todo!(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SortablePredicate(pub(crate) Predicate);

impl PartialOrd for SortablePredicate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (&self.0, &other.0) {
            (Predicate::PIri(x), Predicate::PIri(y)) => compare_jelly_iris(x, y),
            _ => todo!()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SortableObject(pub(crate) Object);

impl PartialOrd for SortableObject {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (&self.0, &other.0) {
            (Object::OIri(x), Object::OIri(y)) => compare_jelly_iris(x, y),
            (Object::OBnode(x), Object::OBnode(y)) => x.partial_cmp(y),
            (Object::OLiteral(x), Object::OLiteral(y)) => compare_jelly_literals(x, y),
            (Object::OTripleTerm(_x), Object::OTripleTerm(_y)) => todo!(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SortableGraphName(pub(crate) Graph);

impl PartialOrd for SortableGraphName {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (&self.0, &other.0) {
            (Graph::GIri(x), Graph::GIri(y)) => compare_jelly_iris(x, y),
            (Graph::GDefaultGraph(_), Graph::GDefaultGraph(_)) => Some(Ordering::Equal),
            _ => todo!()
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SortableRdfQuad {
    pub(crate) subject: SortableSubject,
    pub(crate) predicate: SortablePredicate,
    pub(crate) object: SortableObject,
    pub(crate) graph_name: SortableGraphName
}

impl Eq for SortableRdfQuad {}

impl PartialOrd<Self> for SortableRdfQuad {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.subject.partial_cmp(&other.subject)
            .map(|ord| if ord == Ordering::Equal {
                self.predicate.partial_cmp(&other.predicate)
                    .map(|ord| if ord == Ordering::Equal {
                        self.object.partial_cmp(&other.object)
                            .map(|ord| if ord == Ordering::Equal {
                                self.graph_name.partial_cmp(&other.graph_name)
                            } else { None })
                    } else { None })
            } else { None }).flatten().flatten().flatten()
    }
}

impl Ord for SortableRdfQuad {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Less)
    }
}