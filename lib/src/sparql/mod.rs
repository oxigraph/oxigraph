//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.

mod algebra;
mod eval;
mod model;
mod parser;
mod plan;
mod plan_builder;
mod xml_results;

use crate::sparql::algebra::QueryVariants;
use crate::sparql::eval::SimpleEvaluator;
use crate::sparql::parser::read_sparql_query;
use crate::sparql::plan::PlanNode;
use crate::sparql::plan::TripleTemplate;
use crate::sparql::plan_builder::PlanBuilder;
use crate::store::StoreConnection;
use crate::Result;
use std::fmt;
use std::io::Read;

pub use crate::sparql::model::BindingsIterator;
pub use crate::sparql::model::QueryResult;
pub use crate::sparql::model::QueryResultSyntax;
pub use crate::sparql::model::Variable;

/// A prepared [SPARQL query](https://www.w3.org/TR/sparql11-query/)
pub trait PreparedQuery {
    /// Evaluates the query and returns its results
    fn exec(&self) -> Result<QueryResult<'_>>;
}

/// An implementation of `PreparedQuery` for internal use
pub struct SimplePreparedQuery<S: StoreConnection>(SimplePreparedQueryOptions<S>);

enum SimplePreparedQueryOptions<S: StoreConnection> {
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

impl<S: StoreConnection> SimplePreparedQuery<S> {
    pub(crate) fn new(connection: S, query: impl Read) -> Result<Self> {
        Ok(Self(match read_sparql_query(query, None)? {
            QueryVariants::Select {
                algebra,
                dataset: _,
            } => {
                let (plan, variables) = PlanBuilder::build(&connection, &algebra)?;
                SimplePreparedQueryOptions::Select {
                    plan,
                    variables,
                    evaluator: SimpleEvaluator::new(connection),
                }
            }
            QueryVariants::Ask {
                algebra,
                dataset: _,
            } => {
                let (plan, _) = PlanBuilder::build(&connection, &algebra)?;
                SimplePreparedQueryOptions::Ask {
                    plan,
                    evaluator: SimpleEvaluator::new(connection),
                }
            }
            QueryVariants::Construct {
                construct,
                algebra,
                dataset: _,
            } => {
                let (plan, variables) = PlanBuilder::build(&connection, &algebra)?;
                SimplePreparedQueryOptions::Construct {
                    plan,
                    construct: PlanBuilder::build_graph_template(
                        &connection,
                        &construct,
                        variables,
                    )?,
                    evaluator: SimpleEvaluator::new(connection),
                }
            }
            QueryVariants::Describe {
                algebra,
                dataset: _,
            } => {
                let (plan, _) = PlanBuilder::build(&connection, &algebra)?;
                SimplePreparedQueryOptions::Describe {
                    plan,
                    evaluator: SimpleEvaluator::new(connection),
                }
            }
        }))
    }
}

impl<S: StoreConnection> PreparedQuery for SimplePreparedQuery<S> {
    fn exec(&self) -> Result<QueryResult<'_>> {
        match &self.0 {
            SimplePreparedQueryOptions::Select {
                plan,
                variables,
                evaluator,
            } => evaluator.evaluate_select_plan(&plan, &variables),
            SimplePreparedQueryOptions::Ask { plan, evaluator } => {
                evaluator.evaluate_ask_plan(&plan)
            }
            SimplePreparedQueryOptions::Construct {
                plan,
                construct,
                evaluator,
            } => evaluator.evaluate_construct_plan(&plan, &construct),
            SimplePreparedQueryOptions::Describe { plan, evaluator } => {
                evaluator.evaluate_describe_plan(&plan)
            }
        }
    }
}

/// A parsed [SPARQL query](https://www.w3.org/TR/sparql11-query/)
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct Query(QueryVariants);

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Query {
    /// Parses a SPARQL query
    pub fn read<'a>(reader: impl Read + 'a, base_iri: Option<&'a str>) -> Result<Self> {
        Ok(Query(read_sparql_query(reader, base_iri)?))
    }
}
