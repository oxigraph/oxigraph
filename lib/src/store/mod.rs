//! RDF quads storage implementations.
//!
//! They encode a [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset)
//! and allow querying and updating them using SPARQL.

pub mod memory;
pub(crate) mod numeric_encoder;
#[cfg(feature = "rocksdb")]
pub mod rocksdb;
#[cfg(feature = "sled")]
pub mod sled;

pub use crate::store::memory::MemoryStore;
#[cfg(feature = "rocksdb")]
pub use crate::store::rocksdb::RocksDbStore;
#[cfg(feature = "sled")]
pub use crate::store::sled::SledStore;

use crate::model::*;
use crate::store::numeric_encoder::*;
use crate::{DatasetSyntax, GraphSyntax};
use rio_api::formatter::{QuadsFormatter, TriplesFormatter};
use rio_api::parser::{QuadsParser, TriplesParser};
use rio_turtle::{
    NQuadsFormatter, NQuadsParser, NTriplesFormatter, NTriplesParser, TriGFormatter, TriGParser,
    TurtleError, TurtleFormatter, TurtleParser,
};
use rio_xml::{RdfXmlError, RdfXmlFormatter, RdfXmlParser};
use std::collections::HashMap;
use std::error::Error;
use std::io;
use std::io::{BufRead, Write};
use std::iter::Iterator;

pub(crate) trait ReadableEncodedStore: StrLookup {
    type Error: From<<Self as StrLookup>::Error> + Error + Into<io::Error> + 'static;
    type QuadsIter: Iterator<Item = Result<EncodedQuad, <Self as ReadableEncodedStore>::Error>>
        + 'static;

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm>,
        predicate: Option<EncodedTerm>,
        object: Option<EncodedTerm>,
        graph_name: Option<EncodedTerm>,
    ) -> Self::QuadsIter;
}

pub(crate) trait WritableEncodedStore: StrContainer {
    type Error: From<<Self as StrContainer>::Error> + Error + Into<io::Error>;

    fn insert_encoded(
        &mut self,
        quad: &EncodedQuad,
    ) -> Result<(), <Self as WritableEncodedStore>::Error>;

    fn remove_encoded(
        &mut self,
        quad: &EncodedQuad,
    ) -> Result<(), <Self as WritableEncodedStore>::Error>;
}

fn load_graph<S: WritableEncodedStore>(
    store: &mut S,
    reader: impl BufRead,
    syntax: GraphSyntax,
    to_graph_name: &GraphName,
    base_iri: Option<&str>,
) -> Result<(), io::Error> {
    let base_iri = base_iri.unwrap_or("");
    match syntax {
        GraphSyntax::NTriples => {
            load_from_triple_parser(store, NTriplesParser::new(reader), to_graph_name)
        }
        GraphSyntax::Turtle => {
            load_from_triple_parser(store, TurtleParser::new(reader, base_iri), to_graph_name)
        }
        GraphSyntax::RdfXml => {
            load_from_triple_parser(store, RdfXmlParser::new(reader, base_iri), to_graph_name)
        }
    }
}

fn load_from_triple_parser<S: WritableEncodedStore, P: TriplesParser>(
    store: &mut S,
    parser: Result<P, P::Error>,
    to_graph_name: &GraphName,
) -> Result<(), io::Error>
where
    IoOrParseError<P::Error>: From<P::Error>,
    P::Error: Send + Sync + 'static,
{
    let mut parser = parser.map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let mut bnode_map = HashMap::default();
    let to_graph_name = store
        .encode_graph_name(to_graph_name)
        .map_err(|e| e.into())?;
    let result: Result<(), IoOrParseError<_>> = parser.parse_all(&mut move |t| {
        let quad = store
            .encode_rio_triple_in_graph(t, to_graph_name, &mut bnode_map)
            .map_err(|e| IoOrParseError::Io(e.into()))?;
        store
            .insert_encoded(&quad)
            .map_err(|e| IoOrParseError::Io(e.into()))?;
        Ok(())
    });
    Ok(result?)
}

