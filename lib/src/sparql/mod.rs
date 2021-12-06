//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.
//!
//! Stores execute SPARQL. See [`Store`](crate::store::Store::query()) for an example.

mod algebra;
mod dataset;
mod error;
mod eval;
mod http;
mod model;
mod plan;
mod plan_builder;
mod service;
mod update;

use crate::model::{NamedNode, Term};
pub use crate::sparql::algebra::{Query, Update};
use crate::sparql::dataset::DatasetView;
pub use crate::sparql::error::{EvaluationError, QueryError};
use crate::sparql::eval::SimpleEvaluator;
pub use crate::sparql::model::{QueryResults, QuerySolution, QuerySolutionIter, QueryTripleIter};
use crate::sparql::plan_builder::PlanBuilder;
pub use crate::sparql::service::ServiceHandler;
use crate::sparql::service::{EmptyServiceHandler, ErrorConversionServiceHandler};
pub(crate) use crate::sparql::update::evaluate_update;
use crate::storage::StorageReader;
pub use oxrdf::{Variable, VariableNameParseError};
pub use sparesults::QueryResultsFormat;
pub use spargebra::ParseError;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn evaluate_query(
    reader: StorageReader,
    query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
    options: QueryOptions,
) -> Result<QueryResults, EvaluationError> {
    let query = query.try_into().map_err(std::convert::Into::into)?;
    let dataset = DatasetView::new(reader, &query.dataset);
    match query.inner {
        spargebra::Query::Select {
            pattern, base_iri, ..
        } => {
            let (plan, variables) =
                PlanBuilder::build(&dataset, &pattern, true, &options.custom_functions)?;
            Ok(SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler(),
                Rc::new(options.custom_functions),
            )
            .evaluate_select_plan(&plan, Rc::new(variables)))
        }
        spargebra::Query::Ask {
            pattern, base_iri, ..
        } => {
            let (plan, _) =
                PlanBuilder::build(&dataset, &pattern, false, &options.custom_functions)?;
            SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler(),
                Rc::new(options.custom_functions),
            )
            .evaluate_ask_plan(&plan)
        }
        spargebra::Query::Construct {
            template,
            pattern,
            base_iri,
            ..
        } => {
            let (plan, variables) =
                PlanBuilder::build(&dataset, &pattern, false, &options.custom_functions)?;
            let construct = PlanBuilder::build_graph_template(
                &dataset,
                &template,
                variables,
                &options.custom_functions,
            );
            Ok(SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler(),
                Rc::new(options.custom_functions),
            )
            .evaluate_construct_plan(&plan, construct))
        }
        spargebra::Query::Describe {
            pattern, base_iri, ..
        } => {
            let (plan, _) =
                PlanBuilder::build(&dataset, &pattern, false, &options.custom_functions)?;
            Ok(SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler(),
                Rc::new(options.custom_functions),
            )
            .evaluate_describe_plan(&plan))
        }
    }
}

/// Options for SPARQL query evaluation.
///
///
/// If the `"http_client"` optional feature is enabled,
/// a simple HTTP 1.1 client is used to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
///
/// Usage example disabling the federated query support:
/// ```
/// use oxigraph::store::Store;
/// use oxigraph::sparql::QueryOptions;
///
/// let store = Store::new()?;
/// store.query_opt(
///     "SELECT * WHERE { SERVICE <https://query.wikidata.org/sparql> {} }",
///     QueryOptions::default().without_service_handler()
/// )?;
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Clone, Default)]
pub struct QueryOptions {
    service_handler: Option<Rc<dyn ServiceHandler<Error = EvaluationError>>>,
    custom_functions: HashMap<NamedNode, Rc<dyn Fn(&[Term]) -> Option<Term>>>,
    http_timeout: Option<Duration>,
}

impl QueryOptions {
    /// Use a given [`ServiceHandler`] to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
    #[inline]
    #[must_use]
    pub fn with_service_handler(mut self, service_handler: impl ServiceHandler + 'static) -> Self {
        self.service_handler = Some(Rc::new(ErrorConversionServiceHandler::wrap(
            service_handler,
        )));
        self
    }

    /// Disables the `SERVICE` calls
    #[inline]
    #[must_use]
    pub fn without_service_handler(mut self) -> Self {
        self.service_handler = Some(Rc::new(EmptyServiceHandler));
        self
    }

    /// Sets a timeout for HTTP requests done during SPARQL evaluation
    #[cfg(feature = "http_client")]
    #[inline]
    #[must_use]
    pub fn with_http_timeout(mut self, timeout: Duration) -> Self {
        self.http_timeout = Some(timeout);
        self
    }

    /// Adds a custom SPARQL evaluation function.
    ///
    /// Example with a function serializing terms to N-Triples:
    /// ```
    /// use oxigraph::store::Store;
    /// use oxigraph::model::*;
    /// use oxigraph::sparql::{QueryOptions, QueryResults};
    ///
    /// let store = Store::new()?;
    ///
    /// if let QueryResults::Solutions(mut solutions) = store.query_opt(
    ///     "SELECT (<http://www.w3.org/ns/formats/N-Triples>(1) AS ?nt) WHERE {}",
    ///     QueryOptions::default().with_custom_function(
    ///         NamedNode::new("http://www.w3.org/ns/formats/N-Triples")?,
    ///         |args| args.get(0).map(|t| Literal::from(t.to_string()).into())
    ///     )
    /// )? {
    ///     assert_eq!(solutions.next().unwrap()?.get("nt"), Some(&Literal::from("\"1\"^^<http://www.w3.org/2001/XMLSchema#integer>").into()));
    /// }
    /// # Result::<_,Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    #[must_use]
    pub fn with_custom_function(
        mut self,
        name: NamedNode,
        evaluator: impl Fn(&[Term]) -> Option<Term> + 'static,
    ) -> Self {
        self.custom_functions.insert(name, Rc::new(evaluator));
        self
    }

    fn service_handler(&self) -> Rc<dyn ServiceHandler<Error = EvaluationError>> {
        self.service_handler.clone().unwrap_or_else(|| {
            if cfg!(feature = "http_client") {
                Rc::new(service::SimpleServiceHandler::new(self.http_timeout))
            } else {
                Rc::new(EmptyServiceHandler)
            }
        })
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
