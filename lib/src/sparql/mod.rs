//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.

use crate::sparql::algebra::Query;
use crate::sparql::algebra::QueryResult;
use crate::sparql::algebra::Variable;
use crate::sparql::eval::SimpleEvaluator;
use crate::sparql::parser::read_sparql_query;
use crate::sparql::plan::PlanBuilder;
use crate::sparql::plan::PlanNode;
use crate::sparql::plan::TripleTemplate;
use crate::store::StoreConnection;
use crate::Result;
use std::io::Read;

pub mod algebra;
mod eval;
pub mod parser;
mod plan;
pub mod xml_results;

/// A prepared [SPARQL 1.1](https://www.w3.org/TR/sparql11-query/) query
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
            Query::Select {
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
            Query::Ask {
                algebra,
                dataset: _,
            } => {
                let (plan, _) = PlanBuilder::build(&connection, &algebra)?;
                SimplePreparedQueryOptions::Ask {
                    plan,
                    evaluator: SimpleEvaluator::new(connection),
                }
            }
            Query::Construct {
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
            Query::Describe {
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
