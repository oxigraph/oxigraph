use crate::model::NamedNode;
use crate::sparql::algebra::Query;
use crate::sparql::error::EvaluationError;
use crate::sparql::http::Client;
use crate::sparql::model::QueryResults;
use crate::sparql::results::QueryResultsFormat;
use crate::sparql::QueryDataset;
use oxiri::Iri;
use sparesults::{QueryResultsParser, ReaderQueryResultsParserOutput};
use spareval::{DefaultServiceHandler, QueryEvaluationError, QuerySolutionIter};
use spargebra::algebra::GraphPattern;
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
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub trait ServiceHandler: Send + Sync {
    /// The service evaluation error.
    type Error: Error + Send + Sync + 'static;

    /// Evaluates a [`Query`] against a given service identified by a [`NamedNode`].
    fn handle(&self, service_name: NamedNode, query: Query) -> Result<QueryResults, Self::Error>;
}

pub struct WrappedDefaultServiceHandler<H: ServiceHandler>(pub H);

impl<H: ServiceHandler> DefaultServiceHandler for WrappedDefaultServiceHandler<H> {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        service_name: NamedNode,
        pattern: GraphPattern,
        base_iri: Option<String>,
    ) -> Result<QuerySolutionIter, Self::Error> {
        let QueryResults::Solutions(solutions) = self
            .0
            .handle(
                service_name,
                Query {
                    inner: spargebra::Query::Select {
                        dataset: None,
                        pattern,
                        base_iri: base_iri
                            .map(Iri::parse)
                            .transpose()
                            .map_err(|e| QueryEvaluationError::Service(Box::new(e)))?,
                    },
                    dataset: QueryDataset::new(),
                },
            )
            .map_err(|e| QueryEvaluationError::Service(Box::new(e)))?
        else {
            return Err(QueryEvaluationError::Service(
                "Only query solutions are supported in services".into(),
            ));
        };
        Ok(solutions.into())
    }
}

pub struct EmptyServiceHandler;

impl DefaultServiceHandler for EmptyServiceHandler {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        service_name: NamedNode,

        _: GraphPattern,
        _: Option<String>,
    ) -> Result<QuerySolutionIter, QueryEvaluationError> {
        Err(QueryEvaluationError::UnsupportedService(service_name))
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

impl DefaultServiceHandler for SimpleServiceHandler {
    type Error = EvaluationError;

    fn handle(
        &self,
        service_name: NamedNode,
        pattern: GraphPattern,
        base_iri: Option<String>,
    ) -> Result<QuerySolutionIter, Self::Error> {
        let (content_type, body) = self
            .client
            .post(
                service_name.as_str(),
                spargebra::Query::Select {
                    dataset: None,
                    pattern,
                    base_iri: base_iri
                        .map(Iri::parse)
                        .transpose()
                        .map_err(|e| EvaluationError::Service(Box::new(e)))?,
                }
                .to_string()
                .into_bytes(),
                "application/sparql-query",
                "application/sparql-results+json, application/sparql-results+xml",
            )
            .map_err(|e| EvaluationError::Service(Box::new(e)))?;
        let format = QueryResultsFormat::from_media_type(&content_type)
            .ok_or_else(|| EvaluationError::UnsupportedContentType(content_type))?;
        let ReaderQueryResultsParserOutput::Solutions(reader) =
            QueryResultsParser::from_format(format).for_reader(body)?
        else {
            return Err(EvaluationError::ServiceDoesNotReturnSolutions);
        };
        Ok(QuerySolutionIter::new(
            reader.variables().into(),
            Box::new(reader.map(|t| t.map_err(|e| QueryEvaluationError::Service(Box::new(e))))),
        ))
    }
}
