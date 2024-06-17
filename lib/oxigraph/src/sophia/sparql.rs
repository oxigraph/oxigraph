use std::borrow::Borrow;

use oxrdf::Variable;
use sophia_api::sparql::{Query as SoQuery, SparqlBindings, SparqlDataset, SparqlResult};

use crate::{
    model::Term as OxTerm,
    sparql::{EvaluationError, Query as OxQuery, QueryResults, QuerySolutionIter, QueryTripleIter},
    store::Store,
};

impl SparqlDataset for Store {
    type BindingsTerm = OxTerm;

    type BindingsResult = OxBindings;

    type TriplesResult = QueryTripleIter;

    type SparqlError = EvaluationError;

    type Query = OxQuery;

    fn query<Q>(&self, query: Q) -> Result<SparqlResult<Self>, Self::SparqlError>
    where
        Q: sophia_api::sparql::IntoQuery<Self::Query>,
    {
        Ok(
            match Store::query(self, query.into_query()?.borrow().clone())? {
                QueryResults::Solutions(bindings) => SparqlResult::Bindings(OxBindings(bindings)),
                QueryResults::Boolean(b) => SparqlResult::Boolean(b),
                QueryResults::Graph(triples) => SparqlResult::Triples(triples),
            },
        )
    }
}

/// Wrapper for [`QuerySolutionIter`],
// which makes it implement [`TripleSource`](sophia_api::source::TripleSource).
//
// A wrapper is required, because [`QuerySolutionIter`] already implements `Iterator`,
// with a different type of items.
pub struct OxBindings(QuerySolutionIter);

impl Iterator for OxBindings {
    type Item = Result<Vec<Option<OxTerm>>, EvaluationError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|res| Ok(res?.values().to_vec()))
    }
}

impl SparqlBindings<Store> for OxBindings {
    fn variables(&self) -> Vec<&str> {
        QuerySolutionIter::variables(&self.0)
            .iter()
            .map(Variable::as_str)
            .collect()
    }
}

impl SoQuery for OxQuery {
    type Error = EvaluationError;

    fn parse(query_source: &str) -> Result<Self, Self::Error> {
        OxQuery::parse(query_source, None).map_err(Into::into)
    }
}
