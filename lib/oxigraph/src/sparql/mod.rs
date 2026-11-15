//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.
//!
//! The entry point for SPARQL execution is the [`SparqlEvaluator`] type.
//!
//! # Incremental queries
//!
//! An incremental query keeps a handle to its results as the [`Store`] changes. Parse a
//! `SELECT`, `ASK`, or `CONSTRUCT` query, bind it with
//! [`PreparedSparqlQuery::on_store`], and choose one of two views:
//!
//! - [`BoundPreparedSparqlQuery::execute_incremental_results`] retains the current results.
//!   Call [`IncrementalQueryResultsState::results`] for an initial snapshot and again after
//!   changes. A returned snapshot borrows the state, so consume it before calling `results()`
//!   again.
//! - [`BoundPreparedSparqlQuery::execute_incremental_deltas`] returns additions and deletions
//!   since the previous call. Its batches own their data, so they can be processed after the
//!   state is dropped. This view does not retain a complete result set for you.
//!
//! For example, a `SELECT` snapshot tracks both the initial state and later writes:
//!
//! ```
//! use incroxigraph::model::{GraphName, NamedNode, Quad};
//! use incroxigraph::sparql::{IncrementalQueryResults, SparqlEvaluator};
//! use incroxigraph::store::Store;
//!
//! let store = Store::new().unwrap();
//! let mut query = SparqlEvaluator::new()
//!     .parse_query("SELECT ?s WHERE { ?s ?p ?o }").unwrap()
//!     .on_store(&store)
//!     .execute_incremental_results().unwrap();
//!
//! let IncrementalQueryResults::Solutions(rows) = query.results().unwrap() else {
//!     unreachable!()
//! };
//! assert_eq!(rows.count(), 0);
//!
//! let ex = NamedNode::new("http://example.com").unwrap();
//! let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
//! let mut transaction = store.start_transaction().unwrap(); // Grouping updates in a transaction is best for performance.
//! transaction.insert(quad.clone());
//! transaction.commit().unwrap();
//! let IncrementalQueryResults::Solutions(mut rows) = query.results().unwrap() else {
//!     unreachable!()
//! };
//! assert_eq!(rows.next().unwrap().get("s"), Some(&ex.into()));
//! ```
//!
//! Use deltas when maintaining your own view of the results. `SELECT` yields row additions and
//! deletions, `ASK` yields a boolean only when its value changes (after an initial boolean), and
//! `CONSTRUCT` yields triple additions and deletions using graph set semantics:
//!
//! ```
//! use incroxigraph::model::{GraphName, NamedNode, Quad};
//! use incroxigraph::sparql::{Delta, QueryResultsDelta, SparqlEvaluator};
//! use incroxigraph::store::Store;
//!
//! let store = Store::new().unwrap();
//! let mut query = SparqlEvaluator::new()
//!     .parse_query("SELECT ?s WHERE { ?s ?p ?o }").unwrap()
//!     .on_store(&store)
//!     .execute_incremental_deltas().unwrap();
//! let QueryResultsDelta::Solutions(initial) = query.deltas().unwrap() else { unreachable!() };
//! assert_eq!(initial.count(), 0);
//!
//! let ex = NamedNode::new("http://example.com").unwrap();
//! let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), GraphName::DefaultGraph);
//! let mut transaction = store.start_transaction().unwrap();
//! // Grouping updates in a transaction is best for performance.
//! transaction.insert(quad.clone());
//! transaction.commit().unwrap();
//! let QueryResultsDelta::Solutions(changes) = query.deltas().unwrap() else { unreachable!() };
//! for change in changes {
//!     match change.unwrap() {
//!         Delta::Addition(solution) => assert!(solution.get("s").is_some()),
//!         Delta::Deletion(_) => unreachable!(),
//!     }
//! }
//! ```
//!
//! `ASK` and `CONSTRUCT` use the same entry point, with different delta variants:
//!
//! ```
//! use incroxigraph::model::{GraphName, NamedNode, Quad};
//! use incroxigraph::sparql::{Delta, QueryResultsDelta, SparqlEvaluator};
//! use incroxigraph::store::Store;
//!
//! let store = Store::new().unwrap();
//! let mut ask = SparqlEvaluator::new()
//!     .parse_query("ASK { ?s <urn:p> ?o }").unwrap()
//!     .on_store(&store)
//!     .execute_incremental_deltas().unwrap();
//! let mut construct = SparqlEvaluator::new()
//!     .parse_query("CONSTRUCT { ?s <urn:q> ?o } WHERE { ?s <urn:p> ?o }").unwrap()
//!     .on_store(&store)
//!     .execute_incremental_deltas().unwrap();
//! let QueryResultsDelta::Boolean(initial) = ask.deltas().unwrap() else { unreachable!() };
//! assert_eq!(initial.collect::<Result<Vec<_>, _>>().unwrap(), [false]);
//!
//! let ex = NamedNode::new("http://example.com").unwrap();
//! let quad = Quad::new(
//!     ex.clone(),
//!     NamedNode::new("urn:p").unwrap(),
//!     ex.clone(),
//!     GraphName::DefaultGraph,
//! );
//! let mut transaction = store.start_transaction().unwrap();
//! // Grouping updates in a transaction is best for performance.
//! transaction.insert(quad.clone());
//! transaction.commit().unwrap();
//! let QueryResultsDelta::Boolean(values) = ask.deltas().unwrap() else { unreachable!() };
//! assert_eq!(values.collect::<Result<Vec<_>, _>>().unwrap(), [true]);
//! let QueryResultsDelta::Graph(triples) = construct.deltas().unwrap() else { unreachable!() };
//! assert!(matches!(triples.collect::<Result<Vec<_>, _>>().unwrap().as_slice(), [Delta::Addition(_)]));
//! ```
//!
//! To wait instead of polling, call [`IncrementalQueryResultsState::iter_results`] or
//! [`IncrementalQueryDeltasState::iter_deltas`] and await `next()`. Snapshot iteration yields
//! its initial result, even when empty; delta iteration waits for a non-empty `SELECT` or
//! `CONSTRUCT` batch. `ASK` deltas yield the initial boolean. A call drains all changes ready
//! at that time and may combine several commits; it does not report every intermediate value.
//! An asynchronous consumer can process changes as they arrive:
//!
//! ```no_run
//! use incroxigraph::sparql::{QueryResultsDelta, SparqlEvaluator};
//! use incroxigraph::store::Store;
//!
//! async fn watch(store: &Store) {
//!     let mut query = SparqlEvaluator::new()
//!         .parse_query("SELECT ?s WHERE { ?s <urn:p> ?o }").unwrap()
//!         .on_store(store)
//!         .execute_incremental_deltas().unwrap();
//!     let mut batches = query.iter_deltas();
//!     while let Some(batch) = batches.next().await {
//!         if let QueryResultsDelta::Solutions(rows) = batch.unwrap() {
//!             for row in rows {
//!                 let change = row.unwrap();
//!                 // Update an application-owned cache using change.
//!                 let _ = change;
//!             }
//!         }
//!     }
//! }
//! # let _ = watch;
//! ```
//!
//! Store-bound queries observe committed changes, while a query bound to a transaction sees
//! its isolated snapshot and then finishes. The handles are not `Send`, so create and drive
//! them on the thread that owns them.
//!
//! If `results()`, `deltas()`, or an asynchronous `next()` returns an error, stop and drop the
//! handle. The current batch may have been partly consumed, and a later call has no recovery
//! guarantee. Cancellation does not automatically end the iterator: polling again can return
//! another cancellation error immediately.

