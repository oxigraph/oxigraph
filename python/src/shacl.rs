//! Python bindings for SHACL validation.

use crate::model::*;
use pyo3::exceptions::{PyRuntimeError, PyValueError};
use pyo3::prelude::*;
use sparshacl::{ShaclError, ShaclValidator, ShapesGraph, ValidationReport, ValidationResult};

/// A SHACL shapes graph for validation.
///
/// >>> shapes = ShaclShapesGraph()
/// >>> shapes.parse('''
/// ...     @prefix sh: <http://www.w3.org/ns/shacl#> .
/// ...     @prefix ex: <http://example.org/> .
/// ...     ex:PersonShape a sh:NodeShape ;
/// ...         sh:targetClass ex:Person ;
/// ...         sh:property [
/// ...             sh:path ex:name ;
/// ...             sh:minCount 1
/// ...         ] .
/// ... ''')
#[pyclass(name = "ShaclShapesGraph", module = "pyoxigraph")]
#[derive(Clone)]
pub struct PyShaclShapesGraph {
    inner: ShapesGraph,
}

#[pymethods]
impl PyShaclShapesGraph {
    /// Creates a new empty shapes graph.
    #[new]
    pub fn new() -> Self {
        Self {
            inner: ShapesGraph::new(),
        }
    }

    /// Parses shapes from a string in Turtle format.
    ///
    /// :param data: The Turtle-formatted shapes data
    /// :raises ValueError: If the data cannot be parsed
    ///
    /// >>> shapes = ShaclShapesGraph()
    /// >>> shapes.parse('''
    /// ...     @prefix sh: <http://www.w3.org/ns/shacl#> .
    /// ...     @prefix ex: <http://example.org/> .
    /// ...     ex:Shape a sh:NodeShape .
    /// ... ''')
    pub fn parse(&mut self, data: &str) -> PyResult<()> {
        use oxrdf::Graph;
        use oxrdfio::{RdfFormat, RdfParser};

        let mut graph = Graph::new();
        let parser = RdfParser::from_format(RdfFormat::Turtle);

        for quad_result in parser.for_reader(data.as_bytes()) {
            let quad = quad_result.map_err(|e| PyValueError::new_err(e.to_string()))?;
            graph.insert(quad.as_ref());
        }

        self.inner =
            ShapesGraph::from_graph(&graph).map_err(|e| PyValueError::new_err(e.to_string()))?;

        Ok(())
    }

    /// Returns the number of shapes in the shapes graph.
    pub fn __len__(&self) -> usize {
        self.inner.len()
    }

    /// Returns True if the shapes graph is empty.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// A SHACL validator for validating RDF data against shapes.
///
/// >>> shapes = ShaclShapesGraph()
/// >>> shapes.parse('''
/// ...     @prefix sh: <http://www.w3.org/ns/shacl#> .
/// ...     @prefix ex: <http://example.org/> .
/// ...     ex:PersonShape a sh:NodeShape ;
/// ...         sh:targetClass ex:Person ;
/// ...         sh:property [
/// ...             sh:path ex:name ;
/// ...             sh:minCount 1
/// ...         ] .
/// ... ''')
/// >>> validator = ShaclValidator(shapes)
#[pyclass(name = "ShaclValidator", module = "pyoxigraph")]
pub struct PyShaclValidator {
    inner: ShaclValidator,
}

#[pymethods]
impl PyShaclValidator {
    /// Creates a new validator with the given shapes graph.
    ///
    /// :param shapes: The shapes graph to validate against
    #[new]
    pub fn new(shapes: &PyShaclShapesGraph) -> Self {
        Self {
            inner: ShaclValidator::new(shapes.inner.clone()),
        }
    }

