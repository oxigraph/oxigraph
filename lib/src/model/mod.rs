//! Implements data structures for [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/).
//!
//! Inspired by [RDF/JS](https://rdf.js.org/data-model-spec/) and [Apache Commons RDF](http://commons.apache.org/proper/commons-rdf/)

mod blank_node;
pub mod dataset;
pub mod graph;
mod interning;
mod literal;
mod named_node;
mod parser;
#[cfg(feature = "sophia")]
mod sophia;
mod triple;
pub mod vocab;
pub(crate) mod xsd;

pub use self::blank_node::{BlankNode, BlankNodeIdParseError, BlankNodeRef};
pub use self::dataset::Dataset;
pub use self::graph::Graph;
pub use self::literal::{Literal, LiteralRef};
pub use self::named_node::{NamedNode, NamedNodeRef};
pub use self::parser::TermParseError;
pub use self::triple::{
    GraphName, GraphNameRef, NamedOrBlankNode, NamedOrBlankNodeRef, Quad, QuadRef, Term, TermRef,
    Triple, TripleRef,
};
pub use oxilangtag::LanguageTagParseError;
pub use oxiri::IriParseError;
