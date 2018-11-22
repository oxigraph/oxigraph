//! [SPARQL](https://www.w3.org/TR/sparql11-overview/) implementation.
//!
//! The support of RDF Dataset and SPARQL 1.1 specific features is not done yet.
//!
//! This module adds query capabilities to the `rudf::model::Dataset` implementations.
//!
//! Usage example:
//! ```
//! use rudf::model::*;
//! use rudf::store::MemoryDataset;
//! use rudf::sparql::SparqlDataset;
//! use rudf::sparql::PreparedQuery;
//! use rudf::sparql::algebra::QueryResult;
//! use std::str::FromStr;
//!
//! let dataset = MemoryDataset::default();
//! let ex = NamedNode::from_str("http://example.com").unwrap();
//! dataset.insert(&Quad::new(ex.clone(), ex.clone(), ex.clone(), None));
//! let prepared_query = dataset.prepare_query("SELECT ?s WHERE { ?s ?p ?o }".as_bytes()).unwrap();
//! let results = prepared_query.exec().unwrap();
//! if let QueryResult::Bindings(results) = results {
//!     assert_eq!(results.into_values_iter().next().unwrap().unwrap()[0], Some(ex.into()));
//! }
//! ```

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

/// An extension of the `rudf::model::Dataset` trait to allow SPARQL operations on it.
///
/// It is implemented by all stores provided by Rudf
pub trait SparqlDataset: Dataset {
    type PreparedQuery: PreparedQuery;

    /// Prepares a [SPARQL 1.1](https://www.w3.org/TR/sparql11-query/) query and returns an object that could be used to execute it
    ///
    /// The implementation is a work in progress, RDF Dataset and SPARQL 1.1 specific features are not implemented yet.
    fn prepare_query(&self, query: impl Read) -> Result<Self::PreparedQuery>;
}

/// A prepared [SPARQL 1.1](https://www.w3.org/TR/sparql11-query/) query
pub trait PreparedQuery {
    /// Evaluates the query and returns its results
    fn exec(&self) -> Result<QueryResult>;
}

impl<S: EncodedQuadsStore> SparqlDataset for StoreDataset<S> {
    type PreparedQuery = SimplePreparedQuery<S>;

    fn prepare_query(&self, query: impl Read) -> Result<SimplePreparedQuery<S>> {
        Ok(SimplePreparedQuery(match read_sparql_query(query, None)? {
            Query::Select { algebra, dataset } => {
                let store = self.encoded();
                let (plan, variables) = PlanBuilder::build(&*store, &algebra)?;
                SimplePreparedQueryOptions::Select {
                    plan,
                    variables,
                    evaluator: SimpleEvaluator::new(store),
                }
            }
            Query::Ask { algebra, dataset } => {
                let store = self.encoded();
                let (plan, _) = PlanBuilder::build(&*store, &algebra)?;
                SimplePreparedQueryOptions::Ask {
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
                SimplePreparedQueryOptions::Construct {
                    plan,
                    construct: PlanBuilder::build_graph_template(&*store, &construct, variables)?,
                    evaluator: SimpleEvaluator::new(store),
                }
            }
            Query::Describe { algebra, dataset } => {
                let store = self.encoded();
                let (plan, _) = PlanBuilder::build(&*store, &algebra)?;
                SimplePreparedQueryOptions::Describe {
                    plan,
                    evaluator: SimpleEvaluator::new(store),
                }
            }
        }))
    }
}

/// An implementation of `PreparedQuery` for internal use
pub struct SimplePreparedQuery<S: EncodedQuadsStore>(SimplePreparedQueryOptions<S>);

enum SimplePreparedQueryOptions<S: EncodedQuadsStore> {
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

impl<S: EncodedQuadsStore> PreparedQuery for SimplePreparedQuery<S> {
    fn exec(&self) -> Result<QueryResult> {
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