    /// Validates data against the shapes graph.
    ///
    /// :param data: The data to validate (as Turtle string)
    /// :return: A validation report
    /// :raises ValueError: If the data cannot be parsed
    /// :raises RuntimeError: If validation fails
    ///
    /// >>> shapes = ShaclShapesGraph()
    /// >>> shapes.parse('''
    /// ...     @prefix sh: <http://www.w3.org/ns/shacl#> .
    /// ...     @prefix ex: <http://example.org/> .
    /// ...     ex:PersonShape a sh:NodeShape ;
    /// ...         sh:targetClass ex:Person ;
    /// ...         sh:property [ sh:path ex:name ; sh:minCount 1 ] .
    /// ... ''')
    /// >>> validator = ShaclValidator(shapes)
    /// >>> report = validator.validate('''
    /// ...     @prefix ex: <http://example.org/> .
    /// ...     ex:alice a ex:Person ; ex:name "Alice" .
    /// ... ''')
    /// >>> report.conforms
    /// True
    pub fn validate(&self, data: &str) -> PyResult<PyShaclValidationReport> {
        use oxrdf::Graph;
        use oxrdfio::{RdfFormat, RdfParser};

        let mut graph = Graph::new();
        let parser = RdfParser::from_format(RdfFormat::Turtle);

        for quad_result in parser.for_reader(data.as_bytes()) {
            let quad = quad_result.map_err(|e| PyValueError::new_err(e.to_string()))?;
            graph.insert(quad.as_ref());
        }

        let report = self.inner.validate(&graph).map_err(map_shacl_error)?;

        Ok(PyShaclValidationReport { inner: report })
    }

    /// Validates a Graph object against the shapes graph.
    ///
    /// :param graph: The oxrdf Graph to validate
    /// :return: A validation report
    /// :raises RuntimeError: If validation fails
    pub fn validate_graph(&self, graph: &PyDataset) -> PyResult<PyShaclValidationReport> {
        // Convert PyDataset to oxrdf Graph (use default graph)
        let oxrdf_graph = graph.inner.iter().fold(oxrdf::Graph::new(), |mut g, q| {
            if q.graph_name.is_default_graph() {
                g.insert(&oxrdf::Triple::new(
                    q.subject.clone(),
                    q.predicate.clone(),
                    q.object.clone(),
                ));
            }
            g
        });

        let report = self.inner.validate(&oxrdf_graph).map_err(map_shacl_error)?;

        Ok(PyShaclValidationReport { inner: report })
    }
}

/// A SHACL validation report.
///
/// Contains the results of validating data against a shapes graph.
#[pyclass(name = "ShaclValidationReport", module = "pyoxigraph")]
#[derive(Clone)]
pub struct PyShaclValidationReport {
    inner: ValidationReport,
}

#[pymethods]
impl PyShaclValidationReport {
    /// Whether the data conforms to the shapes graph.
    ///
    /// Returns True if there are no violations.
    #[getter]
    pub fn conforms(&self) -> bool {
        self.inner.conforms()
    }

    /// The number of violations.
    #[getter]
    pub fn violation_count(&self) -> usize {
        self.inner.violation_count()
    }

    /// The number of warnings.
    #[getter]
    pub fn warning_count(&self) -> usize {
        self.inner.warning_count()
    }

    /// The number of info results.
    #[getter]
    pub fn info_count(&self) -> usize {
        self.inner.info_count()
    }

    /// Returns the validation results as a list.
    pub fn results(&self) -> Vec<PyShaclValidationResult> {
        self.inner
            .results()
            .iter()
            .map(|r| PyShaclValidationResult { inner: r.clone() })
            .collect()
    }

    /// Returns the report as a Turtle string.
    pub fn to_turtle(&self) -> PyResult<String> {
        use oxrdfio::{RdfFormat, RdfSerializer};
        use std::io::Cursor;

        let graph = self.inner.to_graph();
        let mut buffer = Cursor::new(Vec::new());

        {
            let mut serializer =
                RdfSerializer::from_format(RdfFormat::Turtle).for_writer(&mut buffer);
            for triple in graph.iter() {
                serializer
                    .serialize_triple(triple)
                    .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
            }
            serializer
                .finish()
                .map_err(|e| PyRuntimeError::new_err(e.to_string()))?;
        }

        String::from_utf8(buffer.into_inner()).map_err(|e| PyRuntimeError::new_err(e.to_string()))
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ShaclValidationReport(conforms={}, violations={}, warnings={}, info={})",
            self.inner.conforms(),
            self.inner.violation_count(),
            self.inner.warning_count(),
            self.inner.info_count()
        )
    }
}

