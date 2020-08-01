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

use crate::error::{invalid_data_error, invalid_input_error};
use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use crate::store::numeric_encoder::*;
use rio_api::formatter::{QuadsFormatter, TriplesFormatter};
use rio_api::parser::{QuadsParser, TriplesParser};
use rio_turtle::{
    NQuadsFormatter, NQuadsParser, NTriplesFormatter, NTriplesParser, TriGFormatter, TriGParser,
    TurtleError, TurtleFormatter, TurtleParser,
};
use rio_xml::{RdfXmlError, RdfXmlFormatter, RdfXmlParser};
use std::collections::HashMap;
use std::convert::Infallible;
use std::io;
use std::io::{BufRead, Write};
use std::iter::Iterator;

pub(crate) trait ReadableEncodedStore: StrLookup {
    type QuadsIter: Iterator<Item = Result<EncodedQuad<Self::StrId>, Self::Error>> + 'static;

    fn encoded_quads_for_pattern(
        &self,
        subject: Option<EncodedTerm<Self::StrId>>,
        predicate: Option<EncodedTerm<Self::StrId>>,
        object: Option<EncodedTerm<Self::StrId>>,
        graph_name: Option<EncodedTerm<Self::StrId>>,
    ) -> Self::QuadsIter;
}

pub(crate) trait WritableEncodedStore: WithStoreError {
    fn insert_encoded(&mut self, quad: &EncodedQuad<Self::StrId>) -> Result<(), Self::Error>;

    fn remove_encoded(&mut self, quad: &EncodedQuad<Self::StrId>) -> Result<(), Self::Error>;
}

fn load_graph<S: WritableEncodedStore + StrContainer>(
    store: &mut S,
    reader: impl BufRead,
    format: GraphFormat,
    to_graph_name: &GraphName,
    base_iri: Option<&str>,
) -> Result<(), StoreOrParseError<S::Error, io::Error>> {
    let base_iri = base_iri.unwrap_or("");
    match format {
        GraphFormat::NTriples => {
            load_from_triple_parser(store, NTriplesParser::new(reader), to_graph_name)
        }
        GraphFormat::Turtle => {
            load_from_triple_parser(store, TurtleParser::new(reader, base_iri), to_graph_name)
        }
        GraphFormat::RdfXml => {
            load_from_triple_parser(store, RdfXmlParser::new(reader, base_iri), to_graph_name)
        }
    }
}

fn load_from_triple_parser<S: WritableEncodedStore + StrContainer, P: TriplesParser>(
    store: &mut S,
    parser: Result<P, P::Error>,
    to_graph_name: &GraphName,
) -> Result<(), StoreOrParseError<S::Error, io::Error>>
where
    StoreOrParseError<S::Error, P::Error>: From<P::Error>,
    P::Error: Send + Sync + 'static,
{
    let mut parser = parser.map_err(invalid_input_error)?;
    let mut bnode_map = HashMap::default();
    let to_graph_name = store
        .encode_graph_name(to_graph_name)
        .map_err(StoreOrParseError::Store)?;
    parser
        .parse_all(&mut move |t| {
            let quad = store
                .encode_rio_triple_in_graph(t, to_graph_name, &mut bnode_map)
                .map_err(StoreOrParseError::Store)?;
            store
                .insert_encoded(&quad)
                .map_err(StoreOrParseError::Store)?;
            Ok(())
        })
        .map_err(|e| match e {
            StoreOrParseError::Store(e) => StoreOrParseError::Store(e),
            StoreOrParseError::Parse(e) => StoreOrParseError::Parse(invalid_data_error(e)),
        })
}

