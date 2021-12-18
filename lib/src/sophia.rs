//! This crate provides implementation of [Sophia](https://docs.rs/sophia/) traits for the `store` module.

use crate::model::{
    BlankNodeRef, GraphName, GraphNameRef, LiteralRef, NamedNodeRef, Quad, QuadRef, Subject,
    SubjectRef, Term, TermRef,
};
use crate::store::{StorageError, Store};
use sophia_api::dataset::{
    CollectibleDataset, DQuadSource, DResultTermSet, DTerm, Dataset, MdResult, MutableDataset,
};
use sophia_api::quad::stream::{QuadSource, StreamResult};
use sophia_api::quad::streaming_mode::{ByValue, StreamedQuad};
use sophia_api::term::{TTerm, TermKind};
use std::collections::HashSet;
use std::hash::Hash;
use std::iter::empty;

type SophiaQuad = ([Term; 3], Option<Term>);
type StreamedSophiaQuad<'a> = StreamedQuad<'a, ByValue<SophiaQuad>>;

impl Dataset for Store {
    type Quad = ByValue<SophiaQuad>;
    type Error = StorageError;

    fn quads(&self) -> DQuadSource<'_, Self> {
        Box::new(self.iter().map(quad_map))
    }

    fn quads_with_s<'s, TS>(&'s self, s: &'s TS) -> DQuadSource<'s, Self>
    where
        TS: TTerm + ?Sized,
    {
        let mut buf_s = String::new();
        let s = convert_subject(s, &mut buf_s);
        if s.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(s, None, None, None).map(quad_map))
        }
    }
    fn quads_with_p<'s, TP>(&'s self, p: &'s TP) -> DQuadSource<'s, Self>
    where
        TP: TTerm + ?Sized,
    {
        let mut buf_p = String::new();
        let p = convert_predicate(p, &mut buf_p);
        if p.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(None, p, None, None).map(quad_map))
        }
    }
    fn quads_with_o<'s, TS>(&'s self, o: &'s TS) -> DQuadSource<'s, Self>
    where
        TS: TTerm + ?Sized,
    {
        let mut buf_o = String::new();
        let o = convert_object(o, &mut buf_o);
        if o.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(None, None, o, None).map(quad_map))
        }
    }
    fn quads_with_g<'s, TS>(&'s self, g: Option<&'s TS>) -> DQuadSource<'s, Self>
    where
        TS: TTerm + ?Sized,
    {
        let mut buf_g = String::new();
        let g = convert_graph_name(g, &mut buf_g);
        if g.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(None, None, None, g).map(quad_map))
        }
    }
    fn quads_with_sp<'s, TS, TP>(&'s self, s: &'s TS, p: &'s TP) -> DQuadSource<'s, Self>
    where
        TS: TTerm + ?Sized,
        TP: TTerm + ?Sized,
    {
        let mut buf_s = String::new();
        let s = convert_subject(s, &mut buf_s);
        let mut buf_p = String::new();
        let p = convert_predicate(p, &mut buf_p);
        if s.is_none() || p.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(s, p, None, None).map(quad_map))
        }
    }
    fn quads_with_so<'s, TS, TO>(&'s self, s: &'s TS, o: &'s TO) -> DQuadSource<'s, Self>
    where
        TS: TTerm + ?Sized,
        TO: TTerm + ?Sized,
    {
        let mut buf_s = String::new();
        let s = convert_subject(s, &mut buf_s);
        let mut buf_o = String::new();
        let o = convert_object(o, &mut buf_o);
        if s.is_none() || o.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(s, None, o, None).map(quad_map))
        }
    }
    fn quads_with_sg<'s, TS, TG>(&'s self, s: &'s TS, g: Option<&'s TG>) -> DQuadSource<'s, Self>
    where
        TS: TTerm + ?Sized,
        TG: TTerm + ?Sized,
    {
        let mut buf_s = String::new();
        let s = convert_subject(s, &mut buf_s);
        let mut buf_g = String::new();
        let g = convert_graph_name(g, &mut buf_g);
        if s.is_none() || g.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(s, None, None, g).map(quad_map))
        }
    }
    fn quads_with_po<'s, TP, TO>(&'s self, p: &'s TP, o: &'s TO) -> DQuadSource<'s, Self>
    where
        TP: TTerm + ?Sized,
        TO: TTerm + ?Sized,
    {
        let mut buf_p = String::new();
        let p = convert_predicate(p, &mut buf_p);
        let mut buf_o = String::new();
        let o = convert_object(o, &mut buf_o);
        if p.is_none() || o.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(None, p, o, None).map(quad_map))
        }
    }
    fn quads_with_pg<'s, TP, TG>(&'s self, p: &'s TP, g: Option<&'s TG>) -> DQuadSource<'s, Self>
    where
        TP: TTerm + ?Sized,
        TG: TTerm + ?Sized,
    {
        let mut buf_p = String::new();
        let p = convert_predicate(p, &mut buf_p);
        let mut buf_g = String::new();
        let g = convert_graph_name(g, &mut buf_g);
        if p.is_none() || g.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(None, p, None, g).map(quad_map))
        }
    }
    fn quads_with_og<'s, TO, TG>(&'s self, o: &'s TO, g: Option<&'s TG>) -> DQuadSource<'s, Self>
    where
        TO: TTerm + ?Sized,
        TG: TTerm + ?Sized,
    {
        let mut buf_o = String::new();
        let o = convert_object(o, &mut buf_o);
        let mut buf_g = String::new();
        let g = convert_graph_name(g, &mut buf_g);
        if o.is_none() || g.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(None, None, o, g).map(quad_map))
        }
    }
    fn quads_with_spo<'s, TS, TP, TO>(
        &'s self,
        s: &'s TS,
        p: &'s TP,
        o: &'s TO,
    ) -> DQuadSource<'s, Self>
    where
        TS: TTerm + ?Sized,
        TP: TTerm + ?Sized,
        TO: TTerm + ?Sized,
    {
        let mut buf_s = String::new();
        let s = convert_subject(s, &mut buf_s);
        let mut buf_p = String::new();
        let p = convert_predicate(p, &mut buf_p);
        let mut buf_o = String::new();
        let o = convert_object(o, &mut buf_o);
        if s.is_none() || p.is_none() || o.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(s, p, o, None).map(quad_map))
        }
    }
    fn quads_with_spg<'s, TS, TP, TG>(
        &'s self,
        s: &'s TS,
        p: &'s TP,
        g: Option<&'s TG>,
    ) -> DQuadSource<'s, Self>
    where
        TS: TTerm + ?Sized,
        TP: TTerm + ?Sized,
        TG: TTerm + ?Sized,
    {
        let mut buf_s = String::new();
        let s = convert_subject(s, &mut buf_s);
        let mut buf_p = String::new();
        let p = convert_predicate(p, &mut buf_p);
        let mut buf_g = String::new();
        let g = convert_graph_name(g, &mut buf_g);
        if s.is_none() || p.is_none() || g.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(s, p, None, g).map(quad_map))
        }
    }
    fn quads_with_sog<'s, TS, TO, TG>(
        &'s self,
        s: &'s TS,
        o: &'s TO,
        g: Option<&'s TG>,
    ) -> DQuadSource<'s, Self>
    where
        TS: TTerm + ?Sized,
        TO: TTerm + ?Sized,
        TG: TTerm + ?Sized,
    {
        let mut buf_s = String::new();
        let s = convert_subject(s, &mut buf_s);
        let mut buf_o = String::new();
        let o = convert_object(o, &mut buf_o);
        let mut buf_g = String::new();
        let g = convert_graph_name(g, &mut buf_g);
        if s.is_none() || o.is_none() || g.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(s, None, o, g).map(quad_map))
        }
    }
    fn quads_with_pog<'s, TP, TO, TG>(
        &'s self,
        p: &'s TP,
        o: &'s TO,
        g: Option<&'s TG>,
    ) -> DQuadSource<'s, Self>
    where
        TP: TTerm + ?Sized,
        TO: TTerm + ?Sized,
        TG: TTerm + ?Sized,
    {
        let mut buf_p = String::new();
        let p = convert_predicate(p, &mut buf_p);
        let mut buf_o = String::new();
        let o = convert_object(o, &mut buf_o);
        let mut buf_g = String::new();
        let g = convert_graph_name(g, &mut buf_g);
        if p.is_none() || o.is_none() || g.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(None, p, o, g).map(quad_map))
        }
    }
    fn quads_with_spog<'s, TS, TP, TO, TG>(
        &'s self,
        s: &'s TS,
        p: &'s TP,
        o: &'s TO,
        g: Option<&'s TG>,
    ) -> DQuadSource<'s, Self>
    where
        TS: TTerm + ?Sized,
        TP: TTerm + ?Sized,
        TO: TTerm + ?Sized,
        TG: TTerm + ?Sized,
    {
        let mut buf_s = String::new();
        let s = convert_subject(s, &mut buf_s);
        let mut buf_p = String::new();
        let p = convert_predicate(p, &mut buf_p);
        let mut buf_o = String::new();
        let o = convert_object(o, &mut buf_o);
        let mut buf_g = String::new();
        let g = convert_graph_name(g, &mut buf_g);
        if s.is_none() || p.is_none() || o.is_none() || g.is_none() {
            Box::new(empty())
        } else {
            Box::new(self.quads_for_pattern(s, p, o, g).map(quad_map))
        }
    }
    fn subjects(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        self.iter()
            .map(|r| r.map(|q| q.subject.into()))
            .collect::<Result<_, _>>()
    }
    fn predicates(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        self.iter()
            .map(|r| r.map(|q| q.predicate.into()))
            .collect::<Result<_, _>>()
    }
    fn objects(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        self.iter()
            .map(|r| r.map(|q| q.object))
            .collect::<Result<_, _>>()
    }
    fn graph_names(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        self.named_graphs()
            .map(|r| r.map(|g| g.into()))
            .collect::<Result<_, _>>()
    }
    fn iris(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        let mut iris = HashSet::new();
        for q in self.iter() {
            let q = q?;
            if let Subject::NamedNode(s) = q.subject {
                iris.insert(s.into());
            }
            iris.insert(q.predicate.into());
            if let Term::NamedNode(o) = q.object {
                iris.insert(o.into());
            }
            if let GraphName::NamedNode(g) = q.graph_name {
                iris.insert(g.into());
            }
        }
        Ok(iris)
    }
    fn bnodes(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        let mut bnodes = HashSet::new();
        for q in self.iter() {
            let q = q?;
            if let Subject::BlankNode(s) = q.subject {
                bnodes.insert(s.into());
            }
            if let Term::BlankNode(o) = q.object {
                bnodes.insert(o.into());
            }
            if let GraphName::BlankNode(g) = q.graph_name {
                bnodes.insert(g.into());
            }
        }
        Ok(bnodes)
    }
    fn literals(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        let mut literals = HashSet::new();
        for q in self.iter() {
            let q = q?;
            if let Term::Literal(o) = q.object {
                literals.insert(o.into());
            }
        }
        Ok(literals)
    }
    fn variables(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        Ok(std::collections::HashSet::new())
    }
}

