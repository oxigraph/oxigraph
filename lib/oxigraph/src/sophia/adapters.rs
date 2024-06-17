//! This module provide adapters for from Sophia to Oxigraph.
//! More precisely, every type MyX is cheaply built from a Sophia Term,
//! (provided that this term has the correct kind),
//! and can in turn can be cheaply converted into type X (via the `From` trait)
//! or type XRef (via its method `as_ref`).

use oxrdf::{
    BlankNode as OxBlankNode, BlankNodeRef, GraphName as OxGraphName, GraphNameRef, Literal,
    LiteralRef, NamedNode, NamedNodeRef, Subject, SubjectRef, Term as OxTerm, TermRef,
    Triple as OxTriple,
};
use sophia_api::{
    term::{BnodeId, IriRef, SimpleTerm, Term as SoTerm},
    MownStr,
};

/// This type is a mix of NamedNode and NamedNodeRef,
/// to accommodate the fact that some Sophia terms can be borrowed from,
/// while others give away the ownership of the data.
pub struct MyNamedNode<'a>(MownStr<'a>);

impl<'a> MyNamedNode<'a> {
    pub fn from_soterm<T: SoTerm + ?Sized>(t: &'a T) -> Option<Self> {
        t.iri().map(Self::from_iri)
    }
    pub fn from_iri(iri: IriRef<MownStr<'a>>) -> Self {
        MyNamedNode(iri.unwrap())
    }
    pub fn as_ref(&self) -> NamedNodeRef<'_> {
        NamedNodeRef::new_unchecked(self.0.as_ref())
    }
}

impl<'a> From<MyNamedNode<'a>> for NamedNode {
    fn from(value: MyNamedNode<'a>) -> Self {
        NamedNode::new_unchecked(value.0)
    }
}

/// This type is a mix of (Ox)BlankNode and BlankNodeRef,
/// to accommodate the fact that some Sophia terms can be borrowed from,
/// while others give away the ownership of the data.
pub struct MyBlankNode<'a>(MownStr<'a>);

impl<'a> MyBlankNode<'a> {
    pub fn from_bnode(iri: BnodeId<MownStr<'a>>) -> Self {
        MyBlankNode(iri.unwrap())
    }
    pub fn as_ref(&self) -> BlankNodeRef<'_> {
        BlankNodeRef::new_unchecked(self.0.as_ref())
    }
}

impl<'a> From<MyBlankNode<'a>> for OxBlankNode {
    fn from(value: MyBlankNode<'a>) -> Self {
        OxBlankNode::new_unchecked(value.0)
    }
}

/// This type is a mix of Literal and LiteralRef,
/// to accommodate the fact that some Sophia terms can be borrowed from,
/// while others give away the ownership of the data.
pub enum MyLiteral<'a> {
    Lang(MownStr<'a>, MownStr<'a>),
    Typed(MownStr<'a>, MyNamedNode<'a>),
}

impl<'a> MyLiteral<'a> {
    pub fn as_ref(&self) -> LiteralRef<'_> {
        match self {
            MyLiteral::Lang(lex, tag) => {
                LiteralRef::new_language_tagged_literal_unchecked(lex, tag)
            }
            MyLiteral::Typed(lex, dt) => LiteralRef::new_typed_literal(lex, dt.as_ref()),
        }
    }
}

impl<'a> From<MyLiteral<'a>> for Literal {
    fn from(value: MyLiteral<'a>) -> Self {
        match value {
            MyLiteral::Lang(lex, tag) => Literal::new_language_tagged_literal_unchecked(lex, tag),
            MyLiteral::Typed(lex, dt) => Literal::new_typed_literal(lex, dt),
        }
    }
}

/// This type is a mix of Subject and SubjectRef,
/// to accommodate the fact that some Sophia terms can be borrowed from,
/// while others give away the ownership of the data.
pub enum MySubject<'a> {
    NamedNode(MyNamedNode<'a>),
    BlankNode(MyBlankNode<'a>),
    Triple(Box<OxTriple>),
}

impl<'a> MySubject<'a> {
    pub fn from_soterm<T: SoTerm + ?Sized>(t: &'a T) -> Option<Self> {
        match t.as_simple() {
            SimpleTerm::Iri(iri) => Some(Self::NamedNode(MyNamedNode::from_iri(iri))),
            SimpleTerm::BlankNode(bnid) => Some(Self::BlankNode(MyBlankNode::from_bnode(bnid))),
            SimpleTerm::Triple(tr) => {
                let s = MySubject::from_soterm(&tr[0])?;
                let p = MyNamedNode::from_soterm(&tr[1])?;
                let o = MyTerm::from_soterm(&tr[2])?;
                Some(Self::Triple(Box::new(OxTriple::new(s, p, o))))
            }
            _ => None,
        }
    }

    pub fn as_ref(&self) -> SubjectRef<'_> {
        match self {
            MySubject::NamedNode(nn) => nn.as_ref().into(),
            MySubject::BlankNode(bn) => bn.as_ref().into(),
            MySubject::Triple(t) => SubjectRef::Triple(t),
        }
    }
}

