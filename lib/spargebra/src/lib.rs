#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

pub mod algebra;
mod algebra_builder;
mod ast;
mod lexer;
mod parser;
mod parser3;
mod query;
pub mod term;
mod update;

pub use parser::{SparqlParser, SparqlSyntaxError};
pub use query::*;
pub use update::*;
