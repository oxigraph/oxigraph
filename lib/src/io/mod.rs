//! Utilities to read and write RDF graphs and datasets.

mod format;
pub mod read;
pub mod write;

#[allow(deprecated)]
pub use self::format::{DatasetFormat, GraphFormat};
#[allow(deprecated)]
pub use self::read::{DatasetParser, GraphParser};
#[allow(deprecated)]
pub use self::write::{DatasetSerializer, GraphSerializer};
pub use oxrdfio::{
    FromReadQuadReader, ParseError, RdfFormat, RdfParser, RdfSerializer, SyntaxError,
    ToWriteQuadWriter,
};