fn dump_graph(
    triples: impl Iterator<Item = Result<Triple, io::Error>>,
    writer: impl Write,
    format: GraphFormat,
) -> Result<(), io::Error> {
    match format {
        GraphFormat::NTriples => {
            let mut formatter = NTriplesFormatter::new(writer);
            for triple in triples {
                formatter.format(&(&triple?).into())?;
            }
            formatter.finish();
        }
        GraphFormat::Turtle => {
            let mut formatter = TurtleFormatter::new(writer);
            for triple in triples {
                formatter.format(&(&triple?).into())?;
            }
            formatter.finish()?;
        }
        GraphFormat::RdfXml => {
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
    io::Error::new(io::ErrorKind::Other, e) // TODO: drop
}

fn load_dataset<S: WritableEncodedStore + StrContainer>(
    store: &mut S,
    reader: impl BufRead,
    format: DatasetFormat,
    base_iri: Option<&str>,
) -> Result<(), StoreOrParseError<S::Error, io::Error>> {
    let base_iri = base_iri.unwrap_or("");
    match format {
        DatasetFormat::NQuads => load_from_quad_parser(store, NQuadsParser::new(reader)),
        DatasetFormat::TriG => load_from_quad_parser(store, TriGParser::new(reader, base_iri)),
    }
}

fn load_from_quad_parser<S: WritableEncodedStore + StrContainer, P: QuadsParser>(
    store: &mut S,
    parser: Result<P, P::Error>,
) -> Result<(), StoreOrParseError<S::Error, io::Error>>
where
    StoreOrParseError<S::Error, P::Error>: From<P::Error>,
    P::Error: Send + Sync + 'static,
{
    let mut parser = parser.map_err(invalid_input_error)?;
    let mut bnode_map = HashMap::default();
    parser
        .parse_all(&mut move |q| {
            let quad = store
                .encode_rio_quad(q, &mut bnode_map)
                .map_err(StoreOrParseError::Store)?;
            store
                .insert_encoded(&quad)
                .map_err(StoreOrParseError::Store)?;
            Ok(())
        })
        .map_err(|e| match e {
            StoreOrParseError::Store(e) => StoreOrParseError::Store(e),
            StoreOrParseError::Parse(e) => StoreOrParseError::Parse(invalid_data_error(e)),
        })
}

fn dump_dataset(
    quads: impl Iterator<Item = Result<Quad, io::Error>>,
    writer: impl Write,
    format: DatasetFormat,
) -> Result<(), io::Error> {
    match format {
        DatasetFormat::NQuads => {
            let mut formatter = NQuadsFormatter::new(writer);
            for quad in quads {
                formatter.format(&(&quad?).into())?;
            }
            formatter.finish();
        }
        DatasetFormat::TriG => {
            let mut formatter = TriGFormatter::new(writer);
            for quad in quads {
                formatter.format(&(&quad?).into())?;
            }
            formatter.finish()?;
        }
    }
    Ok(())
}

enum StoreOrParseError<S, P> {
    Store(S),
    Parse(P),
}

impl<S> From<TurtleError> for StoreOrParseError<S, TurtleError> {
    fn from(error: TurtleError) -> Self {
        Self::Parse(error)
    }
}

impl<S> From<RdfXmlError> for StoreOrParseError<S, RdfXmlError> {
    fn from(error: RdfXmlError) -> Self {
        Self::Parse(error)
    }
}

impl<S> From<io::Error> for StoreOrParseError<S, io::Error> {
    fn from(error: io::Error) -> Self {
        Self::Parse(error)
    }
}

impl<P: Into<io::Error>> From<StoreOrParseError<io::Error, P>> for io::Error {
    fn from(error: StoreOrParseError<io::Error, P>) -> Self {
        match error {
            StoreOrParseError::Store(error) => error,
            StoreOrParseError::Parse(error) => error.into(),
        }
    }
}
impl<P: Into<io::Error>> From<StoreOrParseError<Infallible, P>> for io::Error {
    fn from(error: StoreOrParseError<Infallible, P>) -> Self {
        match error {
            StoreOrParseError::Store(error) => match error {},
            StoreOrParseError::Parse(error) => error.into(),
        }
    }
}

type QuadPattern<I> = (
    Option<EncodedTerm<I>>,
    Option<EncodedTerm<I>>,
    Option<EncodedTerm<I>>,
    Option<EncodedTerm<I>>,
);

fn get_encoded_quad_pattern<E: ReadEncoder>(
    encoder: &E,
    subject: Option<&NamedOrBlankNode>,
    predicate: Option<&NamedNode>,
    object: Option<&Term>,
    graph_name: Option<&GraphName>,
) -> Result<Option<QuadPattern<E::StrId>>, E::Error> {
    Ok(Some((
        if let Some(subject) = transpose(
            subject
                .map(|t| encoder.get_encoded_named_or_blank_node(t))
                .transpose()?,
        ) {
            subject
        } else {
            return Ok(None);
        },
        if let Some(predicate) = transpose(
            predicate
                .map(|t| encoder.get_encoded_named_node(t))
                .transpose()?,
        ) {
            predicate
        } else {
            return Ok(None);
        },
        if let Some(object) = transpose(object.map(|t| encoder.get_encoded_term(t)).transpose()?) {
            object
        } else {
            return Ok(None);
        },
        if let Some(graph_name) = transpose(
            graph_name
                .map(|t| encoder.get_encoded_graph_name(t))
                .transpose()?,
        ) {
            graph_name
        } else {
            return Ok(None);
        },
    )))
}

fn transpose<T>(o: Option<Option<T>>) -> Option<Option<T>> {
    match o {
        Some(Some(v)) => Some(Some(v)),
        Some(None) => None,
        None => Some(None),
    }
}