mod dataset;
mod error;
#[cfg(feature = "http-client")]
mod http;
pub mod results;
mod update;

use crate::model::{IriParseError, NamedNode, Term};
pub use crate::model::{Variable, VariableNameParseError};
use crate::sparql::dataset::DatasetView;
pub use crate::sparql::error::UpdateEvaluationError;
#[cfg(feature = "http-client")]
use crate::sparql::http::HttpServiceHandler;
pub use crate::sparql::update::{BoundPreparedSparqlUpdate, PreparedSparqlUpdate};
use crate::store::{Store, Transaction};
pub use spareval::{
    AggregateFunctionAccumulator, CancellationToken, DefaultServiceHandler, Delta, DeltaKind,
    IncrementalDriverState, IncrementalGraphResults, IncrementalQueryDeltasIter,
    IncrementalQueryDeltasState, IncrementalQueryResults, IncrementalQueryResultsIter,
    IncrementalQueryResultsState, IncrementalSelectDriver, IncrementalSelectResults,
    QueryBooleanDeltaIter, QueryDatasetSpecification, QueryEvaluationError, QueryExplanation,
    QueryResults, QueryResultsDelta, QuerySolution, QuerySolutionDeltaIter, QuerySolutionIter,
    QueryTripleDeltaIter, QueryTripleIter, ServiceHandler,
};
use spareval::{IncrementalQueryableDataset, QueryEvaluator, QueryableDataset};
use spargebra::SparqlParser;
pub use spargebra::{Query, SparqlSyntaxError, Update};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::mem::take;
#[cfg(feature = "http-client")]
use std::time::Duration;

