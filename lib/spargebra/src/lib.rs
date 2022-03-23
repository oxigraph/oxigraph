#![doc = include_str!("../README.md")]
#![deny(unsafe_code)]
#![doc(test(attr(deny(warnings))))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

pub mod algebra;
mod parser;
mod query;
pub mod term;
mod update;

pub use parser::ParseError;
pub use query::*;
pub use update::*;
