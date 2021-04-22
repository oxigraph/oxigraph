//! Utilities for I/O from the store

use crate::error::invalid_input_error;
use crate::io::{DatasetFormat, DatasetSerializer, GraphFormat, GraphSerializer};
use crate::model::{GraphNameRef, Quad, Triple};
use crate::storage::numeric_encoder::WriteEncoder;
use crate::storage::StorageLike;
use oxiri::Iri;
use rio_api::parser::{QuadsParser, TriplesParser};
use rio_turtle::{NQuadsParser, NTriplesParser, TriGParser, TurtleError, TurtleParser};
use rio_xml::{RdfXmlError, RdfXmlParser};
use std::collections::HashMap;
use std::io;
use std::io::{BufRead, Write};

pub(crate) fn load_graph<S: StorageLike>(
    storage: &S,
    reader: impl BufRead,
    format: GraphFormat,
    to_graph_name: GraphNameRef<'_>,
    base_iri: Option<&str>,
) -> Result<(), StoreOrParseError<S::Error>> {
    let base_iri = if let Some(base_iri) = base_iri {
        Some(Iri::parse(base_iri.into()).map_err(invalid_input_error)?)
    } else {
        None
    };
    match format {
        GraphFormat::NTriples => {
            load_from_triple_parser(storage, NTriplesParser::new(reader), to_graph_name)
        }
        GraphFormat::Turtle => {
            load_from_triple_parser(storage, TurtleParser::new(reader, base_iri), to_graph_name)
        }
        GraphFormat::RdfXml => {
            load_from_triple_parser(storage, RdfXmlParser::new(reader, base_iri), to_graph_name)
        }
    }
}

fn load_from_triple_parser<S: StorageLike, P: TriplesParser>(
    storage: &S,
    mut parser: P,
    to_graph_name: GraphNameRef<'_>,
) -> Result<(), StoreOrParseError<S::Error>>
where
    StoreOrParseError<S::Error>: From<P::Error>,
{
    let mut bnode_map = HashMap::default();
    let to_graph_name = storage
        .encode_graph_name(to_graph_name)
        .map_err(StoreOrParseError::Store)?;
    parser.parse_all(&mut move |t| {
        let quad = storage
            .encode_rio_triple_in_graph(t, to_graph_name, &mut bnode_map)
            .map_err(StoreOrParseError::Store)?;
        storage.insert(&quad).map_err(StoreOrParseError::Store)?;
        Ok(())
    })
}

pub fn dump_graph(
    triples: impl Iterator<Item = Result<Triple, io::Error>>,
    writer: impl Write,
    format: GraphFormat,
) -> Result<(), io::Error> {
    let mut writer = GraphSerializer::from_format(format).triple_writer(writer)?;
    for triple in triples {
        writer.write(&triple?)?;
    }
    writer.finish()
}

pub(crate) fn load_dataset<S: StorageLike>(
    store: &S,
    reader: impl BufRead,
    format: DatasetFormat,
    base_iri: Option<&str>,
) -> Result<(), StoreOrParseError<S::Error>> {
    let base_iri = if let Some(base_iri) = base_iri {
        Some(Iri::parse(base_iri.into()).map_err(invalid_input_error)?)
    } else {
        None
    };
    match format {
        DatasetFormat::NQuads => load_from_quad_parser(store, NQuadsParser::new(reader)),
        DatasetFormat::TriG => load_from_quad_parser(store, TriGParser::new(reader, base_iri)),
    }
}

fn load_from_quad_parser<S: StorageLike, P: QuadsParser>(
    store: &S,
    mut parser: P,
) -> Result<(), StoreOrParseError<S::Error>>
where
    StoreOrParseError<S::Error>: From<P::Error>,
{
    let mut bnode_map = HashMap::default();
    parser.parse_all(&mut move |q| {
        let quad = store
            .encode_rio_quad(q, &mut bnode_map)
            .map_err(StoreOrParseError::Store)?;
        store.insert(&quad).map_err(StoreOrParseError::Store)?;
        Ok(())
    })
}

pub fn dump_dataset(
    quads: impl Iterator<Item = Result<Quad, io::Error>>,
    writer: impl Write,
    format: DatasetFormat,
) -> Result<(), io::Error> {
    let mut writer = DatasetSerializer::from_format(format).quad_writer(writer)?;
    for quad in quads {
        writer.write(&quad?)?;
    }
    writer.finish()
}

pub(crate) enum StoreOrParseError<S> {
    Store(S),
    Parse(io::Error),
}

impl<S> From<TurtleError> for StoreOrParseError<S> {
    fn from(error: TurtleError) -> Self {
        Self::Parse(error.into())
    }
}

impl<S> From<RdfXmlError> for StoreOrParseError<S> {
    fn from(error: RdfXmlError) -> Self {
        Self::Parse(error.into())
    }
}

impl<S> From<io::Error> for StoreOrParseError<S> {
    fn from(error: io::Error) -> Self {
        Self::Parse(error)
    }
}

impl From<StoreOrParseError<io::Error>> for io::Error {
    fn from(error: StoreOrParseError<io::Error>) -> Self {
        match error {
            StoreOrParseError::Store(error) => error,
            StoreOrParseError::Parse(error) => error,
        }
    }
}