pub type QueryDataset = QueryDatasetSpecification;

/// State for the deltas of an incremental query executed against a [`Store`].
pub type StoreIncrementalQueryDeltasState =
    IncrementalQueryDeltasState<'static, DatasetView<'static>>;

/// State for complete incremental query results executed against a [`Store`].
pub type StoreIncrementalQueryResultsState =
    IncrementalQueryResultsState<'static, DatasetView<'static>>;

/// SPARQL evaluator.
///
/// It supports [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) and [SPARQL 1.1 update](https://www.w3.org/TR/sparql11-update/).
///
/// If the `"http-client"` optional feature is enabled,
/// a simple HTTP 1.1 client is used to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
///
/// Usage example disabling the federated query support:
/// ```
/// use incroxigraph::model::NamedNode;
/// use incroxigraph::sparql::SparqlEvaluator;
/// use incroxigraph::store::Store;
///
/// SparqlEvaluator::new()
///     .with_custom_function(NamedNode::new("http://example.com/identity")?, |args| {
///         args.get(0).cloned()
///     })
///     .parse_query("SELECT (<http://example.com/identity>('foo') AS ?r) WHERE {}")?
///     .on_store(&Store::new()?)
///     .execute()?;
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone)]
#[must_use]
pub struct SparqlEvaluator {
    #[cfg(feature = "http-client")]
    http_timeout: Option<Duration>,
    #[cfg(feature = "http-client")]
    http_redirection_limit: usize,
    #[cfg(feature = "http-client")]
    with_http_default_service_handler: bool,
    parser: SparqlParser,
    inner: QueryEvaluator,
}

