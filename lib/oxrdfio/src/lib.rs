#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod document;
mod error;
mod format;
mod parser;
mod serializer;

pub use document::LoadedDocument;
pub use error::{RdfParseError, RdfSyntaxError, TextPosition};
pub use format::RdfFormat;
pub use oxjsonld::{JsonLdProfile, JsonLdProfileSet};
#[cfg(feature = "async-tokio")]
pub use parser::TokioAsyncReaderQuadParser;
pub use parser::{RdfParser, ReaderQuadParser, SliceQuadParser};
#[cfg(feature = "async-tokio")]
pub use serializer::TokioAsyncWriterQuadSerializer;
pub use serializer::{RdfSerializer, WriterQuadSerializer};
