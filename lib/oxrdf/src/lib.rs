#![doc = include_str!("../README.md")]
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
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/master/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/master/logo.svg")]

mod blank_node;
pub mod dataset;
pub mod graph;
mod interning;
mod literal;
mod named_node;
mod parser;
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
pub use crate::variable::{Variable, VariableNameParseError, VariableRef};
pub use oxilangtag::LanguageTagParseError;
pub use oxiri::IriParseError;
