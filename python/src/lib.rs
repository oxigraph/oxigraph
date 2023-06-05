mod io;
mod model;
mod sparql;
mod store;

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
    module.add_class::<PyQueryTriples>()?;
    io::add_to_module(module)
}