impl MutableDataset for Store {
    type MutationError = StorageError;
    fn insert<TS, TP, TO, TG>(
        &mut self,
        s: &TS,
        p: &TP,
        o: &TO,
        g: Option<&TG>,
    ) -> MdResult<Self, bool>
    where
        TS: TTerm + ?Sized,
        TP: TTerm + ?Sized,
        TO: TTerm + ?Sized,
        TG: TTerm + ?Sized,
    {
        let mut buf_s = String::new();
        let mut buf_p = String::new();
        let mut buf_o = String::new();
        let mut buf_g = String::new();
        let quadref =
            match convert_quadref(s, p, o, g, &mut buf_s, &mut buf_p, &mut buf_o, &mut buf_g) {
                Some(quad) => quad,
                None => return Ok(false),
            };
        Self::insert(self, quadref).map(|_| true)
    }

    fn remove<TS, TP, TO, TG>(
        &mut self,
        s: &TS,
        p: &TP,
        o: &TO,
        g: Option<&TG>,
    ) -> MdResult<Self, bool>
    where
        TS: TTerm + ?Sized,
        TP: TTerm + ?Sized,
        TO: TTerm + ?Sized,
        TG: TTerm + ?Sized,
    {
        let mut buf_s = String::new();
        let mut buf_p = String::new();
        let mut buf_o = String::new();
        let mut buf_g = String::new();
        let quadref =
            match convert_quadref(s, p, o, g, &mut buf_s, &mut buf_p, &mut buf_o, &mut buf_g) {
                Some(quad) => quad,
                None => return Ok(false),
            };
        Self::remove(self, quadref).map(|_| true)
    }
}