impl<'a> From<MySubject<'a>> for Subject {
    fn from(value: MySubject<'a>) -> Self {
        match value {
            MySubject::NamedNode(nn) => Subject::NamedNode(nn.into()),
            MySubject::BlankNode(bn) => Subject::BlankNode(bn.into()),
            MySubject::Triple(t) => Subject::Triple(t),
        }
    }
}

/// This type is a mix of Term and TermRef,
/// to accommodate the fact that some Sophia terms can be borrowed from,
/// while others give away the ownership of the data.
pub enum MyTerm<'a> {
    NamedNode(MyNamedNode<'a>),
    BlankNode(MyBlankNode<'a>),
    Literal(MyLiteral<'a>),
    Triple(Box<OxTriple>),
}

impl<'a> MyTerm<'a> {
    pub fn from_soterm<T: SoTerm + ?Sized>(t: &'a T) -> Option<Self> {
        #[allow(clippy::match_wildcard_for_single_variants)]
        match t.as_simple() {
            SimpleTerm::Iri(iri) => Some(Self::NamedNode(MyNamedNode::from_iri(iri))),
            SimpleTerm::BlankNode(bnid) => Some(Self::BlankNode(MyBlankNode::from_bnode(bnid))),
            SimpleTerm::LiteralDatatype(lex, dt) => Some(Self::Literal(MyLiteral::Typed(
                lex,
                MyNamedNode::from_iri(dt),
            ))),
            SimpleTerm::LiteralLanguage(lex, tag) => {
                Some(Self::Literal(MyLiteral::Lang(lex, tag.unwrap())))
            }
            SimpleTerm::Triple(tr) => {
                let s = MySubject::from_soterm(&tr[0])?;
                let p = MyNamedNode::from_soterm(&tr[1])?;
                let o = MyTerm::from_soterm(&tr[2])?;
                Some(Self::Triple(Box::new(OxTriple::new(s, p, o))))
            }
            _ => None,
        }
    }

    pub fn as_ref(&'a self) -> TermRef<'a> {
        match self {
            MyTerm::NamedNode(nn) => nn.as_ref().into(),
            MyTerm::BlankNode(bn) => bn.as_ref().into(),
            MyTerm::Literal(lit) => lit.as_ref().into(),
            MyTerm::Triple(t) => TermRef::Triple(t),
        }
    }
}

impl<'a> From<MyTerm<'a>> for OxTerm {
    fn from(value: MyTerm<'a>) -> Self {
        match value {
            MyTerm::NamedNode(nn) => Self::NamedNode(nn.into()),
            MyTerm::BlankNode(bn) => Self::BlankNode(bn.into()),
            MyTerm::Literal(lit) => Self::Literal(lit.into()),
            MyTerm::Triple(t) => Self::Triple(t),
        }
    }
}

/// This type is a mix of GraphName and GraphNameRef,
/// to accommodate the fact that some Sophia terms can be borrowed from,
/// while others give away the ownership of the data.
pub enum MyGraphName<'a> {
    NamedNode(MyNamedNode<'a>),
    BlankNode(MyBlankNode<'a>),
    DefaultGraph,
}

impl<'a> MyGraphName<'a> {
    pub fn from_soterm<T: SoTerm + ?Sized>(t: Option<&'a T>) -> Option<Self> {
        match t.map(MySubject::from_soterm) {
            Some(Some(MySubject::NamedNode(nn))) => Some(Self::NamedNode(nn)),
            Some(Some(MySubject::BlankNode(bn))) => Some(Self::BlankNode(bn)),
            None => Some(Self::DefaultGraph),
            _ => None,
        }
    }

    pub fn as_ref(&'a self) -> GraphNameRef<'a> {
        match self {
            MyGraphName::NamedNode(nn) => nn.as_ref().into(),
            MyGraphName::BlankNode(bn) => bn.as_ref().into(),
            MyGraphName::DefaultGraph => GraphNameRef::DefaultGraph,
        }
    }
}

impl<'a> From<MyGraphName<'a>> for OxGraphName {
    fn from(value: MyGraphName<'a>) -> Self {
        match value {
            MyGraphName::NamedNode(nn) => Self::NamedNode(nn.into()),
            MyGraphName::BlankNode(bn) => Self::BlankNode(bn.into()),
            MyGraphName::DefaultGraph => Self::DefaultGraph,
        }
    }
}
