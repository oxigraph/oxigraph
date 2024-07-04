#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod error;
mod format;
mod parser;
mod serializer;

pub use error::{RdfParseError, RdfSyntaxError, TextPosition};
pub use format::RdfFormat;
#[cfg(feature = "async-tokio")]
pub use parser::FromTokioAsyncReadQuadReader;
pub use parser::{FromReadQuadReader, FromSliceQuadReader, RdfParser};
#[cfg(feature = "async-tokio")]
pub use serializer::ToTokioAsyncWriteQuadWriter;
pub use serializer::{RdfSerializer, ToWriteQuadWriter};