impl CollectibleDataset for Store {
    fn from_quad_source<QS: QuadSource>(quads: QS) -> StreamResult<Self, QS::Error, Self::Error> {
        let mut d = Self::new().map_err(sophia_api::quad::stream::StreamError::SinkError)?;
        d.insert_all(quads)?;
        Ok(d)
    }
}

// helper functions
fn quad_map<'a>(res: Result<Quad, StorageError>) -> Result<StreamedSophiaQuad<'a>, StorageError> {
    res.map(|q| {
        let q: SophiaQuad = q.into();
        StreamedQuad::by_value(q)
    })
}

fn convert_subject<'a, T>(term: &'a T, buffer: &'a mut String) -> Option<SubjectRef<'a>>
where
    T: TTerm + ?Sized + 'a,
{
    match term.kind() {
        TermKind::Iri => Some(convert_iri(term, buffer).into()),
        TermKind::BlankNode => Some(BlankNodeRef::new_unchecked(term.value_raw().0).into()),
        _ => None,
    }
}

fn convert_predicate<'a, T>(term: &'a T, buffer: &'a mut String) -> Option<NamedNodeRef<'a>>
where
    T: TTerm + ?Sized + 'a,
{
    match term.kind() {
        TermKind::Iri => Some(convert_iri(term, buffer)),
        _ => None,
    }
}

