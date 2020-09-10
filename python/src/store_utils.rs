use crate::model::*;
use pyo3::exceptions::{PyIOError, PySyntaxError, PyValueError};
use pyo3::{PyAny, PyErr, PyResult};
use std::convert::TryInto;
use std::io;

pub fn extract_quads_pattern<'a>(
    subject: &'a PyAny,
    predicate: &'a PyAny,
    object: &'a PyAny,
    graph_name: Option<&'a PyAny>,
) -> PyResult<(
    Option<PyNamedOrBlankNodeRef<'a>>,
    Option<PyNamedNodeRef<'a>>,
    Option<PyTermRef<'a>>,
    Option<PyGraphNameRef<'a>>,
)> {
    Ok((
        if subject.is_none() {
            None
        } else {
            Some(subject.try_into()?)
        },
        if predicate.is_none() {
            None
        } else {
            Some(predicate.try_into()?)
        },
        if object.is_none() {
            None
        } else {
            Some(object.try_into()?)
        },
        if let Some(graph_name) = graph_name {
            if graph_name.is_none() {
                None
            } else {
                Some(graph_name.try_into()?)
            }
        } else {
            None
        },
    ))
}

pub fn map_io_err(error: io::Error) -> PyErr {
    match error.kind() {
        io::ErrorKind::InvalidInput => PyValueError::new_err(error.to_string()),
        io::ErrorKind::InvalidData | io::ErrorKind::UnexpectedEof => {
            PySyntaxError::new_err(error.to_string())
        }
        _ => PyIOError::new_err(error.to_string()),
    }
}