impl SparqlEvaluator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Provides an IRI that could be used to resolve the operation relative IRIs.
    ///
    /// ```
    /// use incroxigraph::model::*;
    /// use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use incroxigraph::store::Store;
    ///
    /// if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
    ///     .with_base_iri("http://example.com/")?
    ///     .parse_query("SELECT (<> AS ?r) WHERE {}")?
    ///     .on_store(&Store::new()?)
    ///     .execute()?
    /// {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("r"),
    ///         Some(&NamedNode::new("http://example.com/")?.into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(mut self, base_iri: &str) -> Result<Self, IriParseError> {
        self.parser = self.parser.with_base_iri(base_iri)?;
        Ok(self)
    }

    /// Set a default IRI prefix used during parsing.
    ///
    /// ```
    /// use incroxigraph::model::*;
    /// use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use incroxigraph::store::Store;
    ///
    /// if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
    ///     .with_prefix("ex", "http://example.com/")?
    ///     .parse_query("SELECT (ex: AS ?r) WHERE {}")?
    ///     .on_store(&Store::new()?)
    ///     .execute()?
    /// {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("r"),
    ///         Some(&NamedNode::new("http://example.com/")?.into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: &str,
        prefix_iri: &str,
    ) -> Result<Self, IriParseError> {
        self.parser = self.parser.with_prefix(prefix_name, prefix_iri)?;
        Ok(self)
    }

    /// Use a given [`ServiceHandler`] to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
    ///
    /// See [`ServiceHandler`] for an example.
    #[inline]
    pub fn with_service_handler(
        mut self,
        service_name: impl Into<NamedNode>,
        handler: impl ServiceHandler + 'static,
    ) -> Self {
        self.inner = self.inner.with_service_handler(service_name, handler);
        self
    }

    /// Use a given [`DefaultServiceHandler`] to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls if no explicit service handler is defined for the service.
    ///
    /// This replaces the default service handler that does HTTP requests to remote endpoints.
    ///
    /// See [`DefaultServiceHandler`] for an example.
    #[inline]
    pub fn with_default_service_handler(
        mut self,
        handler: impl DefaultServiceHandler + 'static,
    ) -> Self {
        #[cfg(feature = "http-client")]
        {
            self.with_http_default_service_handler = false;
        }
        self.inner = self.inner.with_default_service_handler(handler);
        self
    }

    /// Disables the default `SERVICE` call implementation that does HTTP requests to remote endpoints.
    #[cfg(feature = "http-client")]
    #[inline]
    pub fn without_default_http_service_handler(mut self) -> Self {
        self.with_http_default_service_handler = false;
        self
    }

    /// Sets a timeout for HTTP requests done during SPARQL evaluation.
    #[cfg(feature = "http-client")]
    #[inline]
    pub fn with_http_timeout(mut self, timeout: Duration) -> Self {
        self.http_timeout = Some(timeout);
        self
    }

    /// Sets an upper bound to the number of HTTP redirections followed per HTTP request done during SPARQL evaluation.
    ///
    /// By default, this value is `0`.
    #[cfg(feature = "http-client")]
    #[inline]
    pub fn with_http_redirection_limit(mut self, redirection_limit: usize) -> Self {
        self.http_redirection_limit = redirection_limit;
        self
    }

    /// Adds a custom SPARQL evaluation function.
    ///
    /// Example with a function serializing terms to N-Triples:
    /// ```
    /// use incroxigraph::model::*;
    /// use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use incroxigraph::store::Store;
    ///
    /// if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
    ///     .with_custom_function(
    ///         NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?,
    ///         |args| args.get(0).map(|t| Literal::from(t.to_string()).into()),
    ///     )
    ///     .parse_query("SELECT (<http://www.w3.org/ns/formats/N-Triples>(1) AS ?nt) WHERE {}")?
    ///     .on_store(&Store::new()?)
    ///     .execute()?
    /// {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("nt"),
    ///         Some(&Literal::from("\"1\"^^<http://www.w3.org/2001/XMLSchema#integer>").into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_custom_function(
        mut self,
        name: NamedNode,
        evaluator: impl Fn(&[Term]) -> Option<Term> + Send + Sync + 'static,
    ) -> Self {
        self.inner = self.inner.with_custom_function(name, evaluator);
        self
    }

    /// Returns the list of custom functions currently registered in the evaluator.
    ///
    /// ```
    /// use incroxigraph::model::*;
    /// use incroxigraph::sparql::SparqlEvaluator;
    ///
    /// let evaluator = SparqlEvaluator::new().with_custom_function(
    ///     NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?,
    ///     |args| args.get(0).map(|t| Literal::from(t.to_string()).into()),
    /// );
    /// assert!(
    ///     evaluator
    ///         .custom_functions()
    ///         .collect::<Vec<_>>()
    ///         .contains(&&NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?)
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn custom_functions(&self) -> impl Iterator<Item = &NamedNode> {
        self.inner.custom_functions()
    }

    /// Adds a custom SPARQL evaluation aggregate function.
    ///
    /// Example with a function doing concatenation:
    /// ```
    /// use incroxigraph::model::{Literal, NamedNode, Term};
    /// use incroxigraph::sparql::{AggregateFunctionAccumulator, QueryResults, SparqlEvaluator};
    /// use incroxigraph::store::Store;
    /// use std::mem::take;
    ///
    /// struct ConcatAccumulator {
    ///     value: String,
    /// }
    ///
    /// impl AggregateFunctionAccumulator for ConcatAccumulator {
    ///     fn accumulate(&mut self, element: Term) {
    ///         if let Term::Literal(v) = element {
    ///             if !self.value.is_empty() {
    ///                 self.value.push(' ');
    ///             }
    ///             self.value.push_str(v.value());
    ///         }
    ///     }
    ///
    ///     fn finish(&mut self) -> Option<Term> {
    ///         Some(Literal::new_simple_literal(take(&mut self.value)).into())
    ///     }
    /// }
    ///
    /// if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
    ///     .with_custom_aggregate_function(NamedNode::new("http://example.com/concat")?, || {
    ///         Box::new(ConcatAccumulator {
    ///             value: String::new(),
    ///         })
    ///     })
    ///     .parse_query(
    ///         "SELECT (<http://example.com/concat>(?v) AS ?r) WHERE { VALUES ?v { 1 2 3 } }",
    ///     )?
    ///     .on_store(&Store::new()?)
    ///     .execute()?
    /// {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("r"),
    ///         Some(&Literal::new_simple_literal("1 2 3").into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_custom_aggregate_function(
        mut self,
        name: NamedNode,
        evaluator: impl Fn() -> Box<dyn AggregateFunctionAccumulator + Send + Sync>
        + Send
        + Sync
        + 'static,
    ) -> Self {
        self.parser = self.parser.with_custom_aggregate_function(name.clone());
        self.inner = self.inner.with_custom_aggregate_function(name, evaluator);
        self
    }

    /// Returns the list of custom aggregate functions currently registered in the evaluator.
    pub fn custom_aggregate_functions(&self) -> impl Iterator<Item = &NamedNode> {
        self.inner.custom_aggregate_functions()
    }

    #[doc(hidden)]
    #[inline]
    pub fn without_optimizations(mut self) -> Self {
        self.inner = self.inner.without_optimizations();
        self
    }

    /// Inject a cancellation token to the SPARQL evaluation.
    ///
    /// Might be used to abort a query cleanly.
    ///
    /// ```
    /// use incroxigraph::model::*;
    /// use incroxigraph::sparql::{
    ///     CancellationToken, QueryEvaluationError, QueryResults, SparqlEvaluator,
    /// };
    /// use incroxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    /// store.insert(Quad::new(
    ///     NamedNode::new("http://example.com/s")?,
    ///     NamedNode::new("http://example.com/p")?,
    ///     NamedNode::new("http://example.com/o")?,
    ///     GraphName::DefaultGraph,
    /// ))?;
    /// let cancellation_token = CancellationToken::new();
    /// if let QueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
    ///     .with_cancellation_token(cancellation_token.clone())
    ///     .parse_query("SELECT * WHERE { ?s ?p ?o }")?
    ///     .on_store(&store)
    ///     .execute()?
    /// {
    ///     cancellation_token.cancel(); // We cancel
    ///     assert!(matches!(
    ///         solutions.next().unwrap().unwrap_err(),
    ///         QueryEvaluationError::Cancelled
    ///     ));
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn with_cancellation_token(mut self, cancellation_token: CancellationToken) -> Self {
        self.inner = self.inner.with_cancellation_token(cancellation_token);
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

    /// Parse a query and returns a [`PreparedSparqlQuery`] for the current evaluator.
    ///
    /// Usage example:
    /// ```
    /// use incroxigraph::model::*;
    /// use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use incroxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    /// let ex = NamedNode::new("http://example.com")?;
    /// store.insert(Quad::new(
    ///     ex.clone(),
    ///     ex.clone(),
    ///     ex.clone(),
    ///     GraphName::DefaultGraph,
    /// ))?;
    ///
    /// let prepared_query = SparqlEvaluator::new().parse_query("SELECT ?s WHERE { ?s ?p ?o }")?;
    ///
    /// if let QueryResults::Solutions(mut solutions) = prepared_query.on_store(&store).execute()? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("s"),
    ///         Some(&ex.clone().into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn parse_query(
        mut self,
        query: &(impl AsRef<str> + ?Sized),
    ) -> Result<PreparedSparqlQuery, SparqlSyntaxError> {
        let query = take(&mut self.parser).parse_query(query.as_ref())?;
        Ok(self.for_query(query))
    }

    /// Returns a [`PreparedSparqlQuery`] for the current evaluator and SPARQL query.
    ///
    /// Usage example:
    /// ```
    /// use incroxigraph::model::*;
    /// use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use incroxigraph::store::Store;
    /// use spargebra::SparqlParser;
    ///
    /// let store = Store::new()?;
    /// let ex = NamedNode::new("http://example.com")?;
    /// store.insert(Quad::new(
    ///     ex.clone(),
    ///     ex.clone(),
    ///     ex.clone(),
    ///     GraphName::DefaultGraph,
    /// ))?;
    ///
    /// let query = SparqlParser::new().parse_query("SELECT ?s WHERE { ?s ?p ?o }")?;
    ///
    /// let prepared_query = SparqlEvaluator::new().for_query(query);
    ///
    /// if let QueryResults::Solutions(mut solutions) = prepared_query.on_store(&store).execute()? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("s"),
    ///         Some(&ex.clone().into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_query(self, query: Query) -> PreparedSparqlQuery {
        let dataset = query.dataset().cloned().map(Into::into).unwrap_or_default();
        PreparedSparqlQuery {
            dataset,
            query,
            evaluator: self.into_evaluator(),
            substitutions: HashMap::new(),
        }
    }

    /// Parse an update and returns a [`PreparedSparqlUpdate`] for the current evaluator.
    ///
    /// Usage example:
    /// ```
    /// use incroxigraph::sparql::SparqlEvaluator;
    /// use incroxigraph::store::Store;
    ///
    /// SparqlEvaluator::new()
    ///     .parse_update(
    ///         "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
    ///     )?
    ///     .on_store(&Store::new()?)
    ///     .execute()?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn parse_update(
        mut self,
        query: &(impl AsRef<str> + ?Sized),
    ) -> Result<PreparedSparqlUpdate, SparqlSyntaxError> {
        let update = take(&mut self.parser).parse_update(query.as_ref())?;
        Ok(self.for_update(update))
    }

    /// Returns a [`PreparedSparqlUpdate`] for the current evaluator and SPARQL update.
    ///
    /// Usage example:
    /// ```
    /// use incroxigraph::sparql::SparqlEvaluator;
    /// use incroxigraph::store::Store;
    /// use spargebra::SparqlParser;
    ///
    /// let update = SparqlParser::new().parse_update(
    ///     "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
    /// )?;
    /// SparqlEvaluator::new()
    ///     .for_update(update)
    ///     .on_store(&Store::new()?)
    ///     .execute()?;
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    pub fn for_update(self, update: Update) -> PreparedSparqlUpdate {
        #[cfg(feature = "http-client")]
        let http_timeout = self.http_timeout;
        #[cfg(feature = "http-client")]
        let http_redirection_limit = self.http_redirection_limit;
        PreparedSparqlUpdate::new(
            self.into_evaluator(),
            update,
            #[cfg(feature = "http-client")]
            http_timeout,
            #[cfg(feature = "http-client")]
            http_redirection_limit,
        )
    }
}

