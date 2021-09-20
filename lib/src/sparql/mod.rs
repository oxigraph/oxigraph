//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.
//!
//! Stores execute SPARQL. See [`Store`](crate::store::Store::query()) for an example.

mod algebra;
mod csv_results;
mod dataset;
mod error;
mod eval;
mod http;
mod json_results;
mod model;
mod plan;
mod plan_builder;
mod service;
mod update;
mod xml_results;

pub use crate::sparql::algebra::{Query, Update};
use crate::sparql::dataset::DatasetView;
pub use crate::sparql::error::EvaluationError;
use crate::sparql::eval::SimpleEvaluator;
pub use crate::sparql::model::QueryResults;
pub use crate::sparql::model::QueryResultsFormat;
pub use crate::sparql::model::QuerySolution;
pub use crate::sparql::model::QuerySolutionIter;
pub use crate::sparql::model::QueryTripleIter;
pub use crate::sparql::model::{Variable, VariableNameParseError};
use crate::sparql::plan_builder::PlanBuilder;
pub use crate::sparql::service::ServiceHandler;
use crate::sparql::service::{EmptyServiceHandler, ErrorConversionServiceHandler};
use crate::sparql::update::SimpleUpdateEvaluator;
use crate::storage::Storage;
pub use spargebra::ParseError;
use std::convert::TryInto;
use std::rc::Rc;
use std::time::Duration;

#[allow(clippy::needless_pass_by_value)]
pub(crate) fn evaluate_query(
    storage: Storage,
    query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
    options: QueryOptions,
) -> Result<QueryResults, EvaluationError> {
    let query = query.try_into().map_err(std::convert::Into::into)?;
    let dataset = DatasetView::new(storage, &query.dataset);
    match query.inner {
        spargebra::Query::Select {
            pattern, base_iri, ..
        } => {
            let (plan, variables) = PlanBuilder::build(&dataset, &pattern)?;
            Ok(SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler(),
            )
            .evaluate_select_plan(
                &plan,
                Rc::new(
                    variables
                        .into_iter()
                        .map(|v| Variable::new_unchecked(v.name))
                        .collect(),
                ),
            ))
        }
        spargebra::Query::Ask {
            pattern, base_iri, ..
        } => {
            let (plan, _) = PlanBuilder::build(&dataset, &pattern)?;
            SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler(),
            )
            .evaluate_ask_plan(&plan)
        }
        spargebra::Query::Construct {
            template,
            pattern,
            base_iri,
            ..
        } => {
            let (plan, variables) = PlanBuilder::build(&dataset, &pattern)?;
            let construct = PlanBuilder::build_graph_template(&dataset, &template, variables);
            Ok(SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler(),
            )
            .evaluate_construct_plan(&plan, construct))
        }
        spargebra::Query::Describe {
            pattern, base_iri, ..
        } => {
            let (plan, _) = PlanBuilder::build(&dataset, &pattern)?;
            Ok(SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler(),
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
#[derive(Clone)]
pub struct QueryOptions {
    pub(crate) service_handler: Option<Rc<dyn ServiceHandler<Error = EvaluationError>>>,
    http_timeout: Option<Duration>,
}

impl Default for QueryOptions {
    #[inline]
    fn default() -> Self {
        Self {
            service_handler: None,
            http_timeout: None,
        }
    }
}

impl QueryOptions {
    /// Use a given [`ServiceHandler`] to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
    #[inline]
    pub fn with_service_handler(mut self, service_handler: impl ServiceHandler + 'static) -> Self {
        self.service_handler = Some(Rc::new(ErrorConversionServiceHandler::wrap(
            service_handler,
        )));
        self
    }

    /// Disables the `SERVICE` calls
    #[inline]
    pub fn without_service_handler(mut self) -> Self {
        self.service_handler = Some(Rc::new(EmptyServiceHandler));
        self
    }

    /// Sets a timeout for HTTP requests done during SPARQL evaluation
    #[cfg(feature = "http_client")]
    pub fn with_http_timeout(mut self, timeout: Duration) -> Self {
        self.http_timeout = Some(timeout);
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

/// Options for SPARQL update evaluation
#[derive(Clone)]
pub struct UpdateOptions {
    query_options: QueryOptions,
}

impl UpdateOptions {
    /// The options related to the querying part of the updates
    #[inline]
    pub fn query_options(&self) -> &QueryOptions {
        &self.query_options
    }

    /// The options related to the querying part of the updates
    #[inline]
    pub fn query_options_mut(&mut self) -> &mut QueryOptions {
        &mut self.query_options
    }
}

impl Default for UpdateOptions {
    #[inline]
    fn default() -> Self {
        Self {
            query_options: QueryOptions::default(),
        }
    }
}

impl From<QueryOptions> for UpdateOptions {
    #[inline]
    fn from(query_options: QueryOptions) -> Self {
        Self { query_options }
    }
}

pub(crate) fn evaluate_update(
    storage: &Storage,
    update: Update,
    options: UpdateOptions,
) -> Result<(), EvaluationError> {
    SimpleUpdateEvaluator::new(storage, update.inner.base_iri.map(Rc::new), options)
        .eval_all(&update.inner.operations, &update.using_datasets)
}
