//! SPARQL 1.1 implementation.
//! This is a work in progress!!!

use model::Dataset;
use sparql::algebra::QueryResult;
use sparql::eval::SparqlEvaluator;
use sparql::parser::read_sparql_query;
use std::io::Read;
use store::store::EncodedQuadsStore;
use store::store::StoreDataset;
use Result;

pub mod algebra;
mod eval;
pub mod parser;
pub mod xml_results;

pub trait SparqlDataset: Dataset {
    fn query(&self, query: impl Read) -> Result<QueryResult>;
}

impl<S: EncodedQuadsStore> SparqlDataset for StoreDataset<S> {
    fn query(&self, query: impl Read) -> Result<QueryResult> {
        let query = read_sparql_query(query, None)?;
        SparqlEvaluator::new(self.encoded()).evaluate(&query)
    }
}