impl Default for SparqlEvaluator {
    fn default() -> Self {
        Self {
            #[cfg(feature = "http-client")]
            http_timeout: None,
            #[cfg(feature = "http-client")]
            http_redirection_limit: 0,
            #[cfg(feature = "http-client")]
            with_http_default_service_handler: true,
            parser: SparqlParser::new(),
            inner: QueryEvaluator::new(),
        }
    }
}

/// A prepared SPARQL query.
///
/// Allows customizing things like the evaluation dataset and substituting variables.
///
/// Usage example:
/// ```
/// use incroxigraph::model::{Literal, Variable};
/// use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
/// use incroxigraph::store::Store;
///
/// let prepared_query = SparqlEvaluator::new()
///     .parse_query("SELECT ?v WHERE {}")?
///     .substitute_variable(Variable::new("v")?, Literal::from(1));
///
/// if let QueryResults::Solutions(mut solutions) =
///     prepared_query.on_store(&Store::new()?).execute()?
/// {
///     assert_eq!(
///         solutions.next().unwrap()?.get("v"),
///         Some(&Literal::from(1).into())
///     );
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone)]
#[must_use]
pub struct PreparedSparqlQuery {
    evaluator: QueryEvaluator,
    query: Query,
    dataset: QueryDatasetSpecification,
    substitutions: HashMap<Variable, Term>,
}

