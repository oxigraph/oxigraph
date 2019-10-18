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
use failure::format_err;
use rio_api::iri::Iri;
use std::fmt;

pub use crate::sparql::algebra::GraphPattern;
pub use crate::sparql::model::BindingsIterator;
pub use crate::sparql::model::QueryResult;
pub use crate::sparql::model::QueryResultSyntax;
pub use crate::sparql::model::Variable;

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/)
pub trait PreparedQuery {
    /// Evaluates the query and returns its results
    fn exec(&self) -> Result<QueryResult>;
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
    pub(crate) fn new(connection: S, query: &str, options: QueryOptions) -> Result<Self> {
        let dataset = DatasetView::new(connection, options.default_graph_as_union);
        Ok(Self(match read_sparql_query(query, options.base_iri)? {
            QueryVariants::Select {
                algebra,
                dataset: _,
                base_iri,
            } => {
                let (plan, variables) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Select {
                    plan,
                    variables,
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
            QueryVariants::Ask {
                algebra,
                dataset: _,
                base_iri,
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
                dataset: _,
                base_iri,
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
                algebra,
                dataset: _,
                base_iri,
            } => {
                let (plan, _) = PlanBuilder::build(dataset.encoder(), &algebra)?;
                SimplePreparedQueryAction::Describe {
                    plan,
                    evaluator: SimpleEvaluator::new(dataset, base_iri, options.service_handler),
                }
            }
        }))
    }

    /// Builds SimplePreparedQuery from an existing `GraphPattern`. This is used to support federated queries via `SERVICE` clauses
    pub(crate) fn new_from_pattern(
        connection: S,
        pattern: &GraphPattern,
        options: QueryOptions,
    ) -> Result<Self> {
        let dataset = DatasetView::new(connection, options.default_graph_as_union);
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
}

impl<S: StoreConnection> PreparedQuery for SimplePreparedQuery<S> {
    fn exec(&self) -> Result<QueryResult> {
        match &self.0 {
            SimplePreparedQueryAction::Select {
                plan,
                variables,
                evaluator,
            } => evaluator.evaluate_select_plan(&plan, &variables),
            SimplePreparedQueryAction::Ask { plan, evaluator } => {
                evaluator.evaluate_ask_plan(&plan)
            }
            SimplePreparedQueryAction::Construct {
                plan,
                construct,
                evaluator,
            } => evaluator.evaluate_construct_plan(&plan, &construct),
            SimplePreparedQueryAction::Describe { plan, evaluator } => {
                evaluator.evaluate_describe_plan(&plan)
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
    ) -> Result<BindingsIterator<'a>>;
}

impl<F: for<'a> Fn(&NamedNode, &'a GraphPattern) -> Result<BindingsIterator<'a>>> ServiceHandler
    for F
{
    fn handle<'a>(
        &'a self,
        service_name: &NamedNode,
        graph_pattern: &'a GraphPattern,
    ) -> Result<BindingsIterator<'a>> {
        self(service_name, graph_pattern)
    }
}

struct EmptyServiceHandler;

impl ServiceHandler for EmptyServiceHandler {
    fn handle<'a>(&'a self, _: &NamedNode, _: &'a GraphPattern) -> Result<BindingsIterator<'a>> {
        Err(format_err!("The SERVICE feature is not implemented"))
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
        self.service_handler = service_handler;
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