/// A single SHACL validation result.
#[pyclass(name = "ShaclValidationResult", module = "pyoxigraph")]
#[derive(Clone)]
pub struct PyShaclValidationResult {
    inner: ValidationResult,
}

#[pymethods]
impl PyShaclValidationResult {
    /// The focus node that was validated.
    #[getter]
    pub fn focus_node(&self) -> PyObject {
        Python::with_gil(|py| term_to_python(py, self.inner.focus_node.clone()))
    }

    /// The value that caused the violation (if any).
    #[getter]
    pub fn value(&self) -> Option<PyObject> {
        self.inner
            .value
            .clone()
            .map(|v| Python::with_gil(|py| term_to_python(py, v)))
    }

    /// The human-readable message (if any).
    #[getter]
    pub fn message(&self) -> Option<String> {
        self.inner.result_message.clone()
    }

    /// The severity level as a string.
    #[getter]
    pub fn severity(&self) -> &'static str {
        match self.inner.result_severity {
            sparshacl::Severity::Violation => "Violation",
            sparshacl::Severity::Warning => "Warning",
            sparshacl::Severity::Info => "Info",
        }
    }

    pub fn __repr__(&self) -> String {
        format!(
            "ShaclValidationResult(severity='{}', focus_node={}, message={:?})",
            self.severity(),
            self.inner.focus_node,
            self.inner.result_message
        )
    }
}

/// Validates RDF data against SHACL shapes (convenience function).
///
/// :param shapes_data: The shapes graph as a Turtle string
/// :param data: The data to validate as a Turtle string
/// :return: A validation report
/// :raises ValueError: If the data cannot be parsed
/// :raises RuntimeError: If validation fails
///
/// >>> report = shacl_validate(
/// ...     shapes_data='''
/// ...         @prefix sh: <http://www.w3.org/ns/shacl#> .
/// ...         @prefix ex: <http://example.org/> .
/// ...         ex:Shape a sh:NodeShape ;
/// ...             sh:targetClass ex:Person ;
/// ...             sh:property [ sh:path ex:name ; sh:minCount 1 ] .
/// ...     ''',
/// ...     data='''
/// ...         @prefix ex: <http://example.org/> .
/// ...         ex:alice a ex:Person ; ex:name "Alice" .
/// ...     '''
/// ... )
/// >>> report.conforms
/// True
#[pyfunction]
pub fn shacl_validate(shapes_data: &str, data: &str) -> PyResult<PyShaclValidationReport> {
    let mut shapes = PyShaclShapesGraph::new();
    shapes.parse(shapes_data)?;

    let validator = PyShaclValidator::new(&shapes);
    validator.validate(data)
}

fn map_shacl_error(error: ShaclError) -> PyErr {
    match error {
        ShaclError::Parse(e) => PyValueError::new_err(e.to_string()),
        ShaclError::Validation(e) => PyRuntimeError::new_err(e.to_string()),
    }
}

fn term_to_python(py: Python<'_>, term: oxrdf::Term) -> PyObject {
    match term {
        oxrdf::Term::NamedNode(n) => PyNamedNode::from(n)
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind(),
        oxrdf::Term::BlankNode(b) => PyBlankNode::from(b)
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind(),
        oxrdf::Term::Literal(l) => PyLiteral::from(l)
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind(),
        #[cfg(feature = "rdf-12")]
        oxrdf::Term::Triple(t) => PyTriple::from(*t)
            .into_pyobject(py)
            .unwrap()
            .into_any()
            .unbind(),
    }
}
