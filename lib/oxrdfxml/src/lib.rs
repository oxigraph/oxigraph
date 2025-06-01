#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod error;
mod parser;
mod serializer;
mod utils;

pub use error::{RdfXmlParseError, RdfXmlSyntaxError};
#[cfg(feature = "async-tokio")]
pub use parser::TokioAsyncReaderRdfXmlParser;
pub use parser::{RdfXmlParser, RdfXmlPrefixesIter, ReaderRdfXmlParser, SliceRdfXmlParser};
#[cfg(feature = "async-tokio")]
pub use serializer::TokioAsyncWriterRdfXmlSerializer;
pub use serializer::{RdfXmlSerializer, WriterRdfXmlSerializer};