impl PreparedSparqlQuery {
    /// Substitute a variable with a given RDF term in the SPARQL query.
    ///
    /// The variable must be part of the `SELECT` clause to be substituted.
    ///
    /// Usage example:
    /// ```
    /// use incroxigraph::model::{Literal, Variable};
    /// use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use incroxigraph::store::Store;
    ///
    /// let prepared_query = SparqlEvaluator::new()
    ///     .parse_query("SELECT ?x ?v WHERE { BIND(?v+1 AS ?x) }")?
    ///     .substitute_variable(Variable::new("v")?, Literal::from(1));
    ///
    /// if let QueryResults::Solutions(mut solutions) =
    ///     prepared_query.on_store(&Store::new()?).execute()?
    /// {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("x"),
    ///         Some(&Literal::from(2).into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn substitute_variable(
        mut self,
        variable: impl Into<Variable>,
        term: impl Into<Term>,
    ) -> Self {
        self.substitutions.insert(variable.into(), term.into());
        self
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) of this prepared query.
    #[inline]
    pub fn dataset(&self) -> &QueryDataset {
        &self.dataset
    }

    /// Returns [the query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset) of this prepared query.
    #[inline]
    pub fn dataset_mut(&mut self) -> &mut QueryDataset {
        &mut self.dataset
    }

    /// Bind the prepared query to the [`Store`] it should be evaluated on.
    pub fn on_store(self, store: &Store) -> BoundPreparedSparqlQuery<'static> {
        let reader = store.storage().snapshot();
        let queryable_dataset = DatasetView::new(reader);
        self.on_queryable_dataset(queryable_dataset)
    }

    /// Bind the prepared query to the [`Transaction`] it should be evaluated on.
    pub fn on_transaction<'b>(
        self,
        transaction: &'b Transaction<'_>,
    ) -> BoundPreparedSparqlQuery<'b> {
        let reader = transaction.inner().reader();
        let dataset = DatasetView::new(reader);
        self.on_queryable_dataset(dataset)
    }

    /// Bind the prepared query to the [`QueryableDataset`] it should be evaluated on.
    pub fn on_queryable_dataset<'a, D: QueryableDataset<'a>>(
        self,
        queryable_dataset: D,
    ) -> BoundPreparedSparqlQuery<'a, D> {
        BoundPreparedSparqlQuery {
            evaluator: self.evaluator,
            query: self.query,
            queryable_dataset,
            substitutions: self.substitutions,
            dataset: self.dataset,
            marker: PhantomData,
        }
    }
}

