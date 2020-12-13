use crate::error::{invalid_data_error, invalid_input_error};
use crate::model::NamedNode;
use crate::sparql::algebra::Query;
use crate::sparql::error::EvaluationError;
use crate::sparql::http::Client;
use crate::sparql::model::QueryResults;
use crate::sparql::QueryResultsFormat;
use http::header::{ACCEPT, CONTENT_TYPE, USER_AGENT};
use http::{Method, Request, StatusCode};
use std::error::Error;

/// Handler for [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE.
///
/// Should be given to [`QueryOptions`](super::QueryOptions::with_service_handler())
/// before evaluating a SPARQL query that uses SERVICE calls.
///
/// ```
/// use oxigraph::MemoryStore;
/// use oxigraph::model::*;
/// use oxigraph::sparql::{QueryOptions, QueryResults, ServiceHandler, Query, EvaluationError};
///
/// #[derive(Default)]
/// struct TestServiceHandler {
///     store: MemoryStore
/// }
///
/// impl ServiceHandler for TestServiceHandler {
///     type Error = EvaluationError;
///
///     fn handle(&self,service_name: NamedNode, query: Query) -> Result<QueryResults,EvaluationError> {
///         if service_name == "http://example.com/service" {
///             self.store.query(query)
///         } else {
///             panic!()
///         }
///     }
/// }
///
/// let store = MemoryStore::new();
/// let service = TestServiceHandler::default();
/// let ex = NamedNode::new("http://example.com")?;
/// service.store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
///
/// if let QueryResults::Solutions(mut solutions) = store.query_opt(
///     "SELECT ?s WHERE { SERVICE <http://example.com/service> { ?s ?p ?o } }",
///     QueryOptions::default().with_service_handler(service)
/// )? {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// }
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub trait ServiceHandler {
    type Error: Error + Send + Sync + 'static;

    /// Evaluates a [`Query`] against a given service identified by a [`NamedNode`](crate::model::NamedNode).
    fn handle(&self, service_name: NamedNode, query: Query) -> Result<QueryResults, Self::Error>;
}

pub struct EmptyServiceHandler;

impl ServiceHandler for EmptyServiceHandler {
    type Error = EvaluationError;

    fn handle(&self, _: NamedNode, _: Query) -> Result<QueryResults, EvaluationError> {
        Err(EvaluationError::msg(
            "The SERVICE feature is not implemented",
        ))
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

    fn handle(
        &self,
        service_name: NamedNode,
        query: Query,
    ) -> Result<QueryResults, EvaluationError> {
        self.handler
            .handle(service_name, query)
            .map_err(EvaluationError::wrap)
    }
}

pub struct SimpleServiceHandler {
    client: Client,
}

impl SimpleServiceHandler {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl ServiceHandler for SimpleServiceHandler {
    type Error = EvaluationError;

    fn handle(
        &self,
        service_name: NamedNode,
        query: Query,
    ) -> Result<QueryResults, EvaluationError> {
        let request = Request::builder()
            .method(Method::POST)
            .uri(service_name.as_str())
            .header(CONTENT_TYPE, "application/sparql-query")
            .header(ACCEPT, QueryResultsFormat::Xml.media_type())
            .header(USER_AGENT, concat!("Oxigraph/", env!("CARGO_PKG_VERSION")))
            .body(Some(query.to_string().into_bytes()))
            .map_err(invalid_input_error)?;
        let response = self.client.request(&request)?;
        if response.status() != StatusCode::OK {
            return Err(EvaluationError::msg(format!(
                "HTTP error code {} returned when querying service {}",
                response.status(),
                service_name
            )));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .ok_or_else(|| {
                EvaluationError::msg(format!(
                    "No Content-Type header returned by {}",
                    service_name
                ))
            })?
            .to_str()
            .map_err(invalid_data_error)?;
        let format = QueryResultsFormat::from_media_type(content_type).ok_or_else(|| {
            EvaluationError::msg(format!(
                "Unsupported Content-Type returned by {}: {}",
                service_name, content_type
            ))
        })?;
        Ok(QueryResults::read(response.into_body(), format)?)
    }
}
