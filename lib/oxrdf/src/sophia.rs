use crate::{
    GraphName as OxGraphName, Quad as OxQuad, Term as OxTerm, TermRef, Triple as OxTriple,
};
use sophia_api::{
    quad::Quad as SoQuad,
    term::{BnodeId, IriRef, LanguageTag, Term as SoTerm, TermKind},
    triple::Triple as SoTriple,
    MownStr,
};

impl<'a> SoTerm for TermRef<'a> {
    type BorrowTerm<'x> = Self where Self: 'x;

    fn kind(&self) -> TermKind {
        match self {
            TermRef::NamedNode(_) => TermKind::Iri,
            TermRef::BlankNode(_) => TermKind::BlankNode,
            TermRef::Literal(_) => TermKind::Literal,
            #[cfg(feature = "rdf-star")]
            TermRef::Triple(_) => TermKind::Triple,
        }
    }

    fn borrow_term(&self) -> Self::BorrowTerm<'_> {
        *self
    }

    fn iri(&self) -> Option<IriRef<MownStr<'_>>> {
        if let TermRef::NamedNode(iri) = self {
            Some(IriRef::new_unchecked(iri.as_str().into()))
        } else {
            None
        }
    }

    fn bnode_id(&self) -> Option<BnodeId<MownStr<'_>>> {
        if let TermRef::BlankNode(bnid) = self {
            Some(BnodeId::new_unchecked(bnid.as_str().into()))
        } else {
            None
        }
    }

    fn lexical_form(&self) -> Option<MownStr<'_>> {
        if let TermRef::Literal(lit) = self {
            Some(lit.value().into())
        } else {
            None
        }
    }

    fn datatype(&self) -> Option<IriRef<MownStr<'_>>> {
        if let TermRef::Literal(lit) = self {
            Some(IriRef::new_unchecked(lit.datatype().as_str().into()))
        } else {
            None
        }
    }

    fn language_tag(&self) -> Option<LanguageTag<MownStr<'_>>> {
        if let TermRef::Literal(lit) = self {
            lit.language()
                .map(|tag| LanguageTag::new_unchecked(tag.into()))
        } else {
            None
        }
    }

    fn triple(&self) -> Option<[Self::BorrowTerm<'_>; 3]> {
        self.to_triple()
    }

    fn to_triple(self) -> Option<[Self; 3]>
    where
        Self: Sized,
    {
        #[cfg(feature = "rdf-star")]
        if let TermRef::Triple(t) = self {
            return Some([
                t.subject.as_ref().into(),
                t.predicate.as_ref().into(),
                t.object.as_ref(),
            ]);
        }
        None
    }
}

impl SoTerm for OxTerm {
    type BorrowTerm<'x> = TermRef<'x> where Self: 'x;

    fn kind(&self) -> TermKind {
        match self {
            OxTerm::NamedNode(_) => TermKind::Iri,
            OxTerm::BlankNode(_) => TermKind::BlankNode,
            OxTerm::Literal(_) => TermKind::Literal,
            #[cfg(feature = "rdf-star")]
            OxTerm::Triple(_) => TermKind::Triple,
        }
    }

    fn borrow_term(&self) -> Self::BorrowTerm<'_> {
        self.as_ref()
    }

    fn iri(&self) -> Option<IriRef<MownStr<'_>>> {
        if let OxTerm::NamedNode(iri) = self {
            Some(IriRef::new_unchecked(iri.as_str().into()))
        } else {
            None
        }
    }

    fn bnode_id(&self) -> Option<BnodeId<MownStr<'_>>> {
        if let OxTerm::BlankNode(bnid) = self {
            Some(BnodeId::new_unchecked(bnid.as_str().into()))
        } else {
            None
        }
    }

    fn lexical_form(&self) -> Option<MownStr<'_>> {
        if let OxTerm::Literal(lit) = self {
            Some(lit.value().into())
        } else {
            None
        }
    }

    fn datatype(&self) -> Option<IriRef<MownStr<'_>>> {
        if let OxTerm::Literal(lit) = self {
            Some(IriRef::new_unchecked(lit.datatype().as_str().into()))
        } else {
            None
        }
    }

    fn language_tag(&self) -> Option<LanguageTag<MownStr<'_>>> {
        if let OxTerm::Literal(lit) = self {
            lit.language()
                .map(|tag| LanguageTag::new_unchecked(tag.into()))
        } else {
            None
        }
    }

    fn triple(&self) -> Option<[Self::BorrowTerm<'_>; 3]> {
        #[cfg(feature = "rdf-star")]
        if let OxTerm::Triple(t) = self {
            return Some([
                t.subject.as_ref().into(),
                t.predicate.as_ref().into(),
                t.object.as_ref(),
            ]);
        }
        None
    }

    fn to_triple(self) -> Option<[Self; 3]>
    where
        Self: Sized,
    {
        #[cfg(feature = "rdf-star")]
        if let OxTerm::Triple(t) = self {
            return Some([t.subject.into(), t.predicate.into(), t.object]);
        }
        None
    }
}

impl SoTriple for OxTriple {
    type Term = OxTerm;

    fn s(&self) -> sophia_api::triple::TBorrowTerm<'_, Self> {
        self.subject.as_ref().into()
    }

    fn p(&self) -> sophia_api::triple::TBorrowTerm<'_, Self> {
        self.predicate.as_ref().into()
    }

    fn o(&self) -> sophia_api::triple::TBorrowTerm<'_, Self> {
        self.object.as_ref()
    }

    fn to_spo(self) -> [Self::Term; 3] {
        [self.subject.into(), self.predicate.into(), self.object]
    }
}

impl SoQuad for OxQuad {
    type Term = OxTerm;

