//! This crate provides [SPARQL 1.1](http://www.w3.org/TR/sparql11-overview/) query and update parsers.
//! The emitted tree is based on [SPARQL 1.1 Query Algebra](https://www.w3.org/TR/sparql11-query/#sparqlQuery) objects.
//!
//! The API entry point for SPARQL queries is [`Query`] and the API entry point for SPARQL updates is [`Update`].
//!
//! This crate is intended to be a building piece for SPARQL implementations in Rust like [Oxigraph](https://oxigraph.org).
//!
//! Support for [SPARQL-star](https://w3c.github.io/rdf-star/cg-spec/) is available behind the `rdf-star` feature.
//!
//! Usage example:
//! ```
//! use spargebra::Query;
//!
//! let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
//! let query = Query::parse(query_str, None)?;
//! assert_eq!(query.to_string(), query_str);
//! # Result::Ok::<_, spargebra::ParseError>(())
//! ```
#![deny(
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_qualifications
)]
#![doc(test(attr(deny(warnings))))]

pub mod algebra;
mod parser;
mod query;
pub mod term;
mod update;

pub use parser::ParseError;
pub use query::*;
pub use update::*;
