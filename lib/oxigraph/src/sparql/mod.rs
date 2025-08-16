//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.
//!
//! The entry point for SPARQL execution is the [`SparqlEvaluator`] type.

mod algebra;
mod dataset;
mod error;
#[cfg(feature = "http-client")]
mod http;
pub mod results;
mod update;

use crate::model::{NamedNode, Term};
#[expect(deprecated)]
pub use crate::sparql::algebra::{Query, QueryDataset, Update};
use crate::sparql::dataset::DatasetView;
pub use crate::sparql::error::UpdateEvaluationError;
#[cfg(feature = "http-client")]
use crate::sparql::http::HttpServiceHandler;
pub use crate::sparql::update::{BoundPreparedSparqlUpdate, PreparedSparqlUpdate};
use crate::storage::StorageReader;
use crate::store::{Store, Transaction};
use oxrdf::IriParseError;
pub use oxrdf::{Variable, VariableNameParseError};
use spareval::QueryEvaluator;
pub use spareval::{
    AggregateFunctionAccumulator, DefaultServiceHandler, QueryEvaluationError, QueryExplanation,
    QueryResults, QuerySolution, QuerySolutionIter, QueryTripleIter, ServiceHandler,
};
use spargebra::SparqlParser;
pub use spargebra::SparqlSyntaxError;
use std::collections::HashMap;
use std::mem::take;
#[cfg(feature = "http-client")]
use std::time::Duration;

#[deprecated(note = "Use SparqlEvaluator instead", since = "0.5.0")]
pub type QueryOptions = SparqlEvaluator;
#[deprecated(note = "Use SparqlEvaluator instead", since = "0.5.0")]
pub type UpdateOptions = SparqlEvaluator;
#[deprecated(note = "Use QueryEvaluationError instead", since = "0.5.0")]
pub type EvaluationError = QueryEvaluationError;

/// SPARQL evaluator.
///
/// It supports [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) and [SPARQL 1.1 update](https://www.w3.org/TR/sparql11-update/).
///
/// If the `"http-client"` optional feature is enabled,
/// a simple HTTP 1.1 client is used to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
///
/// Usage example disabling the federated query support:
/// ```
/// use oxigraph::model::NamedNode;
/// use oxigraph::sparql::SparqlEvaluator;
/// use oxigraph::store::Store;
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
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use oxigraph::store::Store;
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
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.parser = self.parser.with_base_iri(base_iri)?;
        Ok(self)
    }

    /// Set a default IRI prefix used during parsing.
    ///
    /// ```
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use oxigraph::store::Store;
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
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
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

    /// Disables the `SERVICE` calls
    #[cfg(feature = "http-client")]
    #[inline]
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
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use oxigraph::store::Store;
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

    /// Adds a custom SPARQL evaluation aggregate function.
    ///
    /// Example with a function doing concatenation:
    /// ```
    /// use oxigraph::model::{Literal, NamedNode, Term};
    /// use oxigraph::sparql::{AggregateFunctionAccumulator, QueryResults, SparqlEvaluator};
    /// use oxigraph::store::Store;
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

    #[doc(hidden)]
    #[inline]
    pub fn without_optimizations(mut self) -> Self {
        self.inner = self.inner.without_optimizations();
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
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use oxigraph::store::Store;
    ///
    /// let store = Store::new()?;
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// store.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?;
    ///
    /// let prepared_query = SparqlEvaluator::new().parse_query("SELECT ?s WHERE { ?s ?p ?o }")?;
    ///
    /// if let QueryResults::Solutions(mut solutions) = prepared_query.on_store(&store).execute()? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("s"),
    ///         Some(&ex.into_owned().into())
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
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use oxigraph::store::Store;
    /// use spargebra::SparqlParser;
    ///
    /// let store = Store::new()?;
    /// let ex = NamedNodeRef::new("http://example.com")?;
    /// store.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph))?;
    ///
    /// let query = SparqlParser::new().parse_query("SELECT ?s WHERE { ?s ?p ?o }")?;
    ///
    /// let prepared_query = SparqlEvaluator::new().for_query(query);
    ///
    /// if let QueryResults::Solutions(mut solutions) = prepared_query.on_store(&store).execute()? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("s"),
    ///         Some(&ex.into_owned().into())
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[expect(deprecated)]
    pub fn for_query(self, query: impl Into<Query>) -> PreparedSparqlQuery {
        let query = query.into();
        PreparedSparqlQuery {
            dataset: query.dataset,
            query: query.inner,
            evaluator: self.into_evaluator(),
            substitutions: HashMap::new(),
        }
    }

    /// Parse an update and returns a [`PreparedSparqlUpdate`] for the current evaluator.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::sparql::SparqlEvaluator;
    /// use oxigraph::store::Store;
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
    /// use oxigraph::sparql::SparqlEvaluator;
    /// use oxigraph::store::Store;
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
    #[expect(deprecated)]
    pub fn for_update(self, update: impl Into<Update>) -> PreparedSparqlUpdate {
        #[cfg(feature = "http-client")]
        let http_timeout = self.http_timeout;
        #[cfg(feature = "http-client")]
        let http_redirection_limit = self.http_redirection_limit;
        PreparedSparqlUpdate::new(
            self.into_evaluator(),
            update.into(),
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
/// use oxigraph::model::{Literal, Variable};
/// use oxigraph::sparql::{QueryResults, SparqlEvaluator};
/// use oxigraph::store::Store;
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
    query: spargebra::Query,
    dataset: QueryDataset,
    substitutions: HashMap<Variable, Term>,
}

impl PreparedSparqlQuery {
    /// Substitute a variable with a given RDF term in the SPARQL query.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::{Literal, Variable};
    /// use oxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use oxigraph::store::Store;
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
        BoundPreparedSparqlQuery {
            evaluator: self.evaluator,
            query: self.query,
            dataset: self.dataset,
            substitutions: self.substitutions,
            reader: store.storage().snapshot(),
        }
    }

    /// Bind the prepared query to the [`Transaction`] it should be evaluated on.
    pub fn on_transaction<'a>(
        self,
        transaction: &'a Transaction<'_>,
    ) -> BoundPreparedSparqlQuery<'a> {
        BoundPreparedSparqlQuery {
            evaluator: self.evaluator,
            query: self.query,
            dataset: self.dataset,
            substitutions: self.substitutions,
            reader: transaction.inner().reader(),
        }
    }
}

