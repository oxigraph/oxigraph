#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod blank_node;
pub mod dataset;
pub mod graph;
mod literal;
mod named_node;
mod parser;
mod triple;
mod variable;
pub mod vocab;

pub use crate::blank_node::{BlankNode, BlankNodeIdParseError};
pub use crate::dataset::Dataset;
pub use crate::graph::Graph;
#[cfg(feature = "rdf-12")]
pub use crate::literal::BaseDirection;
pub use crate::literal::{Literal, MalformedLanguageTaggedLiteral};
pub use crate::named_node::NamedNode;
pub use crate::parser::TermParseError;
pub use crate::triple::{GraphName, NamedOrBlankNode, Quad, Term, Triple, TryFromTermError};
pub use crate::variable::{Variable, VariableNameParseError};
pub use oxilangtag::LanguageTagParseError;
pub use oxiri::IriParseError;