/// A prepared SPARQL query bound to a storage, ready to be executed.
///
/// Usage example:
/// ```
/// use incroxigraph::model::{Literal, Variable};
/// use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
/// use incroxigraph::store::Store;
///
/// let prepared_query = SparqlEvaluator::new()
///     .parse_query("SELECT ?v WHERE {}")?
///     .substitute_variable(Variable::new("v")?, Literal::from(1));
///
/// if let QueryResults::Solutions(mut solutions) =
///     prepared_query.on_store(&Store::new()?).execute()?
/// {
///     assert_eq!(
///         solutions.next().unwrap()?.get("v"),
///         Some(&Literal::from(1).into())
///     );
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct BoundPreparedSparqlQuery<'a, D: QueryableDataset<'a> = DatasetView<'a>> {
    evaluator: QueryEvaluator,
    query: Query,
    queryable_dataset: D,
    substitutions: HashMap<Variable, Term>,
    dataset: QueryDatasetSpecification,
    marker: PhantomData<&'a ()>,
}

impl<'a, D: QueryableDataset<'a>> BoundPreparedSparqlQuery<'a, D> {
    /// Substitute a variable with a given RDF term in the SPARQL query.
    ///
    /// The variable must be part of the `SELECT` clause to be substituted.
    ///
    /// Usage example:
    /// ```
    /// use incroxigraph::model::{Literal, Variable};
    /// use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use incroxigraph::store::Store;
    ///
    /// let prepared_query = SparqlEvaluator::new()
    ///     .parse_query("SELECT ?x ?v WHERE { BIND(?v+1 AS ?x)}")?
    ///     .on_store(&Store::new()?)
    ///     .substitute_variable(Variable::new("v")?, Literal::from(1));
    ///
    /// if let QueryResults::Solutions(mut solutions) = prepared_query.execute()? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("x"),
    ///         Some(&Literal::from(2).into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn substitute_variable(
        mut self,
        variable: impl Into<Variable>,
        term: impl Into<Term>,
    ) -> Self {
        self.substitutions.insert(variable.into(), term.into());
        self
    }

    /// Evaluate the query against the given store.
    pub fn execute(self) -> Result<QueryResults<'a>, QueryEvaluationError> {
        let mut prepared = self.evaluator.prepare(&self.query);
        for (variable, term) in self.substitutions {
            prepared = prepared.substitute_variable(variable, term);
        }
        *prepared.dataset_mut() = self.dataset;
        prepared.execute(self.queryable_dataset)
    }

    /// Compute statistics during evaluation and fills them in the explanation tree.
    pub fn compute_statistics(mut self) -> Self {
        self.evaluator = self.evaluator.compute_statistics();
        self
    }

    /// Executes a [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) with some options and
    /// returns a query explanation with some statistics (if enabled with the [`compute_statistics`](Self::compute_statistics) option).
    ///
    /// <div class="warning">If you want to compute statistics, you need to exhaust the results iterator before having a look at them.</div>
    ///
    /// Usage example serializing the explanation with statistics in JSON:
    /// ```
    /// use incroxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use incroxigraph::store::Store;
    ///
    /// if let (Ok(QueryResults::Solutions(solutions)), explanation) = SparqlEvaluator::new()
    ///     .parse_query("SELECT ?s WHERE { VALUES ?s { 1 2 3 } }")?
    ///     .on_store(&Store::new()?)
    ///     .explain()
    /// {
    ///     // We make sure to have read all the solutions
    ///     for _ in solutions {}
    ///     let mut buf = Vec::new();
    ///     explanation.write_in_json(&mut buf)?;
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn explain(
        self,
    ) -> (
        Result<QueryResults<'a>, QueryEvaluationError>,
        QueryExplanation,
    ) {
        let mut prepared = self.evaluator.prepare(&self.query);
        for (variable, term) in self.substitutions {
            prepared = prepared.substitute_variable(variable, term);
        }
        *prepared.dataset_mut() = self.dataset;
        prepared.explain(self.queryable_dataset)
    }
}

