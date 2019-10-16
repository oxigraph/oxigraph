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
use crate::sparql::parser::read_sparql_query;
use crate::sparql::plan::TripleTemplate;
use crate::sparql::plan::{DatasetView, PlanNode};
use crate::sparql::plan_builder::PlanBuilder;
use crate::store::StoreConnection;
use crate::Result;
use std::fmt;

pub use crate::sparql::algebra::GraphPattern;
pub use crate::sparql::model::BindingsIterator;
pub use crate::sparql::model::QueryResult;
pub use crate::sparql::model::QueryResultSyntax;
pub use crate::sparql::model::Variable;

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/)
pub trait PreparedQuery {
    /// Evaluates the query and returns its results
    fn exec<'a>(&'a self, options: &'a QueryOptions<'a>) -> Result<QueryResult<'a>>;
}

/// An implementation of `PreparedQuery` for internal use
pub struct SimplePreparedQuery<S: StoreConnection>(SimplePreparedQueryAction<S>);

enum SimplePreparedQueryAction<S: StoreConnection> {
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

impl<'a, S: StoreConnection + 'a> SimplePreparedQuery<S> {
    pub(crate) fn new(connection: S, query: &str, base_iri: Option<&'a str>) -> Result<Self> {
        let dataset = DatasetView::new(connection);
        //TODO avoid inserting terms in the Repository StringStore
        Ok(Self(match read_sparql_query(query, base_iri)? {
            QueryVariants::Select {
                algebra,
                dataset: _,
                ..
            } => {
                let (plan, variables) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Select {
                    plan,
                    variables,
                    evaluator: SimpleEvaluator::new(dataset),
                }
            }
            QueryVariants::Ask {
                algebra,
                dataset: _,
                ..
            } => {
                let (plan, _) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Ask {
                    plan,
                    evaluator: SimpleEvaluator::new(dataset),
                }
            }
            QueryVariants::Construct {
                construct,
                algebra,
                dataset: _,
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
                    evaluator: SimpleEvaluator::new(dataset),
                }
            }
            QueryVariants::Describe {
                algebra,
                dataset: _,
                ..
            } => {
                let (plan, _) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Describe {
                    plan,
                    evaluator: SimpleEvaluator::new(dataset),
                }
            }
        }))
    }

    pub(crate) fn new_from_pattern(
        connection: S,
        pattern: &GraphPattern,
    ) -> Result<Self> {
        let dataset = DatasetView::new(connection);
        let (plan, variables) = PlanBuilder::build(dataset.encoder(), pattern)?;
        Ok(Self(SimplePreparedQueryAction::Select {
            plan,
            variables,
            evaluator: SimpleEvaluator::new(dataset),
        }))
    }
}

impl<S: StoreConnection> PreparedQuery for SimplePreparedQuery<S> {
    fn exec<'a>(&'a self, options: &'a QueryOptions<'a>) -> Result<QueryResult<'a>> {
        match &self.0 {
            SimplePreparedQueryAction::Select {
                plan,
                variables,
                evaluator,
            } => evaluator.evaluate_select_plan(&plan, &variables, options),
            SimplePreparedQueryAction::Ask { plan, evaluator } => {
                evaluator.evaluate_ask_plan(&plan, options)
            }
            SimplePreparedQueryAction::Construct {
                plan,
                construct,
                evaluator,
            } => evaluator.evaluate_construct_plan(&plan, &construct, &options),
            SimplePreparedQueryAction::Describe { plan, evaluator } => {
                evaluator.evaluate_describe_plan(&plan, &options)
            }
        }
    }
}

pub trait ServiceHandler {
    fn handle<'a>(&'a self, node: NamedNode) -> Option<(fn(GraphPattern) -> Result<BindingsIterator<'a>>)>;
}

/// Options for SPARQL query parsing and evaluation like the query base IRI
pub struct QueryOptions<'a> {
    pub(crate) base_iri: Option<&'a str>,
    pub(crate) default_graph_as_union: bool,
    pub(crate) service_handler: Option<Box<dyn ServiceHandler>>,
}

impl<'a> Default for QueryOptions<'a> {
    fn default() -> Self {
        Self {
            base_iri: None,
            default_graph_as_union: false,
            service_handler: None as Option<Box<dyn ServiceHandler>>,
        }
    }
}

impl<'a> QueryOptions<'a> {
    /// Allows to set the base IRI of the query
    pub fn with_base_iri(mut self, base_iri: &'a str) -> Self {
        self.base_iri = Some(base_iri);
        self
    }

    /// Consider the union of all graphs in the repository as the default graph
    pub fn with_default_graph_as_union(mut self) -> Self {
        self.default_graph_as_union = true;
        self
    }

    /// Consider the union of all graphs in the repository as the default graph
    pub fn with_service_handler(mut self, service_handler: Box<dyn ServiceHandler>) -> Self {
        self.service_handler = Some(service_handler);
        self
    }
}

/// A parsed [SPARQL query](https://www.w3.org/TR/sparql11-query/)
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Query(QueryVariants);

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Query {
    /// Parses a SPARQL query
    pub fn parse(query: &str, base_iri: Option<&str>) -> Result<Self> {
        Ok(Query(read_sparql_query(query, base_iri)?))
    }
}
