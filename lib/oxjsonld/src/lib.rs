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

pub use crate::context::{LoadDocumentOptions, RemoteDocument};
pub use crate::error::{JsonLdErrorCode, JsonLdParseError, JsonLdSyntaxError, TextPosition};
#[cfg(feature = "async-tokio")]
pub use crate::from_rdf::TokioAsyncWriterJsonLdSerializer;
pub use crate::from_rdf::{JsonLdSerializer, WriterJsonLdSerializer};
#[cfg(feature = "async-tokio")]
pub use crate::to_rdf::TokioAsyncReaderJsonLdParser;
pub use crate::to_rdf::{JsonLdParser, JsonLdPrefixesIter, ReaderJsonLdParser, SliceJsonLdParser};

const MAX_CONTEXT_RECURSION: usize = 8;