impl<'a, D: IncrementalQueryableDataset<'a>> BoundPreparedSparqlQuery<'a, D> {
    /// Evaluate the query incrementally and return state for complete result snapshots.
    pub fn execute_incremental_results(
        self,
    ) -> Result<IncrementalQueryResultsState<'a, D>, QueryEvaluationError> {
        let mut prepared = self.evaluator.prepare(&self.query);
        for (variable, term) in self.substitutions {
            prepared = prepared.substitute_variable(variable, term);
        }
        *prepared.dataset_mut() = self.dataset;
        prepared.execute_incremental_results(self.queryable_dataset)
    }

    /// Evaluate the query incrementally and return state for result deltas.
    pub fn execute_incremental_deltas(
        self,
    ) -> Result<IncrementalQueryDeltasState<'a, D>, QueryEvaluationError> {
        let mut prepared = self.evaluator.prepare(&self.query);
        for (variable, term) in self.substitutions {
            prepared = prepared.substitute_variable(variable, term);
        }
        *prepared.dataset_mut() = self.dataset;
        prepared.execute_incremental_deltas(self.queryable_dataset)
    }

    /// Evaluate the query incrementally and return a query explanation with result state.
    pub fn explain_incremental_results(
        self,
    ) -> (
        Result<IncrementalQueryResultsState<'a, D>, QueryEvaluationError>,
        QueryExplanation,
    ) {
        let mut prepared = self.evaluator.prepare(&self.query);
        for (variable, term) in self.substitutions {
            prepared = prepared.substitute_variable(variable, term);
        }
        *prepared.dataset_mut() = self.dataset;
        prepared.explain_incremental_results(self.queryable_dataset)
    }

    /// Evaluate the query incrementally and return a query explanation with delta state.
    pub fn explain_incremental_deltas(
        self,
    ) -> (
        Result<IncrementalQueryDeltasState<'a, D>, QueryEvaluationError>,
        QueryExplanation,
    ) {
        let mut prepared = self.evaluator.prepare(&self.query);
        for (variable, term) in self.substitutions {
            prepared = prepared.substitute_variable(variable, term);
        }
        *prepared.dataset_mut() = self.dataset;
        prepared.explain_incremental_deltas(self.queryable_dataset)
    }
}
