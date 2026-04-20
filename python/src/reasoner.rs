//! Python bindings for the `oxreason` OWL 2 RL reasoner.
//!
//! The bindings expose two classes:
//!
//! * `Reasoner` wraps `oxreason::Reasoner` plus its config. Constructed with a
//!   profile string and an optional equality_rules flag.
//! * `ReasoningReport` mirrors `oxreason::ReasoningReport` with read only
//!   accessors for the counters.
//!
//! `Reasoner.expand(dataset)` materialises the OWL 2 RL (or RDFS) closure of
//! the dataset's default graph in place. Triples inferred by the rules are
//! inserted back into the default graph of the same `Dataset`. Named graphs
//! are left untouched. This mirrors how the Rust `Reasoner::expand` call
//! operates on an `oxrdf::Graph`; the Python wrapper simply copies the
//! default graph into a temporary `Graph`, reasons, and writes the result
//! back.
//!
//! SHACL validation and dataset aware reasoning are not exposed yet; the
//! underlying Rust APIs for those still return `NotImplemented`.

use crate::dataset::PyDataset;
use oxigraph::model::{Graph, GraphNameRef, QuadRef};
use oxreason::{ReasonError, Reasoner, ReasonerConfig, ReasoningProfile, ReasoningReport};
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;

/// Summary returned by a successful :py:meth:`Reasoner.expand` call.
///
/// :ivar added: number of triples materialised into the target graph.
/// :vartype added: int
/// :ivar rounds: number of semi naive fixpoint rounds executed.
/// :vartype rounds: int
/// :ivar firings: total rule firings across the full run.
/// :vartype firings: int
#[pyclass(frozen, name = "ReasoningReport", module = "pyoxigraph", eq)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PyReasoningReport {
    inner: ReasoningReport,
}

#[pymethods]
impl PyReasoningReport {
    /// :return: number of triples newly materialised by reasoning.
    /// :rtype: int
    #[getter]
    fn added(&self) -> u64 {
        self.inner.added
    }

    /// :return: number of fixpoint rounds executed before saturation.
    /// :rtype: int
    #[getter]
    fn rounds(&self) -> u32 {
        self.inner.rounds
    }

    /// :return: total number of rule firings observed across the run.
    /// :rtype: int
    #[getter]
    fn firings(&self) -> u64 {
        self.inner.firings
    }

    fn __repr__(&self) -> String {
        format!(
            "<ReasoningReport added={} rounds={} firings={}>",
            self.inner.added, self.inner.rounds, self.inner.firings
        )
    }
}

/// OWL 2 RL forward chaining reasoner.
///
/// :param profile: reasoning profile. Accepted values are ``"owl2-rl"``
///                 (default) and ``"rdfs"``.
/// :type profile: str
/// :param equality_rules: enables the OWL 2 RL equality rules
///                        (``eq-sym``, ``eq-trans``, ``eq-rep-s``,
///                        ``eq-rep-p``, ``eq-rep-o``) and the functional
///                        property rules (``prp-fp``, ``prp-ifp``). These are
///                        correct but can inflate the graph significantly on
///                        noisy data, so they default to ``False``.
/// :type equality_rules: bool
/// :raises ValueError: if ``profile`` is not one of the supported values.
///
/// >>> reasoner = Reasoner(profile="owl2-rl")
/// >>> reasoner.profile
/// 'owl2-rl'
#[pyclass(frozen, name = "Reasoner", module = "pyoxigraph")]
pub struct PyReasoner {
    config: ReasonerConfig,
}

#[pymethods]
impl PyReasoner {
    #[new]
    #[pyo3(signature = (profile = "owl2-rl", equality_rules = false))]
    fn new(profile: &str, equality_rules: bool) -> PyResult<Self> {
        let config = match profile {
            "owl2-rl" | "owl2rl" => ReasonerConfig::owl2_rl(),
            "rdfs" => ReasonerConfig::rdfs(),
            other => {
                return Err(PyValueError::new_err(format!(
                    "unknown reasoning profile '{other}'; expected 'owl2-rl' or 'rdfs'"
                )));
            }
        };
        Ok(Self {
            config: config.with_equality_rules(equality_rules),
        })
    }

    /// :return: the name of the reasoning profile this reasoner is
    ///          configured for.
    /// :rtype: str
    #[getter]
    fn profile(&self) -> &'static str {
        match self.config.profile() {
            ReasoningProfile::Owl2Rl => "owl2-rl",
            ReasoningProfile::Rdfs => "rdfs",
            ReasoningProfile::Custom => "custom",
        }
    }

    /// :return: whether the OWL 2 RL equality rules are enabled.
    /// :rtype: bool
    #[getter]
    fn equality_rules(&self) -> bool {
        self.config.equality_rules_enabled()
    }

    /// Materialises the reasoning closure of a dataset's default graph in
    /// place.
    ///
    /// Only the default graph is reasoned over. Named graphs are ignored by
    /// the current rule engine and are left untouched by this call.
    ///
    /// :param dataset: the dataset whose default graph should be expanded.
    /// :type dataset: Dataset
    /// :return: summary counters for the reasoning run.
    /// :rtype: ReasoningReport
    /// :raises ValueError: if the input graph is found to be inconsistent,
    ///                     for example an individual typed as two classes
    ///                     marked ``owl:disjointWith``.
    /// :raises RuntimeError: if the reasoner signals an internal error.
    fn expand(
        &self,
        dataset: &mut PyDataset,
        py: Python<'_>,
    ) -> PyResult<PyReasoningReport> {
        let config = self.config.clone();
        py.detach(|| {
            let default_view = dataset.inner.graph(GraphNameRef::DefaultGraph);
            let mut graph = Graph::default();
            for triple in &default_view {
                graph.insert(triple);
            }
            drop(default_view);
            let reasoner = Reasoner::new(config);
            let report = reasoner.expand(&mut graph).map_err(map_reason_error)?;
            for triple in &graph {
                dataset.inner.insert(QuadRef::new(
                    triple.subject,
                    triple.predicate,
                    triple.object,
                    GraphNameRef::DefaultGraph,
                ));
            }
            Ok(PyReasoningReport { inner: report })
        })
    }

    fn __repr__(&self) -> String {
        format!(
            "<Reasoner profile={} equality_rules={}>",
            self.profile(),
            self.equality_rules()
        )
    }
}

fn map_reason_error(err: ReasonError) -> PyErr {
    // `ReasonError` is `#[non_exhaustive]`, so a catch-all arm is required for
    // forward compatibility when new error variants are introduced upstream.
    match err {
        ReasonError::NotImplemented(msg) => {
            PyRuntimeError::new_err(format!("reasoning not implemented: {msg}"))
        }
        ReasonError::InvalidIri(iri) => {
            PyValueError::new_err(format!("invalid IRI encountered during reasoning: {iri}"))
        }
        ReasonError::Cancelled => {
            PyRuntimeError::new_err("reasoning was cancelled before it could complete")
        }
        ReasonError::Write(msg) => {
            PyRuntimeError::new_err(format!("failed to write inferred triple: {msg}"))
        }
        ReasonError::Inconsistent {
            individual,
            class_a,
            class_b,
        } => PyValueError::new_err(format!(
            "inconsistent graph: individual {individual} is typed as disjoint classes {class_a} and {class_b}"
        )),
        other => PyRuntimeError::new_err(format!("reasoning error: {other}")),
    }
}
