//! Oxigraph is a work in progress graph database implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.
//!
//! Its goal is to provide a compliant, safe and fast graph database.
//!
//! It currently provides two `Repository` implementation providing [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) capability:
//! * `MemoryRepository`: a simple in memory implementation.
//! * `RocksDbRepository`: a file system implementation based on the [RocksDB](https://rocksdb.org/) key-value store.
//!
//! Usage example with the `MemoryRepository`:
//!
//! ```
//! use oxigraph::model::*;
//! use oxigraph::{Repository, RepositoryConnection, RepositoryTransaction, MemoryRepository, Result};
//! use crate::oxigraph::sparql::{PreparedQuery, QueryOptions};
//! use oxigraph::sparql::QueryResult;
//!
//! let repository = MemoryRepository::default();
//! let mut connection = repository.connection()?;
//!
//! // insertion
//! let ex = NamedNode::parse("http://example.com")?;
//! let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
//! connection.insert(&quad)?;
//!
//! // quad filter
//! let results: Result<Vec<Quad>> = connection.quads_for_pattern(None, None, None, None).collect();
//! assert_eq!(vec![quad], results?);
//!
//! // SPARQL query
//! let prepared_query = connection.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
//! let results = prepared_query.exec()?;
//! if let QueryResult::Bindings(results) = results {
//!     assert_eq!(results.into_values_iter().next().unwrap()?[0], Some(ex.into()));
//! }
//! # Result::Ok(())
//! ```

pub mod model;
mod repository;
pub mod sparql;
pub(crate) mod store;
mod syntax;

pub use failure::Error;
pub type Result<T> = ::std::result::Result<T, failure::Error>;
pub use crate::repository::Repository;
pub use crate::repository::RepositoryConnection;
pub use crate::repository::RepositoryTransaction;
pub use crate::store::MemoryRepository;
#[cfg(feature = "rocksdb")]
pub use crate::store::RocksDbRepository;
pub use crate::syntax::DatasetSyntax;
pub use crate::syntax::FileSyntax;
pub use crate::syntax::GraphSyntax;
