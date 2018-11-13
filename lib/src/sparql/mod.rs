//! SPARQL 1.1 implementation.
//! This is a work in progress!!!

use model::Dataset;
use sparql::algebra::Query;
use sparql::algebra::QueryResult;
use sparql::algebra::Variable;
use sparql::eval::SimpleEvaluator;
use sparql::parser::read_sparql_query;
use sparql::plan::PlanBuilder;
use sparql::plan::PlanNode;
use sparql::plan::TripleTemplate;
use std::io::Read;
use store::encoded::EncodedQuadsStore;
use store::encoded::StoreDataset;
use Result;

pub mod algebra;
mod eval;
pub mod parser;
mod plan;
pub mod xml_results;

pub trait SparqlDataset: Dataset {
    type PreparedQuery: PreparedQuery;
    fn prepare_query(&self, query: impl Read) -> Result<Self::PreparedQuery>;
}

pub trait PreparedQuery {
    fn exec(&self) -> Result<QueryResult>;
}

impl<S: EncodedQuadsStore> SparqlDataset for StoreDataset<S> {
    type PreparedQuery = SimplePreparedQuery<S>;

    fn prepare_query(&self, query: impl Read) -> Result<SimplePreparedQuery<S>> {
        Ok(match read_sparql_query(query, None)? {
            Query::Select { algebra, dataset } => {
                let store = self.encoded();
                let (plan, variables) = PlanBuilder::build(&*store, &algebra)?;
                SimplePreparedQuery::Select {
                    plan,
                    variables,
                    evaluator: SimpleEvaluator::new(store),
                }
            }
            Query::Ask { algebra, dataset } => {
                let store = self.encoded();
                let (plan, _) = PlanBuilder::build(&*store, &algebra)?;
                SimplePreparedQuery::Ask {
                    plan,
                    evaluator: SimpleEvaluator::new(store),
                }
            }
            Query::Construct {
                construct,
                algebra,
                dataset,
            } => {
                let store = self.encoded();
                let (plan, variables) = PlanBuilder::build(&*store, &algebra)?;
                SimplePreparedQuery::Construct {
                    plan,
                    construct: PlanBuilder::build_graph_template(&*store, &construct, variables)?,
                    evaluator: SimpleEvaluator::new(store),
                }
            }
            _ => unimplemented!(),
        })
    }
}

pub enum SimplePreparedQuery<S: EncodedQuadsStore> {
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
}

impl<S: EncodedQuadsStore> PreparedQuery for SimplePreparedQuery<S> {
    fn exec(&self) -> Result<QueryResult> {
        match self {
            SimplePreparedQuery::Select {
                plan,
                variables,
                evaluator,
            } => evaluator.evaluate_select_plan(&plan, &variables),
            SimplePreparedQuery::Ask { plan, evaluator } => evaluator.evaluate_ask_plan(&plan),
            SimplePreparedQuery::Construct {
                plan,
                construct,
                evaluator,
            } => evaluator.evaluate_construct_plan(&plan, &construct),
        }
    }
}
