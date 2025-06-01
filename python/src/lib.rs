#![allow(
    clippy::trivially_copy_pass_by_ref,
    clippy::unused_self,
    clippy::useless_conversion
)]

mod dataset;
mod io;
mod model;
mod sparql;
mod store;

use crate::dataset::*;
use crate::io::*;
use crate::model::*;
use crate::sparql::*;
use crate::store::*;
use pyo3::prelude::*;

/// Oxigraph Python bindings
#[pymodule]
pub mod pyoxigraph {
    #[expect(non_upper_case_globals)]
    #[pymodule_export]
    const __version__: &str = env!("CARGO_PKG_VERSION");
    #[cfg(feature = "rdf-12")]
    #[pymodule_export]
    use super::PyBaseDirection;
    #[pymodule_export]
    use super::{
        PyBlankNode, PyCanonicalizationAlgorithm, PyDataset, PyDefaultGraph, PyLiteral,
        PyNamedNode, PyQuad, PyQuadParser, PyQueryBoolean, PyQueryResultsFormat, PyQuerySolution,
        PyQuerySolutions, PyQueryTriples, PyRdfFormat, PyStore, PyTriple, PyVariable, parse,
        parse_query_results, serialize,
    };
}
