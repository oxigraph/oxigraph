use crate::model::*;
use oxigraph::model::*;
use pyo3::exceptions::{IOError, SyntaxError, ValueError};
use pyo3::prelude::*;
use std::io;

pub fn extract_quads_pattern(
    subject: &PyAny,
    predicate: &PyAny,
    object: &PyAny,
    graph_name: Option<&PyAny>,
) -> PyResult<(
    Option<NamedOrBlankNode>,
    Option<NamedNode>,
    Option<Term>,
    Option<GraphName>,
)> {
    Ok((
        if subject.is_none() {
            None
        } else {
            Some(extract_named_or_blank_node(subject)?)
        },
        if predicate.is_none() {
            None
        } else {
            Some(extract_named_node(predicate)?)
        },
        if object.is_none() {
            None
        } else {
            Some(extract_term(object)?)
        },
        if let Some(graph_name) = graph_name {
            if graph_name.is_none() {
                None
            } else {
                Some(extract_graph_name(graph_name)?)
            }
        } else {
            None
        },
    ))
}

pub fn map_io_err(error: io::Error) -> PyErr {
    match error.kind() {
        io::ErrorKind::InvalidInput => ValueError::py_err(error.to_string()),
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => {
            SyntaxError::py_err(error.to_string())
        }
        _ => IOError::py_err(error.to_string()),
    }
}
