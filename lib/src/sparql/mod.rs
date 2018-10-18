//! SPARQL 1.1 implementation.
//! This is a work in progress!!!

use model::Dataset;
use sparql::algebra::Query;
use sparql::algebra::QueryResult;
use sparql::eval::SimpleEvaluator;
use sparql::parser::read_sparql_query;
use std::io::Read;
use std::sync::Arc;
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
        Ok(SimplePreparedQuery {
            query: read_sparql_query(query, None)?,
            store: self.encoded(),
        })
    }
}

pub struct SimplePreparedQuery<S: EncodedQuadsStore> {
    query: Query,
    store: Arc<S>,
}

impl<S: EncodedQuadsStore> PreparedQuery for SimplePreparedQuery<S> {
    fn exec(&self) -> Result<QueryResult> {
        SimpleEvaluator::new(self.store.clone()).evaluate(&self.query)
    }
}
