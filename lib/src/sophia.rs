//! This crate provides implementation of [Sophia](https://docs.rs/sophia/) traits for the `store` module.

use crate::model::{
    BlankNodeRef, GraphNameRef, LiteralRef, NamedNodeRef, Quad, QuadRef, SubjectRef, Term, TermRef,
};
use crate::sparql::{EvaluationError, QueryResults};
use crate::store::Store;
use sophia_api::dataset::{
    CollectibleDataset, DQuadSource, DResultTermSet, DTerm, Dataset, MDResult, MutableDataset,
};
use sophia_api::quad::stream::{QuadSource, StreamResult};
use sophia_api::quad::streaming_mode::{ByValue, StreamedQuad};
use sophia_api::term::{TTerm, TermKind};
use std::collections::HashSet;
use std::hash::Hash;
use std::io::{Error, ErrorKind};
use std::iter::empty;

type SophiaQuad = ([Term; 3], Option<Term>);
type StreamedSophiaQuad<'a> = StreamedQuad<'a, ByValue<SophiaQuad>>;

impl Dataset for Store {
    type Quad = ByValue<SophiaQuad>;
    type Error = Error;

    fn quads(&self) -> DQuadSource<'_, Self> {
        Box::new(self.iter().map(io_quad_map))
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
            Box::new(self.quads_for_pattern(s, None, None, None).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(None, p, None, None).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(None, None, o, None).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(None, None, None, g).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(s, p, None, None).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(s, None, o, None).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(s, None, None, g).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(None, p, o, None).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(None, p, None, g).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(None, None, o, g).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(s, p, o, None).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(s, p, None, g).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(s, None, o, g).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(None, p, o, g).map(io_quad_map))
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
            Box::new(self.quads_for_pattern(s, p, o, g).map(io_quad_map))
        }
    }
    fn subjects(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        sparql_to_hashset(
            self,
            "SELECT DISTINCT ?s {{?s ?p ?o} UNION { GRAPH ?g {?s ?p ?o}}}",
        )
    }
    fn predicates(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        sparql_to_hashset(
            self,
            "SELECT DISTINCT ?p {{?s ?p ?o} UNION { GRAPH ?g {?s ?p ?o}}}",
        )
    }
    fn objects(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        sparql_to_hashset(
            self,
            "SELECT DISTINCT ?o {{?s ?p ?o} UNION { GRAPH ?g {?s ?p ?o}}}",
        )
    }
    fn graph_names(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        sparql_to_hashset(self, "SELECT DISTINCT ?g {GRAPH ?g {?s ?p ?o}}")
    }
    fn iris(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        sparql_to_hashset(
            self,
            "SELECT DISTINCT ?iri {
                        {?iri ?p ?o} UNION
                        {?s ?iri ?o} UNION
                        {?s ?p ?iri} UNION
                        {GRAPH ?iri {?s ?p ?o}} UNION
                        {GRAPH ?s {?iri ?p ?o}} UNION
                        {GRAPH ?g {?s ?iri ?o}} UNION
                        {GRAPH ?g {?s ?p ?iri}}
                        FILTER isIRI(?iri)
                    }",
        )
    }
    fn bnodes(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        sparql_to_hashset(
            self,
            "SELECT DISTINCT ?bn {
                        {?bn ?p ?o} UNION
                        {?s ?p ?bn} UNION
                        {GRAPH ?bn {?s ?p ?o}} UNION
                        {GRAPH ?s {?bn ?p ?o}} UNION
                        {GRAPH ?g {?s ?p ?bn}}
                        FILTER isBlank(?bn)
                    }",
        )
    }
    fn literals(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        sparql_to_hashset(
            self,
            "SELECT DISTINCT ?lit {
                        {?s ?p ?lit} UNION
                        { GRAPH ?g {?s ?p ?lit}}
                        FILTER isLiteral(?lit)
                    }",
        )
    }
    fn variables(&self) -> DResultTermSet<Self>
    where
        DTerm<Self>: Clone + Eq + Hash,
    {
        Ok(std::collections::HashSet::new())
    }
}

impl MutableDataset for Store {
    type MutationError = Error;
    fn insert<TS, TP, TO, TG>(
        &mut self,
        s: &TS,
        p: &TP,
        o: &TO,
        g: Option<&TG>,
    ) -> MDResult<Self, bool>
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
        Store::insert(self, quadref).map(|_| true)
    }

    fn remove<TS, TP, TO, TG>(
        &mut self,
        s: &TS,
        p: &TP,
        o: &TO,
        g: Option<&TG>,
    ) -> MDResult<Self, bool>
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
        Store::remove(self, quadref).map(|_| true)
    }
}

impl CollectibleDataset for Store {
    fn from_quad_source<QS: QuadSource>(quads: QS) -> StreamResult<Self, QS::Error, Self::Error> {
        let mut d = Store::new().map_err(sophia_api::quad::stream::StreamError::SinkError)?;
        d.insert_all(quads)?;
        Ok(d)
    }
}

// helper functions
fn io_quad_map<'a>(res: Result<Quad, Error>) -> Result<StreamedSophiaQuad<'a>, Error> {
    res.map(|q| {
        let q: SophiaQuad = q.into();
        StreamedQuad::by_value(q)
    })
}

fn io_err_map(err: EvaluationError) -> Error {
    match err {
        EvaluationError::Io(err) => err,
        err => Error::new(ErrorKind::InvalidInput, err),
    }
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
        _ => None,
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

/// Execute a SPARQL query in a store, and return the result as a HashSet,
/// mapping the error (if any) through the given function.
///
/// # Precondition
/// + the query must be a SELECT query with a single selected variable
/// + it must not produce NULL results
fn sparql_to_hashset(store: &Store, sparql: &str) -> Result<HashSet<Term>, Error> {
    if let QueryResults::Solutions(solutions) = store.query(sparql).map_err(io_err_map)? {
        solutions
            .map(|r| r.map(|v| v.get(0).unwrap().clone()))
            .collect::<Result<_, _>>()
            .map_err(io_err_map)
    } else {
        unreachable!()
    }
}

#[cfg(test)]
sophia_api::test_dataset_impl!(test, Store, false, false);
