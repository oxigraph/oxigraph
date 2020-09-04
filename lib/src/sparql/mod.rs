//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.
//!
//! Stores execute SPARQL. See [`MemoryStore`](super::store::memory::MemoryStore::query()) for an example.

pub mod algebra;
mod csv_results;
mod dataset;
mod error;
mod eval;
mod http;
mod json_results;
mod model;
mod parser;
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
pub use crate::sparql::parser::ParseError;
use crate::sparql::plan_builder::PlanBuilder;
pub use crate::sparql::service::ServiceHandler;
use crate::sparql::service::{EmptyServiceHandler, ErrorConversionServiceHandler};
use crate::sparql::update::SimpleUpdateEvaluator;
use crate::store::numeric_encoder::StrContainer;
use crate::store::{ReadableEncodedStore, StoreOrParseError, WritableEncodedStore};
use std::convert::TryInto;
use std::io;
use std::rc::Rc;

pub(crate) fn evaluate_query<R: ReadableEncodedStore + 'static>(
    store: R,
    query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
    options: QueryOptions,
) -> Result<QueryResults, EvaluationError> {
    match query.try_into().map_err(|e| e.into())? {
        Query::Select {
            pattern,
            base_iri,
            dataset,
        } => {
            let dataset = DatasetView::new(store, &dataset)?;
            let (plan, variables) = PlanBuilder::build(&dataset, &pattern)?;
            SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler,
            )
            .evaluate_select_plan(&plan, Rc::new(variables))
        }
        Query::Ask {
            pattern,
            base_iri,
            dataset,
        } => {
            let dataset = DatasetView::new(store, &dataset)?;
            let (plan, _) = PlanBuilder::build(&dataset, &pattern)?;
            SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler,
            )
            .evaluate_ask_plan(&plan)
        }
        Query::Construct {
            template,
            pattern,
            base_iri,
            dataset,
        } => {
            let dataset = DatasetView::new(store, &dataset)?;
            let (plan, variables) = PlanBuilder::build(&dataset, &pattern)?;
            let construct = PlanBuilder::build_graph_template(&dataset, &template, variables)?;
            SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler,
            )
            .evaluate_construct_plan(&plan, construct)
        }
        Query::Describe {
            pattern,
            base_iri,
            dataset,
        } => {
            let dataset = DatasetView::new(store, &dataset)?;
            let (plan, _) = PlanBuilder::build(&dataset, &pattern)?;
            SimpleEvaluator::new(
                Rc::new(dataset),
                base_iri.map(Rc::new),
                options.service_handler,
            )
            .evaluate_describe_plan(&plan)
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
    pub(crate) service_handler: Rc<dyn ServiceHandler<Error = EvaluationError>>,
}

impl Default for QueryOptions {
    #[inline]
    fn default() -> Self {
        Self {
            service_handler: if cfg!(feature = "http_client") {
                Rc::new(service::SimpleServiceHandler::new())
            } else {
                Rc::new(EmptyServiceHandler)
            },
        }
    }
}

impl QueryOptions {
    /// Use a given [`ServiceHandler`] to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
    #[inline]
    pub fn with_service_handler(mut self, service_handler: impl ServiceHandler + 'static) -> Self {
        self.service_handler = Rc::new(ErrorConversionServiceHandler::wrap(service_handler));
        self
    }

    /// Disables the `SERVICE` calls
    #[inline]
    pub fn without_service_handler(mut self) -> Self {
        self.service_handler = Rc::new(EmptyServiceHandler);
        self
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

pub(crate) fn evaluate_update<
    R: ReadableEncodedStore + Clone + 'static,
    W: StrContainer<StrId = R::StrId> + WritableEncodedStore<StrId = R::StrId>,
>(
    read: R,
    write: &mut W,
    update: Update,
    options: UpdateOptions,
) -> Result<(), EvaluationError>
where
    io::Error: From<StoreOrParseError<W::Error>>,
{
    SimpleUpdateEvaluator::new(read, write, update.base_iri.map(Rc::new), options)
        .eval_all(&update.operations)
}
