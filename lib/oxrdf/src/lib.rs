//! OxRDF is a simple library providing datastructures encoding [RDF 1.1 concepts](https://www.w3.org/TR/rdf11-concepts/).
//!
//! This crate is intended to be a basic building block of other crates like [Oxigraph](https://crates.io/crates/oxigraph) or [Spargebra](https://crates.io/crates/spargebra).
//!
//! Support for [RDF-star](https://w3c.github.io/rdf-star/cg-spec/) is available behind the `rdf-star` feature.
//!
//! Inspired by [RDF/JS](https://rdf.js.org/data-model-spec/) and [Apache Commons RDF](http://commons.apache.org/proper/commons-rdf/).
//!
//! Usage example:
//!
//! Usage example:
//! ```
//! use oxrdf::*;
//!
//! let mut graph = Graph::default();
//!
//! // insertion
//! let ex = NamedNodeRef::new("http://example.com")?;
//! let triple = TripleRef::new(ex, ex, ex);
//! graph.insert(triple);
//!
//! // simple filter
//! let results: Vec<_> = graph.triples_for_subject(ex).collect();
//! assert_eq!(vec![triple], results);
//! # Result::<_,Box<dyn std::error::Error>>::Ok(())
//! ```

mod blank_node;
pub mod dataset;
pub mod graph;
mod interning;
mod literal;
mod named_node;
mod parser;
#[cfg(feature = "sophia_api")]
mod sophia;
mod triple;
mod variable;
pub mod vocab;

pub use crate::blank_node::{BlankNode, BlankNodeIdParseError, BlankNodeRef};
pub use crate::dataset::Dataset;
pub use crate::graph::Graph;
pub use crate::literal::{Literal, LiteralRef};
pub use crate::named_node::{NamedNode, NamedNodeRef};
pub use crate::parser::TermParseError;
pub use crate::triple::{
    GraphName, GraphNameRef, NamedOrBlankNode, NamedOrBlankNodeRef, Quad, QuadRef, Subject,
    SubjectRef, Term, TermRef, Triple, TripleRef,
};
pub use crate::variable::{Variable, VariableNameParseError};
pub use oxilangtag::LanguageTagParseError;
pub use oxiri::IriParseError;
