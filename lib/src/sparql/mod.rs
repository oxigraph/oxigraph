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

use crate::model::{GraphName, NamedOrBlankNode};
use crate::sparql::algebra::QueryVariants;
use crate::sparql::dataset::DatasetView;
pub use crate::sparql::error::EvaluationError;
use crate::sparql::eval::SimpleEvaluator;
pub use crate::sparql::model::QueryResults;
pub use crate::sparql::model::QueryResultsFormat;
pub use crate::sparql::model::QuerySolution;
pub use crate::sparql::model::QuerySolutionIter;
pub use crate::sparql::model::QueryTripleIter;
pub use crate::sparql::model::Variable;
pub use crate::sparql::parser::ParseError;
pub use crate::sparql::parser::{Query, Update};
use crate::sparql::plan::{PlanNode, TripleTemplate};
use crate::sparql::plan_builder::PlanBuilder;
pub use crate::sparql::service::ServiceHandler;
use crate::sparql::service::{
    EmptyServiceHandler, ErrorConversionServiceHandler, SimpleServiceHandler,
};
use crate::sparql::update::SimpleUpdateEvaluator;
use crate::store::numeric_encoder::{StrContainer, StrEncodingAware};
use crate::store::{ReadableEncodedStore, StoreOrParseError, WritableEncodedStore};
use std::convert::TryInto;
use std::io;
use std::rc::Rc;

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/)
pub(crate) struct SimplePreparedQuery<S: ReadableEncodedStore + 'static>(
    SimplePreparedQueryAction<S>,
);

#[derive(Clone)]
enum SimplePreparedQueryAction<S: ReadableEncodedStore + 'static> {
    Select {
        plan: Rc<PlanNode<<DatasetView<S> as StrEncodingAware>::StrId>>,
        variables: Rc<Vec<Variable>>,
        evaluator: SimpleEvaluator<DatasetView<S>>,
    },
    Ask {
        plan: Rc<PlanNode<<DatasetView<S> as StrEncodingAware>::StrId>>,
        evaluator: SimpleEvaluator<DatasetView<S>>,
    },
    Construct {
        plan: Rc<PlanNode<<DatasetView<S> as StrEncodingAware>::StrId>>,
        construct: Rc<Vec<TripleTemplate<<DatasetView<S> as StrEncodingAware>::StrId>>>,
        evaluator: SimpleEvaluator<DatasetView<S>>,
    },
    Describe {
        plan: Rc<PlanNode<<DatasetView<S> as StrEncodingAware>::StrId>>,
        evaluator: SimpleEvaluator<DatasetView<S>>,
    },
}

