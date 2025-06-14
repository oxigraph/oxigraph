#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

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
#[cfg(feature = "rdf-12")]
pub use crate::literal::BaseDirection;
pub use crate::literal::{Literal, LiteralRef};
pub use crate::named_node::{NamedNode, NamedNodeRef};
pub use crate::parser::TermParseError;
pub use crate::triple::{
    GraphName, GraphNameRef, NamedOrBlankNode, NamedOrBlankNodeRef, Quad, QuadRef, Term, TermRef,
    Triple, TripleRef, TryFromTermError,
};
pub use crate::variable::{Variable, VariableNameParseError, VariableRef};
pub use oxilangtag::LanguageTagParseError;
pub use oxiri::IriParseError;
#[deprecated(note = "Use `NamedOrBlankNode` instead", since = "0.5.0")]
pub type Subject = NamedOrBlankNode;
#[deprecated(note = "Use `NamedOrBlankNodeRef` instead", since = "0.5.0")]
pub type SubjectRef<'a> = NamedOrBlankNodeRef<'a>;
