#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod context;
mod error;
mod expansion;
mod from_rdf;
mod to_rdf;

pub use error::{JsonLdParseError, JsonLdSyntaxError};
#[cfg(feature = "async-tokio")]
pub use from_rdf::TokioAsyncWriterJsonLdSerializer;
pub use from_rdf::{JsonLdSerializer, WriterJsonLdSerializer};
#[cfg(feature = "async-tokio")]
pub use to_rdf::TokioAsyncReaderJsonLdParser;
pub use to_rdf::{JsonLdParser, JsonLdPrefixesIter, ReaderJsonLdParser, SliceJsonLdParser};