impl<S: ReadableEncodedStore + 'static> SimplePreparedQuery<S> {
    pub(crate) fn new(
        store: S,
        query: impl TryInto<Query, Error = impl Into<EvaluationError>>,
        options: QueryOptions,
    ) -> Result<Self, EvaluationError> {
        Ok(Self(match query.try_into().map_err(|e| e.into())?.0 {
            QueryVariants::Select {
                algebra,
                base_iri,
                dataset,
            } => {
                let dataset = Rc::new(DatasetView::new(
                    store,
                    options.default_graph_as_union,
                    &options.default_graphs,
                    &options.named_graphs,
                    &dataset,
                )?);
                let (plan, variables) = PlanBuilder::build(dataset.as_ref(), &algebra)?;
                SimplePreparedQueryAction::Select {
                    plan: Rc::new(plan),
                    variables: Rc::new(variables),
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
            QueryVariants::Ask {
                algebra,
                base_iri,
                dataset,
            } => {
                let dataset = Rc::new(DatasetView::new(
                    store,
                    options.default_graph_as_union,
                    &options.default_graphs,
                    &options.named_graphs,
                    &dataset,
                )?);
                let (plan, _) = PlanBuilder::build(dataset.as_ref(), &algebra)?;
                SimplePreparedQueryAction::Ask {
                    plan: Rc::new(plan),
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
            QueryVariants::Construct {
                construct,
                algebra,
                base_iri,
                dataset,
            } => {
                let dataset = Rc::new(DatasetView::new(
                    store,
                    options.default_graph_as_union,
                    &options.default_graphs,
                    &options.named_graphs,
                    &dataset,
                )?);
                let (plan, variables) = PlanBuilder::build(dataset.as_ref(), &algebra)?;
                SimplePreparedQueryAction::Construct {
                    plan: Rc::new(plan),
                    construct: Rc::new(PlanBuilder::build_graph_template(
                        dataset.as_ref(),
                        &construct,
                        variables,
                    )?),
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
            QueryVariants::Describe {
                algebra,
                base_iri,
                dataset,
            } => {
                let dataset = Rc::new(DatasetView::new(
                    store,
                    options.default_graph_as_union,
                    &options.default_graphs,
                    &options.named_graphs,
                    &dataset,
                )?);
                let (plan, _) = PlanBuilder::build(dataset.as_ref(), &algebra)?;
                SimplePreparedQueryAction::Describe {
                    plan: Rc::new(plan),
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
        }))
    }

    /// Evaluates the query and returns its results
    pub fn exec(&self) -> Result<QueryResults, EvaluationError> {
        match &self.0 {
            SimplePreparedQueryAction::Select {
                plan,
                variables,
                evaluator,
            } => evaluator.evaluate_select_plan(plan, variables.clone()),
            SimplePreparedQueryAction::Ask { plan, evaluator } => evaluator.evaluate_ask_plan(plan),
            SimplePreparedQueryAction::Construct {
                plan,
                construct,
                evaluator,
            } => evaluator.evaluate_construct_plan(plan, construct.clone()),
            SimplePreparedQueryAction::Describe { plan, evaluator } => {
                evaluator.evaluate_describe_plan(plan)
            }
        }
    }
}

/// Options for SPARQL query evaluation
#[derive(Clone)]
pub struct QueryOptions {
    pub(crate) default_graph_as_union: bool,
    pub(crate) default_graphs: Vec<GraphName>,
    pub(crate) named_graphs: Vec<NamedOrBlankNode>,
    pub(crate) service_handler: Rc<dyn ServiceHandler<Error = EvaluationError>>,
}

impl Default for QueryOptions {
    #[inline]
    fn default() -> Self {
        Self {
            default_graph_as_union: false,
            default_graphs: Vec::new(),
            named_graphs: Vec::new(),
            service_handler: Rc::new(EmptyServiceHandler),
        }
    }
}

impl QueryOptions {
    /// Consider the union of all graphs in the store as the default graph
    #[inline]
    pub fn with_default_graph_as_union(mut self) -> Self {
        self.default_graph_as_union = true;
        self
    }

    /// Adds a graph to the set of graphs considered by the SPARQL query as the queried dataset default graph.
    /// It overrides the `FROM` and `FROM NAMED` elements of the evaluated query.
    #[inline]
    pub fn with_default_graph(mut self, default_graph_name: impl Into<GraphName>) -> Self {
        self.default_graphs.push(default_graph_name.into());
        self
    }

    /// Adds a named graph to the set of graphs considered by the SPARQL query as the queried dataset named graphs.
    /// It overrides the `FROM` and `FROM NAMED` elements of the evaluated query.
    #[inline]
    pub fn with_named_graph(mut self, named_graph_name: impl Into<NamedOrBlankNode>) -> Self {
        self.named_graphs.push(named_graph_name.into());
        self
    }

    /// Use a simple HTTP 1.1 client built into Oxigraph to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
    ///
    /// Requires the `"http_client"` optional feature.
    #[inline]
    #[cfg(feature = "http_client")]
    pub fn with_simple_service_handler(mut self) -> Self {
        self.service_handler = Rc::new(SimpleServiceHandler::new());
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
