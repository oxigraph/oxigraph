#![deny(
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_qualifications
)]

mod io;
mod memory_store;
mod model;
mod sled_store;
mod sparql;
mod store_utils;

use crate::memory_store::*;
use crate::model::*;
use crate::sled_store::*;
use crate::sparql::*;
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
    module.add_class::<PyMemoryStore>()?;
    module.add_class::<PySledStore>()?;
    module.add_class::<PyVariable>()?;
    module.add_class::<PyQuerySolutions>()?;
    module.add_class::<PyQuerySolution>()?;
    module.add_class::<PyQueryTriples>()?;
    io::add_to_module(module)
}
