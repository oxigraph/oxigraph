#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod csv;
mod error;
mod format;
mod json;
mod parser;
mod serializer;
pub mod solution;
mod xml;

pub use crate::error::{ParseError, SyntaxError, TextPosition};
pub use crate::format::QueryResultsFormat;
pub use crate::parser::{FromReadQueryResultsReader, FromReadSolutionsReader, QueryResultsParser};
pub use crate::serializer::{QueryResultsSerializer, ToWriteSolutionsWriter};
pub use crate::solution::QuerySolution;
