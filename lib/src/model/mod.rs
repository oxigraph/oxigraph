//! Implements data structures for [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/).
//!
//! Inspired by [RDF/JS](https://rdf.js.org/data-model-spec/) and [Apache Commons RDF](http://commons.apache.org/proper/commons-rdf/)

mod blank_node;
mod literal;
mod named_node;
mod triple;
pub mod vocab;
pub(crate) mod xsd;

pub use crate::model::blank_node::BlankNode;
pub use crate::model::blank_node::BlankNodeIdParseError;
pub use crate::model::literal::Literal;
pub use crate::model::named_node::NamedNode;
pub use crate::model::triple::NamedOrBlankNode;
pub use crate::model::triple::Quad;
pub use crate::model::triple::Term;
pub use crate::model::triple::Triple;
pub use oxilangtag::LanguageTagParseError;
pub use oxiri::IriParseError;
