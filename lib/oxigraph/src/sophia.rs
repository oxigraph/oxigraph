use std::iter::{empty, once};

use crate::model::{
    GraphName as OxGraphName, NamedNode, Quad as OxQuad, QuadRef, Subject, Term as OxTerm,
};
use crate::sparql::{EvaluationError, QueryResults};
use crate::store::{StorageError, Store};
use sophia_api::dataset::{CollectibleDataset, MdResult};
use sophia_api::prelude::*;
use sophia_api::quad::Quad as SoQuad;
use sophia_api::source::StreamError::{self, SinkError, SourceError};
use sophia_api::source::StreamResult;
use sophia_api::term::matcher::{GraphNameMatcher, TermMatcher};
use sophia_api::term::Term as SoTerm;

mod adapters;
use adapters::*;

// mod sparql;

#[allow(clippy::use_self)]
impl Dataset for Store {
    type Quad<'x> = OxQuad where Self: 'x;

    type Error = SophiaOxigraphError;

    fn quads(&self) -> sophia_api::dataset::DQuadSource<'_, Self> {
        Box::new(self.iter().map(|res| res.map_err(Into::into)))
    }

    fn quads_matching<'s, S, P, O, G>(
        &'s self,
        sm: S,
        pm: P,
        om: O,
        gm: G,
    ) -> sophia_api::dataset::DQuadSource<'s, Self>
    where
        S: TermMatcher + 's,
        P: TermMatcher + 's,
        O: TermMatcher + 's,
        G: GraphNameMatcher + 's,
    {
        //
        let oxsm = match sm.constant().map(MySubject::from_soterm) {
            None => None,                                // Not a constant matcher, match any OxTerm
            Some(Some(oxs)) => Some(Subject::from(oxs)), // Constant matcher that matches one OxTerm
            Some(None) => return Box::new(empty()), // Constant matcher that matches no OxTerm, so return an empty iterator
        };
        let oxpm = match pm.constant().map(MyNamedNode::from_soterm) {
            None => None,
            Some(Some(oxnn)) => Some(NamedNode::from(oxnn)),
            Some(None) => return Box::new(empty()),
        };
        let oxom = match om.constant().map(MyTerm::from_soterm) {
            None => None,
            Some(Some(oxt)) => Some(OxTerm::from(oxt)),
            Some(None) => return Box::new(empty()),
        };
        let oxgm = match gm.constant().map(MyGraphName::from_soterm) {
            None => None,
            Some(Some(oxng)) => Some(OxGraphName::from(oxng)),
            Some(None) => return Box::new(empty()),
        };
        Box::new(
            self.quads_for_pattern(
                oxsm.as_ref().map(Subject::as_ref),
                oxpm.as_ref().map(NamedNode::as_ref),
                oxom.as_ref().map(OxTerm::as_ref),
                oxgm.as_ref().map(OxGraphName::as_ref),
            )
            .filter(move |tr| {
                if let Ok(t) = tr {
                    (sm.constant().is_some() || sm.matches(&t.s()))
                        && (pm.constant().is_some() || pm.matches(&t.p()))
                        && (om.constant().is_some() || om.matches(&t.o()))
                } else {
                    true
                }
            })
            .map(|res| res.map_err(Into::into)),
        )
    }

    fn contains<TS, TP, TO, TG>(
        &self,
        s: TS,
        p: TP,
        o: TO,
        g: sophia_api::term::GraphName<TG>,
    ) -> sophia_api::dataset::DResult<Self, bool>
    where
        TS: SoTerm,
        TP: SoTerm,
        TO: SoTerm,
        TG: SoTerm,
    {
        let Some(s) = MySubject::from_soterm(&s) else {
            return Ok(false);
        };
        let Some(p) = MyNamedNode::from_soterm(&p) else {
            return Ok(false);
        };
        let Some(o) = MyTerm::from_soterm(&o) else {
            return Ok(false);
        };
        let Some(g) = MyGraphName::from_soterm(g.as_ref()) else {
            return Ok(false);
        };
        let quad = QuadRef {
            subject: s.as_ref(),
            predicate: p.as_ref(),
            object: o.as_ref(),
            graph_name: g.as_ref(),
        };
        Ok(Store::contains(self, quad)?)
    }

    fn subjects(&self) -> sophia_api::dataset::DTermSource<'_, Self> {
        list_terms(
            self,
            "SELECT DISTINCT ?s { { ?s ?p ?o } UNION { GRAPH ?g { ?s ?p ?o }} }",
        )
    }

    fn predicates(&self) -> sophia_api::dataset::DTermSource<'_, Self> {
        list_terms(
            self,
            "SELECT DISTINCT ?p { { ?s ?p ?o } UNION { GRAPH ?g { ?s ?p ?o }} }",
        )
    }

    fn objects(&self) -> sophia_api::dataset::DTermSource<'_, Self> {
        list_terms(
            self,
            "SELECT DISTINCT ?o { { ?s ?p ?o } UNION { GRAPH ?g { ?s ?p ?o }} }",
        )
    }

    fn graph_names(&self) -> sophia_api::dataset::DTermSource<'_, Self> {
        Box::new(
            self.named_graphs()
                .map(|res| res.map(OxTerm::from).map_err(Into::into)),
        )
    }
}

