//! This crate provides implementation of [Sophia](https://docs.rs/sophia/) traits for the `model` module.

use crate::model::*;
use sophia_api::term::*;
use std::fmt;

impl TTerm for BlankNode {
    fn kind(&self) -> TermKind {
        TermKind::BlankNode
    }

    fn value_raw(&self) -> RawValue<'_> {
        self.as_str().into()
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl TryCopyTerm for BlankNode {
    type Error = SophiaToOxigraphConversionError;

    fn try_copy<T>(other: &T) -> Result<Self, Self::Error>
    where
        T: TTerm + ?Sized,
    {
        match other.kind() {
            TermKind::BlankNode => Ok(BlankNode::new_unchecked(other.value_raw().0)),
            _ => Err(SophiaToOxigraphConversionError),
        }
    }
}

impl<'a> TTerm for BlankNodeRef<'a> {
    fn kind(&self) -> TermKind {
        TermKind::BlankNode
    }

    fn value_raw(&self) -> RawValue<'_> {
        self.as_str().into()
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl TTerm for Literal {
    fn kind(&self) -> TermKind {
        TermKind::Literal
    }

    fn value_raw(&self) -> RawValue<'_> {
        Literal::value(self).into()
    }

    fn datatype(&self) -> Option<SimpleIri<'_>> {
        Some(SimpleIri::new_unchecked(
            Literal::datatype(self).as_str(),
            None,
        ))
    }

    fn language(&self) -> Option<&str> {
        Literal::language(self)
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl TryCopyTerm for Literal {
    type Error = SophiaToOxigraphConversionError;

    fn try_copy<T>(other: &T) -> Result<Self, Self::Error>
    where
        T: TTerm + ?Sized,
    {
        match other.kind() {
            TermKind::Literal => match other.language() {
                Some(tag) => Ok(Literal::new_language_tagged_literal_unchecked(
                    other.value_raw().0,
                    tag,
                )),
                None => Ok(Literal::new_typed_literal(
                    other.value_raw().0,
                    other.datatype().unwrap(),
                )),
            },
            _ => Err(SophiaToOxigraphConversionError),
        }
    }
}

impl<'a> TTerm for LiteralRef<'a> {
    fn kind(&self) -> TermKind {
        TermKind::Literal
    }

    fn value_raw(&self) -> RawValue<'_> {
        LiteralRef::value(*self).into()
    }

    fn datatype(&self) -> Option<SimpleIri<'_>> {
        Some(SimpleIri::new_unchecked(
            LiteralRef::datatype(*self).as_str(),
            None,
        ))
    }

    fn language(&self) -> Option<&str> {
        LiteralRef::language(*self)
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl TTerm for NamedNode {
    fn kind(&self) -> TermKind {
        TermKind::Iri
    }

    fn value_raw(&self) -> RawValue<'_> {
        self.as_str().into()
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl TryCopyTerm for NamedNode {
    type Error = SophiaToOxigraphConversionError;

    fn try_copy<T>(other: &T) -> Result<Self, Self::Error>
    where
        T: TTerm + ?Sized,
    {
        match other.kind() {
            TermKind::Iri => Ok(NamedNode::new_unchecked(other.value())),
            _ => Err(SophiaToOxigraphConversionError),
        }
    }
}

impl<'a> From<SimpleIri<'a>> for NamedNode {
    fn from(other: SimpleIri<'a>) -> Self {
        NamedNode::new_unchecked(other.value())
    }
}

impl<'a> TTerm for NamedNodeRef<'a> {
    fn kind(&self) -> TermKind {
        TermKind::BlankNode
    }

    fn value_raw(&self) -> RawValue<'_> {
        self.as_str().into()
    }

    fn as_dyn(&self) -> &dyn TTerm {
        self
    }
}

impl From<GraphName> for Option<Term> {
    fn from(other: GraphName) -> Self {
        use GraphName::*;
        match other {
            NamedNode(n) => Some(n.into()),
            BlankNode(n) => Some(n.into()),
            DefaultGraph => None,
        }
    }
}

impl<'a> From<GraphNameRef<'a>> for Option<TermRef<'a>> {
    fn from(other: GraphNameRef<'a>) -> Self {
        use GraphNameRef::*;
        match other {
            NamedNode(n) => Some(n.into()),
            BlankNode(n) => Some(n.into()),
            DefaultGraph => None,
        }
    }
}

impl TTerm for NamedOrBlankNode {
    fn kind(&self) -> TermKind {
        use NamedOrBlankNode::*;
        match self {
            NamedNode(_) => TermKind::Iri,
            BlankNode(_) => TermKind::BlankNode,
        }
    }

    fn value_raw(&self) -> RawValue<'_> {
        use NamedOrBlankNode::*;
        match self {
            NamedNode(n) => n.value_raw(),
            BlankNode(n) => n.value_raw(),
        }
    }

    fn as_dyn(&self) -> &dyn TTerm {
        use NamedOrBlankNode::*;
        match self {
            NamedNode(n) => n.as_dyn(),
            BlankNode(n) => n.as_dyn(),
        }
    }
}

impl TryCopyTerm for NamedOrBlankNode {
    type Error = SophiaToOxigraphConversionError;

