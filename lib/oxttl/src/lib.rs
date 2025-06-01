#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod chunker;
mod lexer;
mod line_formats;
pub mod n3;
pub mod nquads;
pub mod ntriples;
mod terse;
mod toolkit;
pub mod trig;
pub mod turtle;

pub use crate::n3::N3Parser;
pub use crate::nquads::{NQuadsParser, NQuadsSerializer};
pub use crate::ntriples::{NTriplesParser, NTriplesSerializer};
pub use crate::toolkit::{TextPosition, TurtleParseError, TurtleSyntaxError};
pub use crate::trig::{TriGParser, TriGSerializer};
pub use crate::turtle::{TurtleParser, TurtleSerializer};

pub(crate) const MIN_BUFFER_SIZE: usize = 4096;
pub(crate) const MAX_BUFFER_SIZE: usize = 4096 * 4096;
#[expect(clippy::decimal_literal_representation)]
pub(crate) const MIN_PARALLEL_CHUNK_SIZE: usize = 16384;
