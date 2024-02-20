#![allow(
    clippy::unused_self,
    clippy::trivially_copy_pass_by_ref,
    unused_qualifications
)]

mod io;
mod model;
mod sparql;
mod store;

use crate::io::*;
use crate::model::*;
use crate::sparql::*;
use crate::store::*;
use pyo3::prelude::*;

/// Oxigraph Python bindings
#[pymodule]
fn pyoxigraph(_py: Python<'_>, module: &PyModule) -> PyResult<()> {
    module.add("__package__", "pyoxigraph")?;
    module.add("__version__", env!("CARGO_PKG_VERSION"))?;
    module.add("__author__", env!("CARGO_PKG_AUTHORS").replace(':', "\n"))?;

    module.add_class::<PyNamedNode>()?;
    module.add_class::<PyBlankNode>()?;
    module.add_class::<PyLiteral>()?;
    module.add_class::<PyDefaultGraph>()?;
    module.add_class::<PyTriple>()?;
    module.add_class::<PyQuad>()?;
    module.add_class::<PyStore>()?;
    module.add_class::<PyVariable>()?;
    module.add_class::<PyQuerySolutions>()?;
    module.add_class::<PyQuerySolution>()?;
    module.add_class::<PyQueryBoolean>()?;
    module.add_class::<PyQueryTriples>()?;
    module.add_class::<PyRdfFormat>()?;
    module.add_class::<PyQueryResultsFormat>()?;
    module.add_wrapped(wrap_pyfunction!(parse))?;
    module.add_wrapped(wrap_pyfunction!(parse_query_results))?;
    module.add_wrapped(wrap_pyfunction!(serialize))?;
    Ok(())
}
