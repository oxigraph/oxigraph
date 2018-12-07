//! Implements data structures for [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/).
//!
//! Inspired by [RDFjs](http://rdf.js.org/) and [Apache Commons RDF](http://commons.apache.org/proper/commons-rdf/)

mod blank_node;
mod dataset;
mod literal;
mod named_node;
mod triple;
pub mod vocab;

pub use crate::model::blank_node::BlankNode;
pub use crate::model::dataset::Dataset;
pub use crate::model::dataset::Graph;
pub use crate::model::dataset::NamedGraph;
pub use crate::model::literal::Literal;
pub use crate::model::named_node::NamedNode;
pub use crate::model::triple::NamedOrBlankNode;
pub use crate::model::triple::Quad;
pub use crate::model::triple::Term;
pub use crate::model::triple::Triple;
