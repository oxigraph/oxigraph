use crate::{QueryEvaluationError, QuerySolutionIter};
use oxiri::Iri;
use oxrdf::NamedNode;
use spargebra::algebra::GraphPattern;
use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;

/// Handler for [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICEs.
///
/// Should be given to [`QueryOptions`](super::QueryEvaluator::with_service_handler())
/// before evaluating a SPARQL query that uses SERVICE calls.
///
/// Note that you can also use [`DefaultServiceHandler`] if you need to handle any service and not a specific one.
///
/// ```
/// use oxiri::Iri;
/// use oxrdf::{Dataset, Literal, NamedNode, Variable};
/// use sparesults::QuerySolution;
/// use spareval::{QueryEvaluator, QueryResults, QuerySolutionIter, ServiceHandler};
/// use spargebra::SparqlParser;
/// use spargebra::algebra::GraphPattern;
/// use std::convert::Infallible;
/// use std::iter::once;
/// use std::sync::Arc;
///
/// struct TestServiceHandler {}
///
/// impl ServiceHandler for TestServiceHandler {
///     type Error = Infallible;
///
///     fn handle(
///         &self,
///         _pattern: &GraphPattern,
///         _base_iri: Option<&Iri<String>>,
///     ) -> Result<QuerySolutionIter<'static>, Self::Error> {
///         // Always return a single binding foo -> 1
///         let variables = [Variable::new_unchecked("foo")].into();
///         Ok(QuerySolutionIter::new(
///             Arc::clone(&variables),
///             once(Ok(QuerySolution::from((
///                 variables,
///                 vec![Some(Literal::from(1).into())],
///             )))),
///         ))
///     }
/// }
///
/// let evaluator = QueryEvaluator::default().with_service_handler(
///     NamedNode::new("http://example.com/service")?,
///     TestServiceHandler {},
/// );
/// let query = SparqlParser::new()
///     .parse_query("SELECT ?foo WHERE { SERVICE <http://example.com/service> {} }")?;
/// if let QueryResults::Solutions(mut solutions) = evaluator.execute(&Dataset::new(), &query)? {
///     assert_eq!(
///         solutions.next().unwrap()?.get("foo"),
///         Some(&Literal::from(1).into())
///     );
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub trait ServiceHandler: Send + Sync {
    /// The service evaluation error.
    type Error: Error + Send + Sync + 'static;

    /// Evaluates a [`Query`](spargebra::Query) against the service.
    fn handle(
        &self,
        pattern: &GraphPattern,
        base_iri: Option<&Iri<String>>,
    ) -> Result<QuerySolutionIter<'static>, Self::Error>;
}

/// Default handler for [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICEs.
///
/// Should be given to [`QueryOptions`](super::QueryEvaluator::with_default_service_handler())
/// before evaluating a SPARQL query that uses SERVICE calls.
///
/// Note that you can also use [`ServiceHandler`] if you need to handle a single service and not any service.
///
/// ```
/// use oxiri::Iri;
/// use oxrdf::{Dataset, NamedNode, Variable};
/// use sparesults::QuerySolution;
/// use spareval::{DefaultServiceHandler, QueryEvaluator, QueryResults, QuerySolutionIter};
/// use spargebra::SparqlParser;
/// use spargebra::algebra::GraphPattern;
/// use std::convert::Infallible;
/// use std::iter::once;
/// use std::sync::Arc;
///
/// struct TestServiceHandler {}
///
/// impl DefaultServiceHandler for TestServiceHandler {
///     type Error = Infallible;
///
///     fn handle(
///         &self,
///         service_name: &NamedNode,
///         _pattern: &GraphPattern,
///         _base_iri: Option<&Iri<String>>,
///     ) -> Result<QuerySolutionIter<'static>, Self::Error> {
///         // Always return a single binding name -> name of service
///         let variables = [Variable::new_unchecked("foo")].into();
///         Ok(QuerySolutionIter::new(
///             Arc::clone(&variables),
///             once(Ok(QuerySolution::from((
///                 variables,
///                 vec![Some(service_name.clone().into())],
///             )))),
///         ))
///     }
/// }
///
/// let evaluator = QueryEvaluator::default().with_default_service_handler(TestServiceHandler {});
/// let query = SparqlParser::new()
///     .parse_query("SELECT ?foo WHERE { SERVICE <http://example.com/service> {} }")?;
/// if let QueryResults::Solutions(mut solutions) = evaluator.execute(&Dataset::new(), &query)? {
///     assert_eq!(
///         solutions.next().unwrap()?.get("foo"),
///         Some(&NamedNode::new("http://example.com/service")?.into())
///     );
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub trait DefaultServiceHandler: Send + Sync {
    /// The service evaluation error.
    type Error: Error + Send + Sync + 'static;

    /// Evaluates a [`GraphPattern`] against a given service identified by a [`NamedNode`].
    fn handle(
        &self,
        service_name: &NamedNode,
        pattern: &GraphPattern,
        base_iri: Option<&Iri<String>>,
    ) -> Result<QuerySolutionIter<'static>, Self::Error>;
}

#[derive(Clone, Default)]
pub struct ServiceHandlerRegistry {
    default: Option<Arc<dyn DefaultServiceHandler<Error = QueryEvaluationError>>>,
    handlers: HashMap<NamedNode, Arc<dyn ServiceHandler<Error = QueryEvaluationError>>>,
}

impl ServiceHandlerRegistry {
    pub fn with_handler(
        mut self,
        service_name: NamedNode,
        handler: impl ServiceHandler + 'static,
    ) -> Self {
        self.handlers.insert(
            service_name,
            Arc::new(ErrorConversionServiceHandler(handler)),
        );
        self
    }

    pub fn with_default_handler(mut self, default: impl DefaultServiceHandler + 'static) -> Self {
        self.default = Some(Arc::new(ErrorConversionServiceHandler(default)));
        self
    }

    pub fn has_default_handler(&self) -> bool {
        self.default.is_some()
    }

    pub fn handle(
        &self,
        service_name: &NamedNode,
        pattern: &GraphPattern,
        base_iri: Option<&Iri<String>>,
    ) -> Result<QuerySolutionIter<'static>, QueryEvaluationError> {
        if let Some(handler) = self.handlers.get(service_name) {
            return handler.handle(pattern, base_iri);
        }
        if let Some(default) = &self.default {
            return default.handle(service_name, pattern, base_iri);
        }
        Err(QueryEvaluationError::UnsupportedService(
            service_name.clone(),
        ))
    }
}

struct ErrorConversionServiceHandler<S>(S);

impl<S: ServiceHandler> ServiceHandler for ErrorConversionServiceHandler<S> {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        pattern: &GraphPattern,
        base_iri: Option<&Iri<String>>,
    ) -> Result<QuerySolutionIter<'static>, QueryEvaluationError> {
        self.0.handle(pattern, base_iri).map_err(wrap_service_error)
    }
}

impl<S: DefaultServiceHandler> DefaultServiceHandler for ErrorConversionServiceHandler<S> {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        service_name: &NamedNode,
        pattern: &GraphPattern,
        base_iri: Option<&Iri<String>>,
    ) -> Result<QuerySolutionIter<'static>, QueryEvaluationError> {
        self.0
            .handle(service_name, pattern, base_iri)
            .map_err(wrap_service_error)
    }
}

fn wrap_service_error(error: impl Error + Send + Sync + 'static) -> QueryEvaluationError {
    let error: Box<dyn Error + Send + Sync> = Box::new(error);
    match error.downcast() {
        Ok(error) => *error,
        Err(error) => QueryEvaluationError::Service(error),
    }
}