#[allow(clippy::use_self)]
impl MutableDataset for Store {
    type MutationError = SophiaOxigraphError;

    fn insert<TS, TP, TO, TG>(
        &mut self,
        s: TS,
        p: TP,
        o: TO,
        g: sophia_api::term::GraphName<TG>,
    ) -> MdResult<Self, bool>
    where
        TS: SoTerm,
        TP: SoTerm,
        TO: SoTerm,
        TG: SoTerm,
    {
        let Some(s) = MySubject::from_soterm(&s) else {
            return Err(SophiaOxigraphError::Generalized);
        };
        let Some(p) = MyNamedNode::from_soterm(&p) else {
            return Err(SophiaOxigraphError::Generalized);
        };
        let Some(o) = MyTerm::from_soterm(&o) else {
            return Err(SophiaOxigraphError::Generalized);
        };
        let Some(g) = MyGraphName::from_soterm(g.as_ref()) else {
            return Err(SophiaOxigraphError::Generalized);
        };
        Ok(Store::insert(self, &OxQuad::new(s, p, o, g))?)
    }

    fn remove<TS, TP, TO, TG>(
        &mut self,
        s: TS,
        p: TP,
        o: TO,
        g: sophia_api::term::GraphName<TG>,
    ) -> MdResult<Self, bool>
    where
        TS: SoTerm,
        TP: SoTerm,
        TO: SoTerm,
        TG: SoTerm,
    {
        let Some(s) = MySubject::from_soterm(&s) else {
            return Ok(false);
        };
        let Some(p) = MyNamedNode::from_soterm(&p) else {
            return Ok(false);
        };
        let Some(o) = MyTerm::from_soterm(&o) else {
            return Ok(false);
        };
        let Some(g) = MyGraphName::from_soterm(g.as_ref()) else {
            return Ok(false);
        };
        Ok(Store::remove(self, &OxQuad::new(s, p, o, g))?)
    }

    fn insert_all<TS: QuadSource>(
        &mut self,
        src: TS,
    ) -> StreamResult<usize, TS::Error, <Self as MutableDataset>::MutationError> {
        let mut all_quads = src
            .map_quads(|q| {
                let ([s, p, o], g) = q.to_spog();
                let s = MySubject::from_soterm(&s)?;
                let p = MyNamedNode::from_soterm(&p)?;
                let o = MyTerm::from_soterm(&o)?;
                let g = MyGraphName::from_soterm(g.as_ref())?;
                Some(OxQuad::new(s, p, o, g))
            })
            .into_iter()
            .map(|res| match res {
                Ok(Some(q)) => Ok(q),
                Ok(None) => Err(MyStreamError::Sink(SophiaOxigraphError::Generalized)),
                Err(e) => Err(MyStreamError::Source(e)),
            });
        // ensure that there is at least one quad (https://github.com/oxigraph/oxigraph/issues/755)
        let Some(first) = all_quads.next() else {
            return Ok(0);
        };
        let old_len = self.len().map_err(|e| SinkError(e.into()))?;
        self.bulk_loader()
            .load_ok_quads::<_, MyStreamError<TS::Error>>(once(first).chain(all_quads))
            .map_err(StreamError::from)?;
        let new_len = self.len().map_err(|e| SinkError(e.into()))?;
        Ok(new_len - old_len)
    }

