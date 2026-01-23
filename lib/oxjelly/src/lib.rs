#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod jelly {
    include!(concat!(env!("OUT_DIR"), "/jelly-rdf/mod.rs"));
}

mod from_rdf;
mod to_rdf;
mod error;
mod sorted;

pub use error::{JellyParseError, JellySyntaxError};
pub use to_rdf::{JellyParser, JellyPrefixesIter, ReaderJellyParser, SliceJellyParser};
pub use from_rdf::{JellySerializer, WriterJellySerializer};