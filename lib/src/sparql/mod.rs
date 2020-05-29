//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.

mod algebra;
mod eval;
mod json_results;
mod model;
mod parser;
mod plan;
mod plan_builder;
mod xml_results;

use crate::model::NamedNode;
use crate::sparql::algebra::QueryVariants;
use crate::sparql::eval::SimpleEvaluator;
use crate::sparql::plan::TripleTemplate;
use crate::sparql::plan::{DatasetView, PlanNode};
use crate::sparql::plan_builder::PlanBuilder;
use crate::store::ReadableEncodedStore;
use crate::Error;
use crate::Result;
use oxiri::Iri;

pub use crate::sparql::algebra::GraphPattern;
pub use crate::sparql::model::QuerySolution;
pub use crate::sparql::model::QuerySolutionsIterator;
#[deprecated(note = "Please directly use QuerySolutionsIterator type instead")]
pub type BindingsIterator<'a> = QuerySolutionsIterator<'a>;
pub use crate::sparql::model::QueryResult;
pub use crate::sparql::model::QueryResultSyntax;
pub use crate::sparql::model::Variable;
pub use crate::sparql::parser::Query;
pub use crate::sparql::parser::SparqlParseError;

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/)
#[deprecated(
    note = "Not useful anymore. The exec method is already implemented by the different PreparedQuery structures"
)]
pub trait PreparedQuery {}

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/)
pub(crate) struct SimplePreparedQuery<S: ReadableEncodedStore>(SimplePreparedQueryAction<S>);

enum SimplePreparedQueryAction<S: ReadableEncodedStore> {
    Select {
        plan: PlanNode,
        variables: Vec<Variable>,
        evaluator: SimpleEvaluator<S>,
    },
    Ask {
        plan: PlanNode,
        evaluator: SimpleEvaluator<S>,
    },
    Construct {
        plan: PlanNode,
        construct: Vec<TripleTemplate>,
        evaluator: SimpleEvaluator<S>,
    },
    Describe {
        plan: PlanNode,
        evaluator: SimpleEvaluator<S>,
    },
}

impl<S: ReadableEncodedStore> SimplePreparedQuery<S> {
    pub(crate) fn new(store: S, query: &str, options: QueryOptions<'_>) -> Result<Self> {
        let dataset = DatasetView::new(store, options.default_graph_as_union);
        Ok(Self(match Query::parse(query, options.base_iri)?.0 {
            QueryVariants::Select {
                algebra, base_iri, ..
            } => {
                let (plan, variables) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Select {
                    plan,
                    variables,
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
            QueryVariants::Ask {
                algebra, base_iri, ..
            } => {
                let (plan, _) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Ask {
                    plan,
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
            QueryVariants::Construct {
                construct,
                algebra,
                base_iri,
                ..
            } => {
                let (plan, variables) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Construct {
                    plan,
                    construct: PlanBuilder::build_graph_template(
                        dataset.encoder(),
                        &construct,
                        variables,
                    )?,
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
            QueryVariants::Describe {
                algebra, base_iri, ..
            } => {
                let (plan, _) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Describe {
                    plan,
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
        }))
    }

    /// Builds `SimplePreparedQuery` from an existing `GraphPattern`. This is used to support federated queries via `SERVICE` clauses
    pub(crate) fn new_from_pattern(
        store: S,
        pattern: &GraphPattern,
        options: QueryOptions<'_>,
    ) -> Result<Self> {
        let dataset = DatasetView::new(store, options.default_graph_as_union);
        let (plan, variables) = PlanBuilder::build(dataset.encoder(), pattern)?;
        let base_iri = if let Some(base_iri) = options.base_iri {
            Some(Iri::parse(base_iri.to_string())?)
        } else {
            None
        };
        Ok(Self(SimplePreparedQueryAction::Select {
            plan,
            variables,
            evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
        }))
    }

    /// Evaluates the query and returns its results
    pub fn exec(&self) -> Result<QueryResult<'_>> {
        match &self.0 {
            SimplePreparedQueryAction::Select {
                plan,
                variables,
                evaluator,
            } => evaluator.evaluate_select_plan(plan, variables),
            SimplePreparedQueryAction::Ask { plan, evaluator } => evaluator.evaluate_ask_plan(plan),
            SimplePreparedQueryAction::Construct {
                plan,
                construct,
                evaluator,
            } => evaluator.evaluate_construct_plan(plan, construct),
            SimplePreparedQueryAction::Describe { plan, evaluator } => {
                evaluator.evaluate_describe_plan(plan)
            }
        }
    }
}

/// Handler for SPARQL SERVICEs.
///
/// Might be used to implement [SPARQL 1.1 Federated Query](https://www.w3.org/TR/sparql11-federated-query/)
pub trait ServiceHandler {
    /// Evaluates a `GraphPattern` against a given service identified by a `NamedNode`.
    fn handle<'a>(
        &'a self,
        service_name: &NamedNode,
        graph_pattern: &'a GraphPattern,
    ) -> Result<QuerySolutionsIterator<'a>>;
}

impl<F: for<'a> Fn(&NamedNode, &'a GraphPattern) -> Result<QuerySolutionsIterator<'a>>>
    ServiceHandler for F
{
    fn handle<'a>(
        &'a self,
        service_name: &NamedNode,
        graph_pattern: &'a GraphPattern,
    ) -> Result<QuerySolutionsIterator<'a>> {
        self(service_name, graph_pattern)
    }
}

struct EmptyServiceHandler;

impl ServiceHandler for EmptyServiceHandler {
    fn handle<'a>(
        &'a self,
        _: &NamedNode,
        _: &'a GraphPattern,
    ) -> Result<QuerySolutionsIterator<'a>> {
        Err(Error::msg("The SERVICE feature is not implemented"))
    }
}

/// Options for SPARQL query parsing and evaluation like the query base IRI
pub struct QueryOptions<'a> {
    pub(crate) base_iri: Option<&'a str>,
    pub(crate) default_graph_as_union: bool,
    pub(crate) service_handler: Box<dyn ServiceHandler>,
}

impl<'a> Default for QueryOptions<'a> {
    fn default() -> Self {
        Self {
            base_iri: None,
            default_graph_as_union: false,
            service_handler: Box::new(EmptyServiceHandler),
        }
    }
}

impl<'a> QueryOptions<'a> {
    /// Allows setting the base IRI of the query
    pub fn with_base_iri(mut self, base_iri: &'a str) -> Self {
        self.base_iri = Some(base_iri);
        self
    }

    /// Consider the union of all graphs in the store as the default graph
    pub const fn with_default_graph_as_union(mut self) -> Self {
        self.default_graph_as_union = true;
        self
    }

    pub fn with_service_handler(mut self, service_handler: impl ServiceHandler + 'static) -> Self {
        self.service_handler = Box::new(service_handler);
        self
    }
}