    fn try_copy<T>(other: &T) -> Result<Self, Self::Error>
    where
        T: TTerm + ?Sized,
    {
        match other.kind() {
            TermKind::Iri => Ok(NamedNode::try_copy(other).unwrap().into()),
            TermKind::BlankNode => Ok(BlankNode::try_copy(other).unwrap().into()),
            _ => Err(SophiaToOxigraphConversionError),
        }
    }
}

impl<'a> TTerm for NamedOrBlankNodeRef<'a> {
    fn kind(&self) -> TermKind {
        use NamedOrBlankNodeRef::*;
        match self {
            NamedNode(_) => TermKind::Iri,
            BlankNode(_) => TermKind::BlankNode,
        }
    }

    fn value_raw(&self) -> RawValue<'_> {
        use NamedOrBlankNodeRef::*;
        match self {
            NamedNode(n) => n.value_raw(),
            BlankNode(n) => n.value_raw(),
        }
    }

    fn as_dyn(&self) -> &dyn TTerm {
        use NamedOrBlankNodeRef::*;
        match self {
            NamedNode(n) => n.as_dyn(),
            BlankNode(n) => n.as_dyn(),
        }
    }
}

impl TTerm for Term {
    fn kind(&self) -> TermKind {
        use Term::*;
        match self {
            NamedNode(_) => TermKind::Iri,
            BlankNode(_) => TermKind::BlankNode,
            Literal(_) => TermKind::Literal,
        }
    }

    fn value_raw(&self) -> RawValue<'_> {
        use Term::*;
        match self {
            NamedNode(n) => n.value_raw(),
            BlankNode(n) => n.value_raw(),
            Literal(l) => l.value_raw(),
        }
    }

    fn datatype(&self) -> Option<SimpleIri<'_>> {
        use Term::*;
        match self {
            Literal(l) => TTerm::datatype(l),
            _ => None,
        }
    }

    fn language(&self) -> Option<&str> {
        use Term::*;
        match self {
            Literal(l) => TTerm::language(l),
            _ => None,
        }
    }

    fn as_dyn(&self) -> &dyn TTerm {
        use Term::*;
        match self {
            NamedNode(n) => n.as_dyn(),
            BlankNode(n) => n.as_dyn(),
            Literal(l) => l.as_dyn(),
        }
    }
}

impl TryCopyTerm for Term {
    type Error = SophiaToOxigraphConversionError;

    fn try_copy<T>(other: &T) -> Result<Self, Self::Error>
    where
        T: TTerm + ?Sized,
    {
        match other.kind() {
            TermKind::Iri => Ok(NamedNode::try_copy(other).unwrap().into()),
            TermKind::BlankNode => Ok(BlankNode::try_copy(other).unwrap().into()),
            TermKind::Literal => Ok(Literal::try_copy(other).unwrap().into()),
            _ => Err(SophiaToOxigraphConversionError),
        }
    }
}

impl<'a> TTerm for TermRef<'a> {
    fn kind(&self) -> TermKind {
        use TermRef::*;
        match self {
            NamedNode(_) => TermKind::Iri,
            BlankNode(_) => TermKind::BlankNode,
            Literal(_) => TermKind::Literal,
        }
    }

    fn value_raw(&self) -> RawValue<'_> {
        use TermRef::*;
        match self {
            NamedNode(n) => n.value_raw(),
            BlankNode(n) => n.value_raw(),
            Literal(l) => l.value_raw(),
        }
    }

    fn datatype(&self) -> Option<SimpleIri<'_>> {
        use TermRef::*;
        match self {
            Literal(l) => TTerm::datatype(l),
            _ => None,
        }
    }

    fn language(&self) -> Option<&str> {
        use TermRef::*;
        match self {
            Literal(l) => TTerm::language(l),
            _ => None,
        }
    }

    fn as_dyn(&self) -> &dyn TTerm {
        use TermRef::*;
        match self {
            NamedNode(n) => n.as_dyn(),
            BlankNode(n) => n.as_dyn(),
            Literal(l) => l.as_dyn(),
        }
    }
}

impl From<Quad> for ([Term; 3], Option<Term>) {
    fn from(other: Quad) -> Self {
        (
            [other.subject.into(), other.predicate.into(), other.object],
            other.graph_name.into(),
        )
    }
}

impl<'a> From<QuadRef<'a>> for ([TermRef<'a>; 3], Option<TermRef<'a>>) {
    fn from(other: QuadRef<'a>) -> Self {
        (
            [other.subject.into(), other.predicate.into(), other.object],
            other.graph_name.into(),
        )
    }
}

impl From<Triple> for [Term; 3] {
    fn from(other: Triple) -> Self {
        [other.subject.into(), other.predicate.into(), other.object]
    }
}

impl<'a> From<TripleRef<'a>> for [TermRef<'a>; 3] {
    fn from(other: TripleRef<'a>) -> Self {
        [other.subject.into(), other.predicate.into(), other.object]
    }
}

/// Error raised when trying to copy a [Sophia](sophia)
/// term as an incompatible Oxigraph term
/// (e.g. a literal into `NamedNode`).
#[derive(Clone, Copy, Debug)]
pub struct SophiaToOxigraphConversionError;

impl fmt::Display for SophiaToOxigraphConversionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for SophiaToOxigraphConversionError {}
