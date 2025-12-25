//! JavaScript bindings for SHACL validation.

use crate::format_err;
use crate::model::JsTerm;
use js_sys::Array;
use oxrdfio::{RdfFormat, RdfParser, RdfSerializer};
use sparshacl::{ShaclValidator, ShapesGraph, ValidationReport, ValidationResult};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CUSTOM_SECTION: &str = r###"
/**
 * A SHACL shapes graph for validation.
 */
export class ShaclShapesGraph {
    /**
     * Creates a new empty shapes graph.
     */
    constructor();

    /**
     * Parses shapes from a string in Turtle format.
     *
     * @param data - The Turtle-formatted shapes data
     * @throws {Error} If the data cannot be parsed
     */
    parse(data: string): void;

    /**
     * The number of shapes in the shapes graph.
     */
    readonly size: number;

    /**
     * Returns true if the shapes graph is empty.
     */
    isEmpty(): boolean;
}

/**
 * A SHACL validator for validating RDF data against shapes.
 */
export class ShaclValidator {
    /**
     * Creates a new validator with the given shapes graph.
     *
     * @param shapes - The shapes graph to validate against
     */
    constructor(shapes: ShaclShapesGraph);

    /**
     * Validates data against the shapes graph.
     *
     * @param data - The data to validate (as Turtle string)
     * @returns A validation report
     * @throws {Error} If the data cannot be parsed or validation fails
     */
    validate(data: string): ShaclValidationReport;
}

/**
 * A SHACL validation report.
 *
 * Contains the results of validating data against a shapes graph.
 */
export class ShaclValidationReport {
    /**
     * Whether the data conforms to the shapes graph.
     * Returns true if there are no violations.
     */
    readonly conforms: boolean;

    /**
     * The number of violations.
     */
    readonly violationCount: number;

    /**
     * The number of warnings.
     */
    readonly warningCount: number;

    /**
     * The number of info results.
     */
    readonly infoCount: number;

    /**
     * Returns the validation results as an array.
     */
    results(): ShaclValidationResult[];

    /**
     * Returns the report as a Turtle string.
     */
    toTurtle(): string;
}

/**
 * A single SHACL validation result.
 */
export class ShaclValidationResult {
    /**
     * The focus node that was validated.
     */
    readonly focusNode: Term;

    /**
     * The value that caused the violation (if any).
     */
    readonly value?: Term;

    /**
     * The human-readable message (if any).
     */
    readonly message?: string;

    /**
     * The severity level as a string.
     */
    readonly severity: "Violation" | "Warning" | "Info";
}

/**
 * Validates RDF data against SHACL shapes (convenience function).
 *
 * @param shapesData - The shapes graph as a Turtle string
 * @param data - The data to validate as a Turtle string
 * @returns A validation report
 * @throws {Error} If the data cannot be parsed or validation fails
 */
export function shaclValidate(shapesData: string, data: string): ShaclValidationReport;
"###;

/// A SHACL shapes graph for validation.
#[wasm_bindgen(js_name = ShaclShapesGraph, skip_typescript)]
#[derive(Clone)]
pub struct JsShaclShapesGraph {
    inner: ShapesGraph,
}

#[wasm_bindgen(js_class = ShaclShapesGraph)]
impl JsShaclShapesGraph {
    /// Creates a new empty shapes graph.
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {
            inner: ShapesGraph::new(),
        }
    }

    /// Parses shapes from a string in Turtle format.
    pub fn parse(&mut self, data: &str) -> Result<(), JsValue> {
        use oxrdf::Graph;

        let mut graph = Graph::new();
        let parser = RdfParser::from_format(RdfFormat::Turtle);

        for quad_result in parser.for_reader(data.as_bytes()) {
            let quad = quad_result.map_err(|e| format_err!("{}", e))?;
            graph.insert(quad.as_ref());
        }

        self.inner = ShapesGraph::from_graph(&graph).map_err(|e| format_err!("{}", e))?;

        Ok(())
    }

    /// Returns the number of shapes in the shapes graph.
    #[wasm_bindgen(getter)]
    pub fn size(&self) -> usize {
        self.inner.len()
    }

    /// Returns true if the shapes graph is empty.
    #[wasm_bindgen(js_name = isEmpty)]
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// A SHACL validator for validating RDF data against shapes.
#[wasm_bindgen(js_name = ShaclValidator, skip_typescript)]
pub struct JsShaclValidator {
    inner: ShaclValidator,
}

