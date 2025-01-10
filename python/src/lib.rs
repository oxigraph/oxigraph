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
    use super::*;
    #[pymodule_export]
    use super::{
        parse, parse_query_results, serialize, PyBlankNode, PyCanonicalizationAlgorithm, PyDataset,
        PyDefaultGraph, PyLiteral, PyNamedNode, PyQuad, PyQuadParser, PyQueryBoolean,
        PyQueryResultsFormat, PyQuerySolution, PyQuerySolutions, PyQueryTriples, PyRdfFormat,
        PyStore, PyTriple, PyVariable,
    };

    #[pymodule_init]
    fn pymodule_init(module: &Bound<'_, PyModule>) -> PyResult<()> {
        module.add("__package__", "pyoxigraph")?;
        module.add("__version__", env!("CARGO_PKG_VERSION"))?;
        module.add("__author__", env!("CARGO_PKG_AUTHORS").replace(':', "\n"))
    }
}
