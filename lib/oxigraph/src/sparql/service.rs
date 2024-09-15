use crate::model::NamedNode;
use crate::sparql::algebra::Query;
use crate::sparql::error::EvaluationError;
use crate::sparql::http::Client;
use crate::sparql::model::QueryResults;
use crate::sparql::results::QueryResultsFormat;
use std::error::Error;
use std::time::Duration;

/// Handler for [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE.
///
/// Should be given to [`QueryOptions`](super::QueryOptions::with_service_handler())
/// before evaluating a SPARQL query that uses SERVICE calls.
///
/// ```
/// use oxigraph::model::*;
/// use oxigraph::sparql::{EvaluationError, Query, QueryOptions, QueryResults, ServiceHandler};
/// use oxigraph::store::Store;
///
/// struct TestServiceHandler {
///     store: Store,
/// }
///
/// impl ServiceHandler for TestServiceHandler {
///     type Error = EvaluationError;
///
///     fn handle(
///         &self,
///         service_name: NamedNode,
///         query: Query,
///     ) -> Result<QueryResults, Self::Error> {
///         if service_name == "http://example.com/service" {
///             self.store.query(query)
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
/// if let QueryResults::Solutions(mut solutions) = store.query_opt(
///     "SELECT ?s WHERE { SERVICE <http://example.com/service> { ?s ?p ?o } }",
///     QueryOptions::default().with_service_handler(service),
/// )? {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// }
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub trait ServiceHandler: Send + Sync {
    /// The service evaluation error.
    type Error: Error + Send + Sync + 'static;

    /// Evaluates a [`Query`] against a given service identified by a [`NamedNode`].
    fn handle(&self, service_name: NamedNode, query: Query) -> Result<QueryResults, Self::Error>;
}

pub struct EmptyServiceHandler;

impl ServiceHandler for EmptyServiceHandler {
    type Error = EvaluationError;

    fn handle(&self, service_name: NamedNode, _: Query) -> Result<QueryResults, Self::Error> {
        Err(EvaluationError::UnsupportedService(service_name))
    }
}

pub struct ErrorConversionServiceHandler<S: ServiceHandler> {
    handler: S,
}

impl<S: ServiceHandler> ErrorConversionServiceHandler<S> {
    pub fn wrap(handler: S) -> Self {
        Self { handler }
    }
}

impl<S: ServiceHandler> ServiceHandler for ErrorConversionServiceHandler<S> {
    type Error = EvaluationError;

    fn handle(&self, service_name: NamedNode, query: Query) -> Result<QueryResults, Self::Error> {
        self.handler
            .handle(service_name, query)
            .map_err(|e| EvaluationError::Service(Box::new(e)))
    }
}

pub struct SimpleServiceHandler {
    client: Client,
}

impl SimpleServiceHandler {
    pub fn new(http_timeout: Option<Duration>, http_redirection_limit: usize) -> Self {
        Self {
            client: Client::new(http_timeout, http_redirection_limit),
        }
    }
}

impl ServiceHandler for SimpleServiceHandler {
    type Error = EvaluationError;

    fn handle(&self, service_name: NamedNode, query: Query) -> Result<QueryResults, Self::Error> {
        let (content_type, body) = self
            .client
            .post(
                service_name.as_str(),
                query.to_string().into_bytes(),
                "application/sparql-query",
                "application/sparql-results+json, application/sparql-results+xml",
            )
            .map_err(|e| EvaluationError::Service(Box::new(e)))?;
        let format = QueryResultsFormat::from_media_type(&content_type)
            .ok_or_else(|| EvaluationError::UnsupportedContentType(content_type))?;
        Ok(QueryResults::read(body, format)?)
    }
}