    fn s(&self) -> sophia_api::quad::QBorrowTerm<'_, Self> {
        self.subject.as_ref().into()
    }

    fn p(&self) -> sophia_api::quad::QBorrowTerm<'_, Self> {
        self.predicate.as_ref().into()
    }

    fn o(&self) -> sophia_api::quad::QBorrowTerm<'_, Self> {
        self.object.as_ref()
    }

    fn g(&self) -> sophia_api::term::GraphName<sophia_api::quad::QBorrowTerm<'_, Self>> {
        match &self.graph_name {
            OxGraphName::NamedNode(gn) => Some(gn.as_ref().into()),
            OxGraphName::BlankNode(gn) => Some(gn.as_ref().into()),
            OxGraphName::DefaultGraph => None,
        }
    }

    fn to_spog(self) -> sophia_api::quad::Spog<Self::Term> {
        (
            [self.subject.into(), self.predicate.into(), self.object],
            match self.graph_name {
                OxGraphName::NamedNode(gn) => Some(gn.into()),
                OxGraphName::BlankNode(gn) => Some(gn.into()),
                OxGraphName::DefaultGraph => None,
            },
        )
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{BlankNode, Literal, NamedNode};
    use sophia_api::{ns::xsd, term::assert_consistent_term_impl};

    #[test]
    fn named_node() {
        let value = "https://example.org/ns/foo";
        let t = OxTerm::from(NamedNode::new_unchecked(value));
        assert_consistent_term_impl(&t);
        assert_eq!(t.kind(), TermKind::Iri);
        assert_eq!(&t.iri().unwrap(), value);
        let t = t.as_ref();
        assert_consistent_term_impl(&t);
        assert_eq!(t.kind(), TermKind::Iri);
        assert_eq!(&t.iri().unwrap(), value);
    }

    #[test]
    fn blank_node() {
        let bnid = "b01";
        let t = OxTerm::from(BlankNode::new_unchecked(bnid));
        assert_consistent_term_impl(&t);
        assert_eq!(t.kind(), TermKind::BlankNode);
        assert_eq!(&t.bnode_id().unwrap(), bnid);
        let t = t.as_ref();
        assert_consistent_term_impl(&t);
        assert_eq!(t.kind(), TermKind::BlankNode);
        assert_eq!(&t.bnode_id().unwrap(), bnid);
    }

    #[test]
    fn simple_literal() {
        let value = "hello world";
        let t = OxTerm::from(Literal::new_simple_literal(value));
        assert_consistent_term_impl(&t);
        assert_eq!(t.kind(), TermKind::Literal);
        assert_eq!(t.lexical_form().unwrap(), value);
        assert_eq!(t.datatype(), xsd::string.iri());
        let t = t.as_ref();
        assert_consistent_term_impl(&t);
        assert_eq!(t.kind(), TermKind::Literal);
        assert_eq!(t.lexical_form().unwrap(), value);
        assert_eq!(t.datatype(), xsd::string.iri());
    }

    #[test]
    fn typed_literal() {
        let value = "42";
        let t = OxTerm::from(Literal::new_typed_literal(
            value,
            NamedNode::new_unchecked(xsd::integer.iri().unwrap().to_string()),
        ));
        assert_consistent_term_impl(&t);
        assert_eq!(t.kind(), TermKind::Literal);
        assert_eq!(t.lexical_form().unwrap(), value);
        assert_eq!(t.datatype(), xsd::integer.iri());
        let t = t.as_ref();
        assert_consistent_term_impl(&t);
        assert_eq!(t.kind(), TermKind::Literal);
        assert_eq!(t.lexical_form().unwrap(), value);
        assert_eq!(t.datatype(), xsd::integer.iri());
    }

    #[test]
    fn language_string() {
        let t = OxTerm::from(Literal::new_language_tagged_literal_unchecked("chat", "fr"));
        assert_consistent_term_impl(&t);
        assert_consistent_term_impl(&t.as_ref());
    }

    #[cfg(feature = "rdf-star")]
    #[test]
    fn triple_term() {
        let t = OxTerm::from(OxTriple::new(
            BlankNode::new_unchecked("b01"),
            NamedNode::new_unchecked("https://example.org/ns/foo"),
            Literal::new_simple_literal("bar"),
        ));
        assert_consistent_term_impl(&t);
        assert_eq!(t.kind(), TermKind::Triple);
        assert!(t.triple().unwrap().s().is_blank_node());
        assert!(t.triple().unwrap().p().is_iri());
        assert!(t.triple().unwrap().o().is_literal());
        let t = t.as_ref();
        assert_consistent_term_impl(&t);
        assert_eq!(t.kind(), TermKind::Triple);
        assert!(t.triple().unwrap().s().is_blank_node());
        assert!(t.triple().unwrap().p().is_iri());
        assert!(t.triple().unwrap().o().is_literal());
    }

    #[test]
    fn triple() {
        let t = OxTriple::new(
            BlankNode::new_unchecked("b01"),
            NamedNode::new_unchecked("https://example.org/ns/foo"),
            Literal::new_simple_literal("bar"),
        );
        assert!(t.s().is_blank_node());
        assert!(t.p().is_iri());
        assert!(t.o().is_literal());
    }

    #[test]
    fn quad() {
        let t = OxQuad::new(
            BlankNode::new_unchecked("b01"),
            NamedNode::new_unchecked("https://example.org/ns/foo"),
            Literal::new_simple_literal("bar"),
            OxGraphName::DefaultGraph,
        );
        assert!(t.s().is_blank_node());
        assert!(t.p().is_iri());
        assert!(t.o().is_literal());
        assert!(t.g().is_none());
    }
}
