//! Implements data structures for [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/)
//! Inspired by [RDFjs](http://rdf.js.org/) and [Apache Commons RDF](http://commons.apache.org/proper/commons-rdf/)

mod blank_node;
mod dataset;
mod literal;
mod named_node;
mod triple;
pub mod vocab;

pub use model::blank_node::BlankNode;
pub use model::dataset::Dataset;
pub use model::dataset::Graph;
pub use model::dataset::NamedGraph;
pub use model::literal::Literal;
pub use model::named_node::NamedNode;
pub use model::triple::NamedOrBlankNode;
pub use model::triple::Quad;
pub use model::triple::Term;
pub use model::triple::Triple;
