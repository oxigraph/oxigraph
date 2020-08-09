//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.
//!
//! SPARQL evaluation is done from a store. See [`MemoryStore`](../store/memory/struct.MemoryStore.html#method.query) for an example.

mod algebra;
mod dataset;
mod error;
mod eval;
mod json_results;
mod model;
mod parser;
mod plan;
mod plan_builder;
mod xml_results;

use crate::model::NamedNode;
use crate::sparql::algebra::{DatasetSpec, QueryVariants};
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
pub use crate::sparql::parser::Query;
use crate::sparql::plan::{PlanNode, TripleTemplate};
use crate::sparql::plan_builder::PlanBuilder;
use crate::store::numeric_encoder::StrEncodingAware;
use crate::store::ReadableEncodedStore;
use std::convert::TryInto;
use std::error::Error;
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
                    if options.dataset.is_empty() {
                        &dataset
                    } else {
                        &options.dataset
                    },
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
                    if options.dataset.is_empty() {
                        &dataset
                    } else {
                        &options.dataset
                    },
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
                    if options.dataset.is_empty() {
                        &dataset
                    } else {
                        &options.dataset
                    },
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
                    if options.dataset.is_empty() {
                        &dataset
                    } else {
                        &options.dataset
                    },
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
    pub(crate) dataset: DatasetSpec,
    pub(crate) service_handler: Rc<dyn ServiceHandler<Error = EvaluationError>>,
}

impl Default for QueryOptions {
    #[inline]
    fn default() -> Self {
        Self {
            default_graph_as_union: false,
            dataset: DatasetSpec::default(),
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

    /// Adds a named graph to the set of graphs considered by the SPARQL query as the queried dataset default graph.
    /// It overrides the `FROM` and `FROM NAMED` elements of the evaluated query.
    #[inline]
    pub fn with_default_graph(mut self, default_graph_name: impl Into<NamedNode>) -> Self {
        self.dataset.default.push(default_graph_name.into());
        self
    }

    /// Adds a named graph to the set of graphs considered by the SPARQL query as the queried dataset named graphs.
    /// It overrides the `FROM` and `FROM NAMED` elements of the evaluated query.
    #[inline]
    pub fn with_named_graph(mut self, named_graph_name: impl Into<NamedNode>) -> Self {
        self.dataset.named.push(named_graph_name.into());
        self
    }

    /// Use a given [`ServiceHandler`](trait.ServiceHandler.html) to execute [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE calls.
    #[inline]
    pub fn with_service_handler(mut self, service_handler: impl ServiceHandler + 'static) -> Self {
        self.service_handler = Rc::new(ErrorConversionServiceHandler {
            handler: service_handler,
        });
        self
    }
}

/// Handler for [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/) SERVICE.
///
/// Should be given to [`QueryOptions`](struct.QueryOptions.html#method.with_service_handler)
/// before evaluating a SPARQL query that uses SERVICE calls.
///
/// ```
/// use oxigraph::MemoryStore;
/// use oxigraph::model::*;
/// use oxigraph::sparql::{QueryOptions, QueryResults, ServiceHandler, Query, EvaluationError};
///
/// #[derive(Default)]
/// struct TestServiceHandler {
///     store: MemoryStore
/// }
///
/// impl ServiceHandler for TestServiceHandler {
///     type Error = EvaluationError;
///
///     fn handle(&self,service_name: NamedNode, query: Query) -> Result<QueryResults,EvaluationError> {
///         if service_name == "http://example.com/service" {
///             self.store.query(query, QueryOptions::default())
///         } else {
///             panic!()
///         }
///     }
/// }
///
/// let store = MemoryStore::new();
/// let service = TestServiceHandler::default();
/// let ex = NamedNode::new("http://example.com")?;
/// service.store.insert(Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
///
/// if let QueryResults::Solutions(mut solutions) = store.query(
///     "SELECT ?s WHERE { SERVICE <http://example.com/service> { ?s ?p ?o } }",
///     QueryOptions::default().with_service_handler(service)
/// )? {
///     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
/// }
/// # Result::<_,Box<dyn std::error::Error>>::Ok(())
/// ```
pub trait ServiceHandler {
    type Error: Error + Send + Sync + 'static;

    /// Evaluates a [`Query`](struct.Query.html) against a given service identified by a [`NamedNode`](../model/struct.NamedNode.html).
    fn handle(&self, service_name: NamedNode, query: Query) -> Result<QueryResults, Self::Error>;
}

struct EmptyServiceHandler;

impl ServiceHandler for EmptyServiceHandler {
    type Error = EvaluationError;

    fn handle(&self, _: NamedNode, _: Query) -> Result<QueryResults, EvaluationError> {
        Err(EvaluationError::msg(
            "The SERVICE feature is not implemented",
        ))
    }
}

struct ErrorConversionServiceHandler<S: ServiceHandler> {
    handler: S,
}

impl<S: ServiceHandler> ServiceHandler for ErrorConversionServiceHandler<S> {
    type Error = EvaluationError;

    fn handle(
        &self,
        service_name: NamedNode,
        query: Query,
    ) -> Result<QueryResults, EvaluationError> {
        self.handler
            .handle(service_name, query)
            .map_err(EvaluationError::wrap)
    }
}
