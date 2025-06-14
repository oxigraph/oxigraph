//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.
//!
//! Stores execute SPARQL. See [`Store`](crate::store::Store::query()) for an example.

mod algebra;
mod dataset;
mod error;
#[cfg(feature = "http-client")]
mod http;
mod model;
pub mod results;
mod service;
mod update;

use crate::model::{NamedNode, Term};
pub use crate::sparql::algebra::{Query, QueryDataset, Update};
use crate::sparql::dataset::DatasetView;
pub use crate::sparql::error::EvaluationError;
#[cfg(feature = "http-client")]
use crate::sparql::http::HttpServiceHandler;
pub use crate::sparql::model::{QueryResults, QuerySolution, QuerySolutionIter, QueryTripleIter};
pub use crate::sparql::service::{DefaultServiceHandler, ServiceHandler};
use crate::sparql::service::{WrappedDefaultServiceHandler, WrappedServiceHandler};
pub(crate) use crate::sparql::update::evaluate_update;
use crate::storage::StorageReader;
pub use oxrdf::{Variable, VariableNameParseError};
use spareval::QueryEvaluator;
pub use spareval::QueryExplanation;
pub use spargebra::SparqlSyntaxError;
#[cfg(feature = "http-client")]
use std::time::Duration;

pub(crate) fn evaluate_query(
    reader: StorageReader,
    query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
    options: QueryOptions,
    run_stats: bool,
    substitutions: impl IntoIterator<Item = (Variable, Term)>,
) -> Result<(Result<QueryResults, EvaluationError>, QueryExplanation), EvaluationError> {
    let query = query.try_into().map_err(Into::into)?;
    let dataset = DatasetView::new(reader, &query.dataset);
    let mut evaluator = options.into_evaluator();
    if run_stats {
        evaluator = evaluator.compute_statistics();
    }
    let (results, explanation) =
        evaluator.explain_with_substituted_variables(dataset, &query.inner, substitutions);
    let results = results.map_err(Into::into).map(Into::into);
    Ok((results, explanation))
}

/// Options for SPARQL query evaluation.
///
///
/// If the `"http-client"` optional feature is enabled,
/// a simple HTTP 1.1 client is used to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
///
/// Usage example disabling the federated query support:
/// ```
/// use oxigraph::sparql::QueryOptions;
/// use oxigraph::store::Store;
///
/// let store = Store::new()?;
/// store.query_opt(
///     "SELECT * WHERE { SERVICE <https://query.wikidata.org/sparql> {} }",
///     QueryOptions::default().without_service_handler(),
/// )?;
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone)]
pub struct QueryOptions {
    #[cfg(feature = "http-client")]
    http_timeout: Option<Duration>,
    #[cfg(feature = "http-client")]
    http_redirection_limit: usize,
    #[cfg(feature = "http-client")]
    with_http_default_service_handler: bool,
    inner: QueryEvaluator,
}

impl QueryOptions {
    /// Use a given [`ServiceHandler`] to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
    ///
    /// See [`ServiceHandler`] for an example.
    #[inline]
    #[must_use]
    pub fn with_service_handler(
        mut self,
        service_name: impl Into<NamedNode>,
        handler: impl ServiceHandler + 'static,
    ) -> Self {
        self.inner = self
            .inner
            .with_service_handler(service_name, WrappedServiceHandler(handler));
        self
    }

    /// Use a given [`DefaultServiceHandler`] to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls if no explicit service handler is defined for the service.
    ///
    /// This replaces the default service handler that does HTTP requests to remote endpoints.
    ///
    /// See [`DefaultServiceHandler`] for an example.
    #[inline]
    #[must_use]
    pub fn with_default_service_handler(
        mut self,
        handler: impl DefaultServiceHandler + 'static,
    ) -> Self {
        #[cfg(feature = "http-client")]
        {
            self.with_http_default_service_handler = false;
        }
        self.inner = self
            .inner
            .with_default_service_handler(WrappedDefaultServiceHandler(handler));
        self
    }

    /// Disables the default `SERVICE` call implementation that does HTTP requests to remote endpoints.
    #[cfg(feature = "http-client")]
    #[inline]
    #[must_use]
    pub fn without_default_http_service_handler(mut self) -> Self {
        self.with_http_default_service_handler = false;
        self
    }

    /// Disables the `SERVICE` calls
    #[cfg(feature = "http-client")]
    #[inline]
    #[must_use]
    #[deprecated(
        note = "Use `without_default_http_service_handler` instead",
        since = "0.5.0"
    )]
    pub fn without_service_handler(self) -> Self {
        self.without_default_http_service_handler()
    }

    /// Sets a timeout for HTTP requests done during SPARQL evaluation.
    #[cfg(feature = "http-client")]
    #[inline]
    #[must_use]
    pub fn with_http_timeout(mut self, timeout: Duration) -> Self {
        self.http_timeout = Some(timeout);
        self
    }

    /// Sets an upper bound to the number of HTTP redirections followed per HTTP request done during SPARQL evaluation.
    ///
    /// By default, this value is `0`.
    #[cfg(feature = "http-client")]
    #[inline]
    #[must_use]
    pub fn with_http_redirection_limit(mut self, redirection_limit: usize) -> Self {
        self.http_redirection_limit = redirection_limit;
        self
    }

    #[cfg_attr(not(feature = "http-client"), expect(unused_mut))]
    fn into_evaluator(mut self) -> QueryEvaluator {
        #[cfg(feature = "http-client")]
        if self.with_http_default_service_handler {
            self.inner = self
                .inner
                .with_default_service_handler(HttpServiceHandler::new(
                    self.http_timeout,
                    self.http_redirection_limit,
                ))
        }
        self.inner
    }

    /// Adds a custom SPARQL evaluation function.
    ///
    /// Example with a function serializing terms to N-Triples:
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryOptions, QueryResults};
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    ///
    /// if let QueryResults::Solutions(mut solutions) = store.query_opt(
    ///     "SELECT (<http://www.w3.org/ns/formats/N-Triples>(1) AS ?nt) WHERE {}",
    ///     QueryOptions::default().with_custom_function(
    ///         NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?,
    ///         |args| args.get(0).map(|t| Literal::from(t.to_string()).into()),
    ///     ),
    /// )? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("nt"),
    ///         Some(&Literal::from("\"1\"^^<http://www.w3.org/2001/XMLSchema#integer>").into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    #[must_use]
    pub fn with_custom_function(
        mut self,
        name: NamedNode,
        evaluator: impl Fn(&[Term]) -> Option<Term> + Send + Sync + 'static,
    ) -> Self {
        self.inner = self.inner.with_custom_function(name, evaluator);
        self
    }

    #[doc(hidden)]
    #[inline]
    #[must_use]
    pub fn without_optimizations(mut self) -> Self {
        self.inner = self.inner.without_optimizations();
        self
    }
}

impl Default for QueryOptions {
    fn default() -> Self {
        Self {
            #[cfg(feature = "http-client")]
            http_timeout: None,
            #[cfg(feature = "http-client")]
            http_redirection_limit: 0,
            #[cfg(feature = "http-client")]
            with_http_default_service_handler: true,
            inner: QueryEvaluator::new(),
        }
    }
}

/// Options for SPARQL update evaluation.
#[derive(Clone, Default)]
pub struct UpdateOptions {
    query_options: QueryOptions,
}

impl From<QueryOptions> for UpdateOptions {
    #[inline]
    fn from(query_options: QueryOptions) -> Self {
        Self { query_options }
    }
}
