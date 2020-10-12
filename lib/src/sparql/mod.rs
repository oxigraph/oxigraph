//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.
//!
//! Stores execute SPARQL. See [`MemoryStore`](../store/memory/struct.MemoryStore.html#method.query) for an example.

mod algebra;
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

use crate::sparql::algebra::QueryVariants;
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
    match query.try_into().map_err(|e| e.into())?.0 {
        QueryVariants::Select {
            algebra,
            base_iri,
            dataset,
        } => {
            let dataset = DatasetView::new(store, &dataset)?;
            let (plan, variables) = PlanBuilder::build(&dataset, &algebra)?;
            SimpleEvaluator::new(Rc::new(dataset), base_iri, options.service_handler)
                .evaluate_select_plan(&plan, Rc::new(variables))
        }
        QueryVariants::Ask {
            algebra,
            base_iri,
            dataset,
        } => {
            let dataset = DatasetView::new(store, &dataset)?;
            let (plan, _) = PlanBuilder::build(&dataset, &algebra)?;
            SimpleEvaluator::new(Rc::new(dataset), base_iri, options.service_handler)
                .evaluate_ask_plan(&plan)
        }
        QueryVariants::Construct {
            construct,
            algebra,
            base_iri,
            dataset,
        } => {
            let dataset = DatasetView::new(store, &dataset)?;
            let (plan, variables) = PlanBuilder::build(&dataset, &algebra)?;
            let construct = PlanBuilder::build_graph_template(&dataset, &construct, variables)?;
            SimpleEvaluator::new(Rc::new(dataset), base_iri, options.service_handler)
                .evaluate_construct_plan(&plan, construct)
        }
        QueryVariants::Describe {
            algebra,
            base_iri,
            dataset,
        } => {
            let dataset = DatasetView::new(store, &dataset)?;
            let (plan, _) = PlanBuilder::build(&dataset, &algebra)?;
            SimpleEvaluator::new(Rc::new(dataset), base_iri, options.service_handler)
                .evaluate_describe_plan(&plan)
        }
    }
}

/// Options for SPARQL query evaluation
#[derive(Clone)]
pub struct QueryOptions {
    pub(crate) service_handler: Rc<dyn ServiceHandler<Error = EvaluationError>>,
}

impl Default for QueryOptions {
    #[inline]
    fn default() -> Self {
        Self {
            service_handler: Rc::new(EmptyServiceHandler),
        }
    }
}

impl QueryOptions {
    /// Use a simple HTTP 1.1 client built into Oxigraph to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
    ///
    /// Requires the `"http_client"` optional feature.
    #[inline]
    #[cfg(feature = "http_client")]
    pub fn with_simple_service_handler(mut self) -> Self {
        self.service_handler = Rc::new(service::SimpleServiceHandler::new());
        self
    }

    /// Use a given [`ServiceHandler`](trait.ServiceHandler.html) to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
    #[inline]
    pub fn with_service_handler(mut self, service_handler: impl ServiceHandler + 'static) -> Self {
        self.service_handler = Rc::new(ErrorConversionServiceHandler::wrap(service_handler));
        self
    }
}

pub(crate) fn evaluate_update<
    R: ReadableEncodedStore + Clone + 'static,
    W: StrContainer<StrId = R::StrId> + WritableEncodedStore<StrId = R::StrId>,
>(
    read: R,
    write: &mut W,
    update: &Update,
) -> Result<(), EvaluationError>
where
    io::Error: From<StoreOrParseError<W::Error>>,
{
    SimpleUpdateEvaluator::new(
        read,
        write,
        update.base_iri.clone(),
        Rc::new(EmptyServiceHandler),
    )
    .eval_all(&update.operations)
}
