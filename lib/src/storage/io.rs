//! Utilities for I/O from the store

use crate::error::invalid_input_error;
use crate::io::{
    DatasetFormat, DatasetParser, DatasetSerializer, GraphFormat, GraphParser, GraphSerializer,
};
use crate::model::{GraphNameRef, Quad, Triple};
use crate::storage::StorageLike;
use std::io;
use std::io::{BufRead, Write};

pub fn load_graph<S: StorageLike>(
    store: &S,
    reader: impl BufRead,
    format: GraphFormat,
    to_graph_name: GraphNameRef<'_>,
    base_iri: Option<&str>,
) -> Result<(), StoreOrParseError<S::Error>> {
    let mut parser = GraphParser::from_format(format);
    if let Some(base_iri) = base_iri {
        parser = parser
            .with_base_iri(base_iri)
            .map_err(|e| StoreOrParseError::Parse(invalid_input_error(e)))?;
    }
    for t in parser
        .read_triples(reader)
        .map_err(StoreOrParseError::Parse)?
    {
        store
            .insert(
                t.map_err(StoreOrParseError::Parse)?
                    .as_ref()
                    .in_graph(to_graph_name),
            )
            .map_err(StoreOrParseError::Store)?;
    }
    Ok(())
}

pub fn dump_graph(
    triples: impl Iterator<Item = io::Result<Triple>>,
    writer: impl Write,
    format: GraphFormat,
) -> io::Result<()> {
    let mut writer = GraphSerializer::from_format(format).triple_writer(writer)?;
    for triple in triples {
        writer.write(&triple?)?;
    }
    writer.finish()
}

pub fn load_dataset<S: StorageLike>(
    store: &S,
    reader: impl BufRead,
    format: DatasetFormat,
    base_iri: Option<&str>,
) -> Result<(), StoreOrParseError<S::Error>> {
    let mut parser = DatasetParser::from_format(format);
    if let Some(base_iri) = base_iri {
        parser = parser
            .with_base_iri(base_iri)
            .map_err(|e| StoreOrParseError::Parse(invalid_input_error(e)))?;
    }
    for t in parser
        .read_quads(reader)
        .map_err(StoreOrParseError::Parse)?
    {
        store
            .insert(t.map_err(StoreOrParseError::Parse)?.as_ref())
            .map_err(StoreOrParseError::Store)?;
    }
    Ok(())
}

pub fn dump_dataset(
    quads: impl Iterator<Item = io::Result<Quad>>,
    writer: impl Write,
    format: DatasetFormat,
) -> io::Result<()> {
    let mut writer = DatasetSerializer::from_format(format).quad_writer(writer)?;
    for quad in quads {
        writer.write(&quad?)?;
    }
    writer.finish()
}

pub enum StoreOrParseError<S> {
    Store(S),
    Parse(io::Error),
}

impl From<StoreOrParseError<Self>> for io::Error {
    fn from(error: StoreOrParseError<Self>) -> Self {
        match error {
            StoreOrParseError::Store(error) | StoreOrParseError::Parse(error) => error,
        }
    }
}