/// A prepared SPARQL query bound to a storage, ready to be executed.
///
/// Usage example:
/// ```
/// use oxigraph::model::{Literal, Variable};
/// use oxigraph::sparql::{QueryResults, SparqlEvaluator};
/// use oxigraph::store::Store;
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
pub struct BoundPreparedSparqlQuery<'a> {
    evaluator: QueryEvaluator,
    query: spargebra::Query,
    dataset: QueryDataset,
    substitutions: HashMap<Variable, Term>,
    reader: StorageReader<'a>,
}

impl<'a> BoundPreparedSparqlQuery<'a> {
    /// Substitute a variable with a given RDF term in the SPARQL query.
    ///
    /// Usage example:
    /// ```
    /// use oxigraph::model::{Literal, Variable};
    /// use oxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use oxigraph::store::Store;
    ///
    /// let prepared_query = SparqlEvaluator::new()
    ///     .parse_query("SELECT ?v WHERE {}")?
    ///     .on_store(&Store::new()?)
    ///     .substitute_variable(Variable::new("v")?, Literal::from(1));
    ///
    /// if let QueryResults::Solutions(mut solutions) = prepared_query.execute()? {
    ///     assert_eq!(
    ///         solutions.next().unwrap()?.get("v"),
    ///         Some(&Literal::from(1).into())
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
        let dataset = DatasetView::new(self.reader, &self.dataset);
        self.evaluator
            .execute_with_substituted_variables(dataset, &self.query, self.substitutions)
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
    /// use oxigraph::sparql::{QueryResults, SparqlEvaluator};
    /// use oxigraph::store::Store;
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
        let dataset = DatasetView::new(self.reader, &self.dataset);
        let (results, explanation) = self.evaluator.explain_with_substituted_variables(
            dataset,
            &self.query,
            self.substitutions,
        );
        (results, explanation)
    }
}
