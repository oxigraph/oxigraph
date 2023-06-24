#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod error;
mod parser;
mod serializer;
mod utils;

pub use crate::serializer::{RdfXmlSerializer, ToWriteRdfXmlWriter};
pub use error::RdfXmlError;
pub use parser::{FromReadRdfXmlReader, RdfXmlParser};
