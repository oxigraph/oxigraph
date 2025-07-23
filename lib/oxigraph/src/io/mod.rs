//! Utilities to read and write RDF graphs and datasets using [OxRDF I/O](https://crates.io/crates/oxrdfio).
//!
//! The entry points of this module are the two [`RdfParser`] and [`RdfSerializer`] structs.
//!
//! Usage example converting a Turtle file to a N-Triples file:
//! ```
//! use oxigraph::io::{RdfFormat, RdfParser, RdfSerializer};
//!
//! let turtle_file = "@base <http://example.com/> .
//! @prefix schema: <http://schema.org/> .
//! <foo> a schema:Person ;
//!     schema:name \"Foo\" .
//! <bar> a schema:Person ;
//!     schema:name \"Bar\" .";
//!
//! let ntriples_file = "<http://example.com/foo> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
//! <http://example.com/foo> <http://schema.org/name> \"Foo\" .
//! <http://example.com/bar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://schema.org/Person> .
//! <http://example.com/bar> <http://schema.org/name> \"Bar\" .
//! ";
//!
//! let mut serializer = RdfSerializer::from_format(RdfFormat::NTriples).for_writer(Vec::new());
//! for quad in RdfParser::from_format(RdfFormat::Turtle).for_reader(turtle_file.as_bytes()) {
//!     serializer.serialize_quad(&quad.unwrap()).unwrap();
//! }
//! assert_eq!(serializer.finish().unwrap(), ntriples_file.as_bytes());
//! ```

pub use oxrdfio::{
    JsonLdProfile, JsonLdProfileSet, LoadedDocument, RdfFormat, RdfParseError, RdfParser,
    RdfSerializer, RdfSyntaxError, ReaderQuadParser, SliceQuadParser, TextPosition,
    WriterQuadSerializer,
};
