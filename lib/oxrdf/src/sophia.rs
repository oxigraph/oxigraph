use crate::{
    BlankNodeRef, GraphName as OxGraphName, GraphNameRef, LiteralRef, NamedNodeRef, Quad as OxQuad,
    QuadRef, SubjectRef, Term as OxTerm, TermRef, Triple as OxTriple, TripleRef,
};
use sophia_api::{
    quad::Quad as SoQuad,
    term::{BnodeId, IriRef, LanguageTag, SimpleTerm, Term as SoTerm, TermKind},
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

/// Extension trait for [`SimpleTerm`]
pub trait SimpleTermExt: Sized {
    /// Ensures that this SimpleTerm is as expected by oxrdf.
    ///
    /// This entails converting the language tag to lowercase.
    ///
    /// See also [`SimpleTermExt::normalized`]
    fn normalize(&mut self);

    /// Ensures that this SimpleTerm is as expected by oxrdf.
    ///
    /// This entails converting the language tag to lowercase.
    ///
    /// See also [`SimpleTermExt::normalize`]
    #[must_use]
    fn normalized(mut self) -> Self {
        self.normalize();
        self
    }

    /// Borrow this SimpleTerm as an OxRDF [`TermRef`].
    ///
    /// # Return
    /// None if self is a generalized RDF term not supported by OxRdf.
    ///
    /// # Precondition
    /// This must only be used on a [normalized](SimpleTermExt::normalize) SimpleTerm.
    fn as_term_ref(&self) -> Option<TermRef<'_>>;
}

impl SimpleTermExt for SimpleTerm<'_> {
    fn normalize(&mut self) {
        match self {
            SimpleTerm::LiteralLanguage(_, tag) => {
                if !tag.bytes().all(|b| b.is_ascii_lowercase()) {
                    *tag = LanguageTag::new_unchecked(tag.to_ascii_lowercase().into());
                }
            }
            SimpleTerm::Triple(triple) => {
                triple.iter_mut().for_each(SimpleTermExt::normalize);
            }
            _ => {}
        }
    }

    fn as_term_ref(&self) -> Option<TermRef<'_>> {
        match self {
            SimpleTerm::Iri(iri) => Some(TermRef::NamedNode(
                // NB: iri could be relative, which is not supported by NamedNodeRef
                NamedNodeRef::new(iri.as_str()).ok()?,
            )),
            SimpleTerm::BlankNode(bnid) => Some(TermRef::BlankNode(BlankNodeRef::new_unchecked(
                bnid.as_str(),
            ))),
            SimpleTerm::LiteralDatatype(lex, dt) => Some(TermRef::Literal(
                LiteralRef::new_typed_literal(lex.as_ref(), NamedNodeRef::new(dt.as_str()).ok()?),
            )),
            SimpleTerm::LiteralLanguage(lex, tag) => {
                debug_assert!(
                    tag.bytes().all(|b| b.is_ascii_lowercase()),
                    "SimpleTerm must be normalized"
                );
                Some(TermRef::Literal(
                    LiteralRef::new_language_tagged_literal_unchecked(lex.as_ref(), tag.as_str()),
                ))
            }
            SimpleTerm::Triple(_triple) => {
                None
                // should build a TripleRef here (only if feature rdf-star is enabled).
                // Unfortunately, TermRef::Triple expects a &Triple :-(
                // See https://github.com/oxigraph/oxigraph/issues/884
            }
            SimpleTerm::Variable(_) => None,
        }
    }
}

/// Extension trait for `[`[`SimpleTerm`]`; 3]`
pub trait SimpleTermTripleExt {
    /// Borrow this triple as an OxRDF [`TripleRef`].
    ///
    /// # Return
    /// None if this triple is a generalized RDF triple not supported by OxRdf.
    ///
    /// # Precondition
    /// This must only be used on a [normalized](SimpleTermExt::normalize) SimpleTerms.
    fn as_triple_ref(&self) -> Option<TripleRef<'_>>;
}