#[wasm_bindgen(js_class = ShaclValidator)]
impl JsShaclValidator {
    /// Creates a new validator with the given shapes graph.
    #[wasm_bindgen(constructor)]
    pub fn new(shapes: &JsShaclShapesGraph) -> Self {
        Self {
            inner: ShaclValidator::new(shapes.inner.clone()),
        }
    }

    /// Validates data against the shapes graph.
    pub fn validate(&self, data: &str) -> Result<JsShaclValidationReport, JsValue> {
        use oxrdf::Graph;

        let mut graph = Graph::new();
        let parser = RdfParser::from_format(RdfFormat::Turtle);

        for quad_result in parser.for_reader(data.as_bytes()) {
            let quad = quad_result.map_err(|e| format_err!("{}", e))?;
            graph.insert(quad.as_ref());
        }

        let report = self.inner.validate(&graph).map_err(|e| format_err!("{}", e))?;

        Ok(JsShaclValidationReport { inner: report })
    }
}

/// A SHACL validation report.
#[wasm_bindgen(js_name = ShaclValidationReport, skip_typescript)]
#[derive(Clone)]
pub struct JsShaclValidationReport {
    inner: ValidationReport,
}

#[wasm_bindgen(js_class = ShaclValidationReport)]
impl JsShaclValidationReport {
    /// Whether the data conforms to the shapes graph.
    #[wasm_bindgen(getter)]
    pub fn conforms(&self) -> bool {
        self.inner.conforms()
    }

    /// The number of violations.
    #[wasm_bindgen(getter = violationCount)]
    pub fn violation_count(&self) -> usize {
        self.inner.violation_count()
    }

    /// The number of warnings.
    #[wasm_bindgen(getter = warningCount)]
    pub fn warning_count(&self) -> usize {
        self.inner.warning_count()
    }

    /// The number of info results.
    #[wasm_bindgen(getter = infoCount)]
    pub fn info_count(&self) -> usize {
        self.inner.info_count()
    }

    /// Returns the validation results as an array.
    pub fn results(&self) -> Array {
        self.inner
            .results()
            .iter()
            .map(|r| JsShaclValidationResult { inner: r.clone() })
            .map(JsValue::from)
            .collect()
    }

    /// Returns the report as a Turtle string.
    #[wasm_bindgen(js_name = toTurtle)]
    pub fn to_turtle(&self) -> Result<String, JsValue> {
        let graph = self.inner.to_graph();
        let mut buffer = Vec::new();

        {
            let mut serializer = RdfSerializer::from_format(RdfFormat::Turtle).for_writer(&mut buffer);
            for triple in graph.iter() {
                serializer
                    .serialize_triple(triple)
                    .map_err(|e| format_err!("{}", e))?;
            }
            serializer.finish().map_err(|e| format_err!("{}", e))?;
        }

        String::from_utf8(buffer).map_err(|e| format_err!("{}", e))
    }
}

/// A single SHACL validation result.
#[wasm_bindgen(js_name = ShaclValidationResult, skip_typescript)]
#[derive(Clone)]
pub struct JsShaclValidationResult {
    inner: ValidationResult,
}

#[wasm_bindgen(js_class = ShaclValidationResult)]
impl JsShaclValidationResult {
    /// The focus node that was validated.
    #[wasm_bindgen(getter = focusNode)]
    pub fn focus_node(&self) -> JsValue {
        JsTerm::from(self.inner.focus_node.clone()).into()
    }

    /// The value that caused the violation (if any).
    #[wasm_bindgen(getter)]
    pub fn value(&self) -> Option<JsValue> {
        self.inner
            .value
            .clone()
            .map(|v| JsTerm::from(v).into())
    }

    /// The human-readable message (if any).
    #[wasm_bindgen(getter)]
    pub fn message(&self) -> Option<String> {
        self.inner.result_message.clone()
    }

    /// The severity level as a string.
    #[wasm_bindgen(getter)]
    pub fn severity(&self) -> String {
        match self.inner.result_severity {
            sparshacl::Severity::Violation => "Violation".to_owned(),
            sparshacl::Severity::Warning => "Warning".to_owned(),
            sparshacl::Severity::Info => "Info".to_owned(),
        }
    }
}

/// Validates RDF data against SHACL shapes (convenience function).
#[wasm_bindgen(js_name = shaclValidate, skip_typescript)]
pub fn shacl_validate(shapes_data: &str, data: &str) -> Result<JsShaclValidationReport, JsValue> {
    let mut shapes = JsShaclShapesGraph::new();
    shapes.parse(shapes_data)?;

    let validator = JsShaclValidator::new(&shapes);
    validator.validate(data)
}
