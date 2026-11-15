#![expect(clippy::trivially_copy_pass_by_ref, clippy::unused_self)]

mod dataset;
mod incremental;
mod io;
mod model;
mod sparql;
mod store;
mod transaction;

use crate::dataset::*;
use crate::incremental::*;
use crate::io::*;
use crate::model::*;
use crate::sparql::*;
use crate::store::*;
use crate::transaction::*;
use pyo3::prelude::*;

/// Self-contained Incroxigraph Python bindings.
#[pymodule]
pub mod pyincroxigraph {
    #[expect(non_upper_case_globals)]
    #[pymodule_export]
    const __version__: &str = env!("CARGO_PKG_VERSION");
    #[cfg(feature = "rdf-12")]
    #[pymodule_export]
    use super::PyBaseDirection;
    #[pymodule_export]
    use super::{
        PyBlankNode, PyCanonicalizationAlgorithm, PyDataset, PyDefaultGraph, PyDelta, PyDeltaKind,
        PyIncrementalQueryDeltas, PyIncrementalQueryResults, PyLiteral, PyNamedNode, PyQuad,
        PyQuadParser, PyQueryBoolean, PyQueryBooleanDeltas, PyQueryResultsFormat, PyQuerySolution,
        PyQuerySolutionDeltas, PyQuerySolutions, PyQueryTripleDeltas, PyQueryTriples, PyRdfFormat,
        PyStore, PyTransaction, PyTriple, PyVariable, parse, parse_query_results, serialize,
    };
}