fn convert_object<'a, T>(term: &'a T, buffer: &'a mut String) -> Option<TermRef<'a>>
where
    T: TTerm + ?Sized + 'a,
{
    match term.kind() {
        TermKind::Iri => Some(convert_iri(term, buffer).into()),
        TermKind::BlankNode => Some(BlankNodeRef::new_unchecked(term.value_raw().0).into()),
        TermKind::Literal => {
            let value = term.value_raw().0;
            let lit = match term.language() {
                Some(tag) => LiteralRef::new_language_tagged_literal_unchecked(value, tag),
                None => {
                    let (ns, suffix) = term.datatype().unwrap().destruct();
                    let datatype = convert_iri_raw(ns, suffix, buffer);
                    LiteralRef::new_typed_literal(value, datatype)
                }
            };
            Some(lit.into())
        }
        TermKind::Variable => None,
    }
}

fn convert_graph_name<'a, T>(
    graph_name: Option<&'a T>,
    buffer: &'a mut String,
) -> Option<GraphNameRef<'a>>
where
    T: TTerm + ?Sized + 'a,
{
    match graph_name {
        None => Some(GraphNameRef::DefaultGraph),
        Some(term) => match term.kind() {
            TermKind::Iri => Some(convert_iri(term, buffer).into()),
            TermKind::BlankNode => Some(BlankNodeRef::new_unchecked(term.value_raw().0).into()),
            _ => None,
        },
    }
}

fn convert_iri<'a, T>(term: &'a T, buffer: &'a mut String) -> NamedNodeRef<'a>
where
    T: TTerm + ?Sized + 'a,
{
    debug_assert_eq!(term.kind(), TermKind::Iri);
    let raw = term.value_raw();
    convert_iri_raw(raw.0, raw.1, buffer)
}

fn convert_iri_raw<'a>(
    ns: &'a str,
    suffix: Option<&'a str>,
    buffer: &'a mut String,
) -> NamedNodeRef<'a> {
    NamedNodeRef::new_unchecked(match suffix {
        Some(suffix) => {
            buffer.clear();
            buffer.push_str(ns);
            buffer.push_str(suffix);
            buffer
        }
        None => ns,
    })
}

fn convert_quadref<'a, TS, TP, TO, TG>(
    s: &'a TS,
    p: &'a TP,
    o: &'a TO,
    g: Option<&'a TG>,
    buf_s: &'a mut String,
    buf_p: &'a mut String,
    buf_o: &'a mut String,
    buf_g: &'a mut String,
) -> Option<QuadRef<'a>>
where
    TS: TTerm + ?Sized,
    TP: TTerm + ?Sized,
    TO: TTerm + ?Sized,
    TG: TTerm + ?Sized,
{
    let s = match convert_subject(s, buf_s) {
        Some(s) => s,
        None => return None,
    };
    let p = match convert_predicate(p, buf_p) {
        Some(p) => p,
        None => return None,
    };
    let o = match convert_object(o, buf_o) {
        Some(o) => o,
        None => return None,
    };
    let g = match convert_graph_name(g, buf_g) {
        Some(g) => g,
        None => return None,
    };
    Some(QuadRef::new(s, p, o, g))
}

#[cfg(test)]
sophia_api::test_dataset_impl!(test, Store, false, false);
