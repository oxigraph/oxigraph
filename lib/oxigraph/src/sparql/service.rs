use crate::model::NamedNode;
use crate::sparql::model::QueryResults;
use oxiri::Iri;
use spareval::{
    DefaultServiceHandler as EvalDefaultServiceHandler, QueryEvaluationError, QuerySolutionIter,
    ServiceHandler as EvalServiceHandler,
};
use spargebra::Query;
use spargebra::algebra::GraphPattern;
use std::error::Error;

/// Catch-all default handler for [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE.
///
/// Should be given to [`QueryOptions`](super::QueryOptions::with_service_handler())
/// before evaluating a SPARQL query that uses SERVICE calls.
///
/// ```
/// use oxigraph::model::*;
/// use oxigraph::sparql::{DefaultServiceHandler, EvaluationError, QueryResults, SparqlEvaluator};
/// use oxigraph::store::Store;
/// use spargebra::Query;
///
/// struct TestServiceHandler {
///     store: Store,
/// }
///
/// impl DefaultServiceHandler for TestServiceHandler {
///     type Error = EvaluationError;
///
///     fn handle(
///         &self,
///         service_name: NamedNode,
///         query: Query,
///     ) -> Result<QueryResults, Self::Error> {
///         if service_name == "http://example.com/service" {
///             SparqlEvaluator::new()
///                 .for_query(query)
///                 .on_store(&self.store)
///                 .execute()
///         } else {
///             panic!()
///         }
///     }
/// }
///
/// let store = Store::new()?;
/// let service = TestServiceHandler {
///     store: Store::new()?,
/// };
/// let ex = NamedNodeRef::new("http://example.com")?;
/// service
///     .store
///     .insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?;
///
/// if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
///     .with_default_service_handler(service)
///     .parse_query("SELECT ?s WHERE { SERVICE <http://example.com/service> { ?s ?p ?o } }")?
///     .on_store(&store)
///     .execute()?
/// {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub trait DefaultServiceHandler: Send + Sync {
    /// The service evaluation error.
    type Error: Error + Send + Sync + 'static;

    /// Evaluates a [`Query`] against a given service identified by a [`NamedNode`].
    fn handle(&self, service_name: NamedNode, query: Query) -> Result<QueryResults, Self::Error>;
}

pub struct WrappedDefaultServiceHandler<H: DefaultServiceHandler>(pub H);

impl<H: DefaultServiceHandler> EvalDefaultServiceHandler for WrappedDefaultServiceHandler<H> {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        service_name: NamedNode,
        pattern: GraphPattern,
        base_iri: Option<String>,
    ) -> Result<QuerySolutionIter, Self::Error> {
        let QueryResults::Solutions(solutions) = self
            .0
            .handle(service_name, query_from_pattern(pattern, base_iri)?)
            .map_err(|e| QueryEvaluationError::Service(Box::new(e)))?
        else {
            return Err(QueryEvaluationError::Service(
                "Only query solutions are supported in services".into(),
            ));
        };
        Ok(solutions.into())
    }
}

/// Handler for a given [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE.
///
/// Should be given to [`QueryOptions`](super::QueryOptions::with_service_handler())
/// before evaluating a SPARQL query that uses SERVICE calls.
///
/// ```
/// use oxigraph::model::*;
/// use oxigraph::sparql::{EvaluationError, QueryResults, ServiceHandler, SparqlEvaluator};
/// use oxigraph::store::Store;
/// use spargebra::Query;
///
/// struct TestServiceHandler {
///     store: Store,
/// }
///
/// impl ServiceHandler for TestServiceHandler {
///     type Error = EvaluationError;
///
///     fn handle(&self, query: Query) -> Result<QueryResults, Self::Error> {
///         SparqlEvaluator::new()
///             .for_query(query)
///             .on_store(&self.store)
///             .execute()
///     }
/// }
///
/// let store = Store::new()?;
/// let service = TestServiceHandler {
///     store: Store::new()?,
/// };
/// let ex = NamedNodeRef::new("http://example.com")?;
/// service
///     .store
///     .insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?;
///
/// if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
///     .with_service_handler(NamedNodeRef::new("http://example.com/service")?, service)
///     .parse_query("SELECT ?s WHERE { SERVICE <http://example.com/service> { ?s ?p ?o } }")?
///     .on_store(&store)
///     .execute()?
/// {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub trait ServiceHandler: Send + Sync {
    /// The service evaluation error.
    type Error: Error + Send + Sync + 'static;

    /// Evaluates a [`Query`] against a given service identified by a [`NamedNode`].
    fn handle(&self, query: Query) -> Result<QueryResults, Self::Error>;
}

pub struct WrappedServiceHandler<H: ServiceHandler>(pub H);

impl<H: ServiceHandler> EvalServiceHandler for WrappedServiceHandler<H> {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        pattern: GraphPattern,
        base_iri: Option<String>,
    ) -> Result<QuerySolutionIter, QueryEvaluationError> {
        let QueryResults::Solutions(solutions) = self
            .0
            .handle(query_from_pattern(pattern, base_iri)?)
            .map_err(|e| QueryEvaluationError::Service(Box::new(e)))?
        else {
            return Err(QueryEvaluationError::Service(
                "Only query solutions are supported in services".into(),
            ));
        };
        Ok(solutions.into())
    }
}

fn query_from_pattern(
    pattern: GraphPattern,
    base_iri: Option<String>,
) -> Result<Query, QueryEvaluationError> {
    Ok(Query::Select {
        dataset: None,
        pattern,
        base_iri: base_iri
            .map(Iri::parse)
            .transpose()
            .map_err(|e| QueryEvaluationError::Service(Box::new(e)))?,
    })
}