fn dump_graph(
    triples: impl Iterator<Item = Result<Triple, io::Error>>,
    writer: &mut impl Write,
    syntax: GraphSyntax,
) -> Result<(), io::Error> {
    match syntax {
        GraphSyntax::NTriples => {
            let mut formatter = NTriplesFormatter::new(writer);
            for triple in triples {
                formatter.format(&(&triple?).into())?;
            }
            formatter.finish();
        }
        GraphSyntax::Turtle => {
            let mut formatter = TurtleFormatter::new(writer);
            for triple in triples {
                formatter.format(&(&triple?).into())?;
            }
            formatter.finish()?;
        }
        GraphSyntax::RdfXml => {
            let mut formatter = RdfXmlFormatter::new(writer).map_err(map_xml_err)?;
            for triple in triples {
                formatter.format(&(&triple?).into()).map_err(map_xml_err)?;
            }
            formatter.finish().map_err(map_xml_err)?;
        }
    }
    Ok(())
}

fn map_xml_err(e: RdfXmlError) -> io::Error {
    io::Error::new(io::ErrorKind::Other, e)
}

fn load_dataset<S: WritableEncodedStore>(
    store: &mut S,
    reader: impl BufRead,
    syntax: DatasetSyntax,
    base_iri: Option<&str>,
) -> Result<(), io::Error> {
    let base_iri = base_iri.unwrap_or("");
    match syntax {
        DatasetSyntax::NQuads => load_from_quad_parser(store, NQuadsParser::new(reader)),
        DatasetSyntax::TriG => load_from_quad_parser(store, TriGParser::new(reader, base_iri)),
    }
}

fn load_from_quad_parser<S: WritableEncodedStore, P: QuadsParser>(
    store: &mut S,
    parser: Result<P, P::Error>,
) -> Result<(), io::Error>
where
    IoOrParseError<P::Error>: From<P::Error>,
    P::Error: Send + Sync + 'static,
{
    let mut parser = parser.map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
    let mut bnode_map = HashMap::default();
    let result: Result<(), IoOrParseError<_>> = parser.parse_all(&mut move |q| {
        let quad = store
            .encode_rio_quad(q, &mut bnode_map)
            .map_err(|e| IoOrParseError::Io(e.into()))?;
        store
            .insert_encoded(&quad)
            .map_err(|e| IoOrParseError::Io(e.into()))?;
        Ok(())
    });
    Ok(result?)
}

fn dump_dataset(
    quads: impl Iterator<Item = Result<Quad, io::Error>>,
    writer: &mut impl Write,
    syntax: DatasetSyntax,
) -> Result<(), io::Error> {
    match syntax {
        DatasetSyntax::NQuads => {
            let mut formatter = NQuadsFormatter::new(writer);
            for quad in quads {
                formatter.format(&(&quad?).into())?;
            }
            formatter.finish();
        }
        DatasetSyntax::TriG => {
            let mut formatter = TriGFormatter::new(writer);
            for quad in quads {
                formatter.format(&(&quad?).into())?;
            }
            formatter.finish()?;
        }
    }
    Ok(())
}

enum IoOrParseError<E: Sized> {
    Io(io::Error),
    Parse(E),
}

impl<E: Sized> From<io::Error> for IoOrParseError<E> {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<TurtleError> for IoOrParseError<TurtleError> {
    fn from(error: TurtleError) -> Self {
        Self::Parse(error)
    }
}

impl From<RdfXmlError> for IoOrParseError<RdfXmlError> {
    fn from(error: RdfXmlError) -> Self {
        Self::Parse(error)
    }
}

impl<E: Sized + Error + Send + Sync + 'static> From<IoOrParseError<E>> for io::Error {
    fn from(error: IoOrParseError<E>) -> Self {
        match error {
            IoOrParseError::Io(error) => error,
            IoOrParseError::Parse(error) => io::Error::new(io::ErrorKind::InvalidData, error),
        }
    }
}
