//! Utilities to read and write RDF graphs and datasets using [OxRDF I/O](https://crates.io/crates/oxrdfio).
//!
//! The entry points of this module are the two [`RdfParser`] and [`RdfSerializer`] structs.
//!
//! Usage example converting a Turtle file to a N-Triples file:
//! ```
//! use oxigraph::io::{RdfFormat, RdfParser, RdfSerializer};
//!
//! let turtle_file = b"@base <http://example.com/> .
//! @prefix schema: <http://schema.org/> .
//! <foo> a schema:Person ;
//!     schema:name \"Foo\" .
//! <bar> a schema:Person ;
//!     schema:name \"Bar\" .";
//!
//! let ntriples_file = b"<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
//! <http://example.com/foo> <http://schema.org/name> \"Foo\" .
//! <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
//! <http://example.com/bar> <http://schema.org/name> \"Bar\" .
//! ";
//!
//! let mut writer = RdfSerializer::from_format(RdfFormat::NTriples).serialize_to_write(Vec::new());
//! for quad in RdfParser::from_format(RdfFormat::Turtle).parse_read(turtle_file.as_ref()) {
//!     writer.write_quad(&quad.unwrap()).unwrap();
//! }
//! assert_eq!(writer.finish().unwrap(), ntriples_file);
//! ```

mod format;
pub mod read;
pub mod write;

#[allow(deprecated)]
pub use self::format::{DatasetFormat, GraphFormat};
#[allow(deprecated)]
pub use self::read::{DatasetParser, GraphParser};
#[allow(deprecated)]
pub use self::write::{DatasetSerializer, GraphSerializer};
pub use oxrdfio::*;