impl SimpleTermTripleExt for [SimpleTerm<'_>; 3] {
    fn as_triple_ref(&self) -> Option<TripleRef<'_>> {
        let s = match self[0].as_term_ref()? {
            TermRef::NamedNode(n) => SubjectRef::NamedNode(n),
            TermRef::BlankNode(b) => SubjectRef::BlankNode(b),
            #[cfg(feature = "rdf-star")]
            TermRef::Triple(t) => SubjectRef::Triple(t),
            TermRef::Literal(_) => {
                return None;
            }
        };
        let TermRef::NamedNode(p) = self[1].as_term_ref()? else {
            return None;
        };
        let o = self[2].as_term_ref()?;
        Some(TripleRef::new(s, p, o))
    }
}

/// Extension trait for [`sophia_api::triple::Triple`]
pub trait TripleExt<F, O> {
    /// Extract a [`TripleRef`] from self, and pass it to the given closure.
    ///
    /// # Return
    /// None if self is a generalized triple not supported by OxRdf,
    /// otherwise the result of the closure.
    fn pass_as_triple_ref(self, f: F) -> Option<O>;
}

impl<F, O, T> TripleExt<F, O> for T
where
    F: FnOnce(TripleRef<'_>) -> O,
    T: SoTriple,
{
    fn pass_as_triple_ref(self, f: F) -> Option<O> {
        let spo = self.to_spo();
        // let simple_spo = spo.each_ref().map(|t| t.as_simple().normalized()); // only stable in 1.77
        let simple_spo = [&spo[0], &spo[1], &spo[2]].map(|t| t.as_simple().normalized());
        let triple_ref = simple_spo.as_triple_ref()?;
        Some(f(triple_ref))
    }
}

/// Extension trait for [`sophia_api::quad::Quad`]
pub trait QuadExt<F, O> {
    /// Extract a [`QuadRef`] from self, and pass it to the given closure.
    ///
    /// # Return
    /// None if self is a generalized quad not supported by OxRdf,
    /// otherwise the result of the closure.
    fn pass_as_quad_ref(self, f: F) -> Option<O>;
}

impl<F, O, T> QuadExt<F, O> for T
where
    F: FnOnce(QuadRef<'_>) -> O,
    T: SoQuad,
{
    fn pass_as_quad_ref(self, f: F) -> Option<O> {
        let (spo, g) = self.to_spog();
        let simple_g = g.as_ref().map(|t| t.as_simple().normalized());
        let gname_ref = match &simple_g {
            None => GraphNameRef::DefaultGraph,
            Some(gn) => {
                let term_ref = gn.as_term_ref()?;
                match term_ref {
                    TermRef::NamedNode(n) => GraphNameRef::NamedNode(n),
                    TermRef::BlankNode(b) => GraphNameRef::BlankNode(b),
                    _ => {
                        return None;
                    }
                }
            }
        };
        spo.pass_as_triple_ref(|tr| {
            let q = QuadRef::new(tr.subject, tr.predicate, tr.object, gname_ref);

            f(q)
        })
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

        assert_eq!(t.as_simple().as_term_ref().unwrap(), t);
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

        assert_eq!(t.as_simple().as_term_ref().unwrap(), t);
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

        assert_eq!(t.as_simple().as_term_ref().unwrap(), t);
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

        assert_eq!(t.as_simple().as_term_ref().unwrap(), t);
    }

    #[test]
    fn language_string() {
        let t = OxTerm::from(Literal::new_language_tagged_literal_unchecked("chat", "fr"));
        assert_consistent_term_impl(&t);
        assert_consistent_term_impl(&t.as_ref());

        assert_eq!(t.as_simple().as_term_ref().unwrap(), t.as_ref());
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

        // assert_eq!(t.as_simple().as_term_ref().unwrap(), t); // TODO uncomment when #884 is solved
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
