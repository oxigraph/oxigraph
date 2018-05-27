extern crate reqwest;
extern crate rudf;
extern crate url;

use reqwest::Client;
use reqwest::Response;
use rudf::rio::ntriples::read_ntriples;
use rudf::rio::turtle::read_turtle;
use rudf::rio::RioError;
use rudf::rio::RioResult;
use rudf::sparql::ast::Query;
use rudf::sparql::parser::read_sparql_query;
use rudf::store::memory::MemoryGraph;
use std::error::Error;
use url::Url;

pub struct RDFClient {
    client: Client,
}

impl Default for RDFClient {
    fn default() -> Self {
        Self {
            client: Client::new(),
        }
    }
}

impl RDFClient {
    pub fn load_turtle(&self, url: Url) -> RioResult<MemoryGraph> {
        Ok(read_turtle(self.get(&url)?, Some(url))?.collect())
    }

    pub fn load_ntriples(&self, url: Url) -> RioResult<MemoryGraph> {
        read_ntriples(self.get(&url)?).collect()
    }

    pub fn load_sparql_query(&self, url: Url) -> RioResult<Query> {
        read_sparql_query(self.get(&url)?, Some(url))
    }

    fn get(&self, url: &Url) -> RioResult<Response> {
        match self.client.get(url.clone()).send() {
            Ok(response) => Ok(response),
            Err(error) => if error.description() == "message is incomplete" {
                self.get(url)
            } else {
                Err(RioError::new(error))
            },
        }
    }
}
