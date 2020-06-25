//! Implementation of [W3C RDF tests](http://w3c.github.io/rdf-tests/) to tests Oxigraph conformance.
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

pub mod files;
pub mod manifest;
pub mod parser_evaluator;
pub mod report;
pub mod sparql_evaluator;
mod vocab;
