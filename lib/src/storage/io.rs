//! Utilities for I/O from the store

use crate::error::invalid_input_error;
use crate::io::{
    DatasetFormat, DatasetParser, DatasetSerializer, GraphFormat, GraphParser, GraphSerializer,
};
use crate::model::{GraphNameRef, Quad, Triple};
use crate::storage::StorageWriter;
use std::io::{BufRead, Result, Write};

pub fn load_graph(
    writer: &mut StorageWriter,
    reader: impl BufRead,
    format: GraphFormat,
    to_graph_name: GraphNameRef<'_>,
    base_iri: Option<&str>,
) -> Result<()> {
    let mut parser = GraphParser::from_format(format);
    if let Some(base_iri) = base_iri {
        parser = parser
            .with_base_iri(base_iri)
            .map_err(invalid_input_error)?;
    }
    for t in parser.read_triples(reader)? {
        writer.insert(t?.as_ref().in_graph(to_graph_name))?;
    }
    Ok(())
}

pub fn dump_graph(
    triples: impl Iterator<Item = Result<Triple>>,
    writer: impl Write,
    format: GraphFormat,
) -> Result<()> {
    let mut writer = GraphSerializer::from_format(format).triple_writer(writer)?;
    for triple in triples {
        writer.write(&triple?)?;
    }
    writer.finish()
}

pub fn load_dataset(
    writer: &mut StorageWriter,
    reader: impl BufRead,
    format: DatasetFormat,
    base_iri: Option<&str>,
) -> Result<()> {
    let mut parser = DatasetParser::from_format(format);
    if let Some(base_iri) = base_iri {
        parser = parser
            .with_base_iri(base_iri)
            .map_err(invalid_input_error)?;
    }
    for t in parser.read_quads(reader)? {
        writer.insert(t?.as_ref())?;
    }
    Ok(())
}

pub fn dump_dataset(
    quads: impl Iterator<Item = Result<Quad>>,
    writer: impl Write,
    format: DatasetFormat,
) -> Result<()> {
    let mut writer = DatasetSerializer::from_format(format).quad_writer(writer)?;
    for quad in quads {
        writer.write(&quad?)?;
    }
    writer.finish()
}
