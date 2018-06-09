///! Implements data structures for https://www.w3.org/TR/rdf11-concepts/
///! Inspired by [RDFjs](http://rdf.js.org/)
mod blank_node;
mod literal;
mod named_node;
mod triple;
pub mod vocab;

pub use model::blank_node::BlankNode;
pub use model::literal::Literal;
pub use model::named_node::NamedNode;
pub use model::triple::NamedOrBlankNode;
pub use model::triple::Quad;
pub use model::triple::QuadLike;
pub use model::triple::Term;
pub use model::triple::Triple;
pub use model::triple::TripleLike;