    fn remove_matching<S, P, O, G>(
        &mut self,
        ms: S,
        mp: P,
        mo: O,
        mg: G,
    ) -> Result<usize, Self::MutationError>
    where
        S: TermMatcher,
        P: TermMatcher,
        O: TermMatcher,
        G: GraphNameMatcher,
        Self::MutationError: From<Self::Error>,
    {
        let mut count = 0;
        for quad in self.quads_matching(ms, mp, mo, mg) {
            let ([s, p, o], g) = quad?.to_spog();
            let Some(s) = MySubject::from_soterm(&s) else {
                unreachable!()
            };
            let Some(p) = MyNamedNode::from_soterm(&p) else {
                unreachable!()
            };
            let Some(o) = MyTerm::from_soterm(&o) else {
                unreachable!()
            };
            let Some(g) = MyGraphName::from_soterm(g.as_ref()) else {
                unreachable!()
            };
            if Store::remove(
                self,
                QuadRef::new(s.as_ref(), p.as_ref(), o.as_ref(), g.as_ref()),
            )? {
                count += 1;
            }
        }
        Ok(count)
    }

    fn retain_matching<S, P, O, G>(
        &mut self,
        ms: S,
        mp: P,
        mo: O,
        mg: G,
    ) -> Result<(), Self::MutationError>
    where
        S: TermMatcher,
        P: TermMatcher,
        O: TermMatcher,
        G: GraphNameMatcher,
        Self::MutationError: From<Self::Error>,
    {
        let to_keep = self.quads_matching(ms, mp, mo, mg);
        self.clear()?;
        for quad in to_keep {
            let ([s, p, o], g) = quad?.to_spog();
            let Some(s) = MySubject::from_soterm(&s) else {
                unreachable!()
            };
            let Some(p) = MyNamedNode::from_soterm(&p) else {
                unreachable!()
            };
            let Some(o) = MyTerm::from_soterm(&o) else {
                unreachable!()
            };
            let Some(g) = MyGraphName::from_soterm(g.as_ref()) else {
                unreachable!()
            };
            Store::insert(
                self,
                QuadRef::new(s.as_ref(), p.as_ref(), o.as_ref(), g.as_ref()),
            )?;
        }
        Ok(())
    }
}

#[allow(clippy::use_self)]
impl CollectibleDataset for Store {
    fn from_quad_source<TS: QuadSource>(quads: TS) -> StreamResult<Self, TS::Error, Self::Error> {
        let mut store = Store::new().map_err(|e| SinkError(e.into()))?;
        MutableDataset::insert_all(&mut store, quads)?;
        Ok(store)
    }
}

impl sophia_api::dataset::SetDataset for Store {}

// Used internally to implement Dataset::subjects, Dataset::predicates, Dataset::objects and Dataset::graph_names
fn list_terms(
    store: &Store,
    query: &str,
) -> Box<dyn Iterator<Item = Result<OxTerm, SophiaOxigraphError>>> {
    match store.query(query) {
        Ok(QueryResults::Solutions(solutions)) => Box::new(solutions.map(|res| match res {
            Ok(sol) => Ok(sol.get(0).unwrap().clone()),
            Err(EvaluationError::Storage(e)) => Err(e.into()),
            Err(e) => unreachable!("{e:?}"),
        })),

        Ok(_) => unreachable!(),
        Err(EvaluationError::Storage(e)) => Box::new(once(Err(e.into()))),
        Err(e) => unreachable!("{e:?}"),
    }
}

/// Errors raised when using an Oxigraph [`Store`] as a Sophia [`Dataset`].
#[derive(Debug)]
pub enum SophiaOxigraphError {
    /// Error raised by the [`Store`] itself
    Storage(StorageError),
    /// Error caused by the [`Store`]'s inability to store Sophia's generalized RDF
    Generalized,
}

impl std::fmt::Display for SophiaOxigraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Storage(e) => write!(f, "{e}"),
            Self::Generalized => {
                write!(f, "Can not store generalized RDF in Oxigraph")
            }
        }
    }
}

impl std::error::Error for SophiaOxigraphError {}

impl From<StorageError> for SophiaOxigraphError {
    fn from(value: StorageError) -> Self {
        Self::Storage(value)
    }
}

/// A temporary error type required to satisfy trait bounds in Oxigraph's bulk loader
#[derive(Debug)]
enum MyStreamError<E> {
    Source(E),
    Sink(SophiaOxigraphError),
}

impl<E> From<StorageError> for MyStreamError<E> {
    fn from(value: StorageError) -> Self {
        Self::Sink(SophiaOxigraphError::Storage(value))
    }
}

impl<E: std::error::Error> From<MyStreamError<E>> for StreamError<E, SophiaOxigraphError> {
    fn from(value: MyStreamError<E>) -> Self {
        match value {
            MyStreamError::Source(e) => SourceError(e),
            MyStreamError::Sink(e) => SinkError(e),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    sophia_api::test_dataset_impl!(as_dataset, Store, true, false);
}
