#![allow(clippy::inherent_to_string)]

use crate::format_err;
use crate::model::JsTerm;
use js_sys::{Array, Map};
use oxigraph::io::RdfSerializer;
use oxigraph::sparql::results::{
    QueryResultsFormat, QueryResultsParser, QueryResultsSerializer, ReaderQueryResultsParserOutput,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CUSTOM_SECTION: &str = r###"
/**
 * Parses SPARQL query results from a string.
 *
 * Supported formats:
 * - "json" or "application/sparql-results+json" - SPARQL JSON Results
 * - "xml" or "application/sparql-results+xml" - SPARQL XML Results
 * - "csv" or "text/csv" - SPARQL CSV
 * - "tsv" or "text/tab-separated-values" - SPARQL TSV
 *
 * @param input - The query results string to parse
 * @param format - The format of the query results (file extension or media type)
 * @returns Either a boolean (for ASK results) or an array of solution objects (for SELECT results)
 *
 * @example
 * ```javascript
 * // Parse JSON SELECT query results
 * const solutions = parseQueryResults(
 *   '{"head":{"vars":["s","p"]},"results":{"bindings":[{"s":{"type":"uri","value":"http://example.org/s"},"p":{"type":"uri","value":"http://example.org/p"}}]}}',
 *   'json'
 * );
 * // solutions is an array of Map objects: [Map { "s" => NamedNode(...), "p" => NamedNode(...) }]
 *
 * // Parse JSON ASK query results
 * const result = parseQueryResults('{"boolean":true}', 'json');
 * // result is true
 * ```
 */
export function parseQueryResults(input: string, format: string): boolean | Map<string, Term>[];

/**
 * Serializes SPARQL SELECT query results to a string.
 *
 * @param solutions - An array of solution maps (each map has variable names as keys and Terms as values)
 * @param variables - The list of variable names
 * @param format - The serialization format (json, xml, csv, tsv or their media types)
 * @returns The serialized results string
 */
export function serializeQuerySolutions(
    solutions: Map<string, Term>[],
    variables: string[],
    format: string
): string;

/**
 * Serializes a SPARQL ASK query result to a string.
 *
 * @param value - The boolean result value
 * @param format - The serialization format (json, xml, csv, tsv or their media types)
 * @returns The serialized result string
 */
export function serializeQueryBoolean(value: boolean, format: string): string;

/**
 * Result of a SPARQL SELECT query.
 *
 * This wraps an array of solution maps and provides serialization capabilities.
 *
 * @example
 * ```javascript
 * const solutions = new QuerySolutions(
 *   [new Map([["s", NamedNode("http://example.com")]])],
 *   ["s"]
 * );
 * console.log(solutions.variables); // ["s"]
 * const json = solutions.serialize("json");
 * // Iterate using values()
 * for (const solution of solutions.values()) {
 *   console.log(solution.get("s"));
 * }
 * ```
 */
export class QuerySolutions {
    /**
     * Creates a new QuerySolutions instance.
     *
     * @param solutions - Array of solution maps (each map has variable names as keys and Terms as values)
     * @param variables - List of variable names
     */
    constructor(solutions: Map<string, Term>[], variables: string[]);

    /**
     * Gets the ordered list of all variables that could appear in the query results.
     */
    readonly variables: string[];

    /**
     * Serializes the query solutions to a string.
     *
     * Supported formats:
     * - "json" or "application/sparql-results+json" - SPARQL JSON Results
     * - "xml" or "application/sparql-results+xml" - SPARQL XML Results
     * - "csv" or "text/csv" - SPARQL CSV
     * - "tsv" or "text/tab-separated-values" - SPARQL TSV
     *
     * @param format - The serialization format (file extension or media type)
     * @returns The serialized results string
     */
    serialize(format: string): string;

    /**
     * Returns the solutions as an array for iteration.
     *
     * @returns Array of solution maps
     */
    values(): Map<string, Term>[];

    [Symbol.iterator](): Iterator<Map<string, Term>>;
}

/**
 * Result of a SPARQL ASK query.
 *
 * This wraps a boolean value and can be used in boolean contexts.
 *
 * @example
 * ```javascript
 * const result = new QueryBoolean(true);
 * if (result) {
 *   console.log("Query returned true");
 * }
 * const json = result.serialize("json");
 * ```
 */
export class QueryBoolean {
    /**
     * Creates a new QueryBoolean instance.
     *
     * @param value - The boolean value
     */
    constructor(value: boolean);

    /**
     * Serializes the boolean result to a string.
     *
     * Supported formats:
     * - "json" or "application/sparql-results+json" - SPARQL JSON Results
     * - "xml" or "application/sparql-results+xml" - SPARQL XML Results
     * - "csv" or "text/csv" - SPARQL CSV
     * - "tsv" or "text/tab-separated-values" - SPARQL TSV
     *
     * @param format - The serialization format (file extension or media type)
     * @returns The serialized result string
     */
    serialize(format: string): string;

    /**
     * Returns the boolean value (allows the object to be used in boolean contexts).
     */
    valueOf(): boolean;
}

/**
 * Result of a SPARQL CONSTRUCT or DESCRIBE query.
 *
 * This wraps an array of triples/quads and provides serialization capabilities.
 *
 * @example
 * ```javascript
 * const triples = new QueryTriples([
 *   quad(NamedNode("http://example.com/s"), NamedNode("http://example.com/p"), NamedNode("http://example.com/o"))
 * ]);
 * const ntriples = triples.serialize("nt");
 * // Iterate using values()
 * for (const triple of triples.values()) {
 *   console.log(triple);
 * }
 * ```
 */
export class QueryTriples {
    /**
     * Creates a new QueryTriples instance.
     *
     * @param triples - Array of quads/triples
     */
    constructor(triples: Quad[]);

    /**
     * Serializes the triples to a string.
     *
     * Supported RDF formats include:
     * - "nt" or "application/n-triples" - N-Triples
     * - "ttl" or "text/turtle" - Turtle
     * - "rdf" or "application/rdf+xml" - RDF/XML
     * - "nq" or "application/n-quads" - N-Quads
     * - "trig" or "application/trig" - TriG
     *
     * @param format - The serialization format (file extension or media type)
     * @returns The serialized RDF string
     */
    serialize(format: string): string;

    /**
     * Returns the triples as an array for iteration.
     *
     * @returns Array of quads
     */
    values(): Quad[];

    [Symbol.iterator](): Iterator<Quad>;
}
"###;

/// Parses SPARQL query results from a string
///
/// ```javascript
/// // Parse JSON SELECT query results
/// const solutions = parseQueryResults(
///   '{"head":{"vars":["s","p"]},"results":{"bindings":[{"s":{"type":"uri","value":"http://example.org/s"},"p":{"type":"uri","value":"http://example.org/p"}}]}}',
///   'json'
/// );
/// // solutions is an array of Map objects: [Map { "s" => NamedNode(...), "p" => NamedNode(...) }]
///
/// // Parse JSON ASK query results
/// const result = parseQueryResults('{"boolean":true}', 'json');
/// // result is true
/// ```
#[wasm_bindgen(js_name = parseQueryResults, skip_typescript)]
pub fn parse_query_results(input: &str, format: &str) -> Result<JsValue, JsValue> {
    console_error_panic_hook::set_once();

    let results_format = query_results_format(format)?;
    let parser = QueryResultsParser::from_format(results_format);

    let results = parser
        .for_reader(input.as_bytes())
        .map_err(|e| format_err!("Error parsing query results: {}", e))?;

    match results {
        ReaderQueryResultsParserOutput::Boolean(value) => Ok(JsValue::from_bool(value)),
        ReaderQueryResultsParserOutput::Solutions(solutions_reader) => {
            let results = Array::new();
            for solution in solutions_reader {
                let solution =
                    solution.map_err(|e| format_err!("Error reading solution: {}", e))?;
                let result = Map::new();
                for (variable, value) in solution.iter() {
                    result.set(
                        &variable.as_str().into(),
                        &JsTerm::from(value.clone()).into(),
                    );
                }
                results.push(&result.into());
            }
            Ok(results.into())
        }
    }
}

/// Serializes SPARQL SELECT query results to a string
#[wasm_bindgen(js_name = serializeQuerySolutions, skip_typescript)]
pub fn serialize_query_solutions(
    solutions: &JsValue,
    variables: Vec<String>,
    format: &str,
) -> Result<String, JsValue> {
    let format = query_results_format(format)?;

    // Convert variable names to Variable objects
    let vars: Vec<oxigraph::sparql::Variable> = variables
        .iter()
        .map(|v| {
            oxigraph::sparql::Variable::new(v.strip_prefix('?').unwrap_or(v))
                .map_err(|e| format_err!("Invalid variable name '{}': {}", v, e))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let mut serializer = QueryResultsSerializer::from_format(format)
        .serialize_solutions_to_writer(Vec::new(), vars.clone())
        .map_err(JsError::from)?;

    // Iterate over solutions
    if let Some(iter) = js_sys::try_iter(solutions)? {
        for item in iter {
            let item = item?;
            // Each item should be a Map
            let map = Map::from(item);

            // Build values in order of variables
            let mut values: Vec<Option<oxigraph::model::Term>> = vec![None; vars.len()];

            // Extract variable bindings
            map.for_each(&mut |value, key| {
                if let Some(var_name) = key.as_string() {
                    let var_name = var_name.strip_prefix('?').unwrap_or(&var_name);
                    // Find the position of this variable
                    if let Some(pos) = vars.iter().position(|v| v.as_str() == var_name) {
                        if let Ok(term) = crate::model::FROM_JS.with(|c| c.to_term(&value)) {
                            if let Ok(term) = oxigraph::model::Term::try_from(term) {
                                values[pos] = Some(term);
                            }
                        }
                    }
                }
            });

            // Create a QuerySolution from (variables, values)
            let solution = oxigraph::sparql::QuerySolution::from((vars.clone(), values));
            serializer.serialize(&solution).map_err(JsError::from)?;
        }
    } else {
        return Err(format_err!("solutions must be iterable"));
    }

    let bytes = serializer.finish().map_err(JsError::from)?;
    Ok(String::from_utf8(bytes).map_err(JsError::from)?)
}

/// Serializes a SPARQL ASK query result to a string
#[wasm_bindgen(js_name = serializeQueryBoolean, skip_typescript)]
pub fn serialize_query_boolean(value: bool, format: &str) -> Result<String, JsValue> {
    let format = query_results_format(format)?;
    let bytes = QueryResultsSerializer::from_format(format)
        .serialize_boolean_to_writer(Vec::new(), value)
        .map_err(JsError::from)?;
    Ok(String::from_utf8(bytes).map_err(JsError::from)?)
}

/// Parse a query results format from a string (extension or media type)
fn query_results_format(format: &str) -> Result<QueryResultsFormat, JsValue> {
    if format.contains('/') {
        QueryResultsFormat::from_media_type(format).ok_or_else(|| {
            format_err!(
                "Not supported SPARQL query results format media type: {}",
                format
            )
        })
    } else {
        QueryResultsFormat::from_extension(format).ok_or_else(|| {
            format_err!(
                "Not supported SPARQL query results format extension: {}",
                format
            )
        })
    }
}

/// Parse an RDF format from a string (extension or media type)
fn rdf_format(format: &str) -> Result<oxigraph::io::RdfFormat, JsValue> {
    if format.contains('/') {
        oxigraph::io::RdfFormat::from_media_type(format)
            .ok_or_else(|| format_err!("Not supported RDF format media type: {}", format))
    } else {
        oxigraph::io::RdfFormat::from_extension(format)
            .ok_or_else(|| format_err!("Not supported RDF format extension: {}", format))
    }
}

/// Result of a SPARQL SELECT query
#[wasm_bindgen(js_name = QuerySolutions, skip_typescript)]
pub struct JsQuerySolutions {
    solutions: Array,
    variables: Vec<String>,
}

#[wasm_bindgen(js_class = QuerySolutions)]
impl JsQuerySolutions {
    /// Creates a new QuerySolutions instance
    #[wasm_bindgen(constructor)]
    pub fn new(solutions: &JsValue, variables: Vec<String>) -> Result<JsQuerySolutions, JsValue> {
        console_error_panic_hook::set_once();

        // Convert solutions to an Array
        let solutions_array = if let Some(iter) = js_sys::try_iter(solutions)? {
            let arr = Array::new();
            for item in iter {
                arr.push(&item?);
            }
            arr
        } else {
            return Err(format_err!("solutions must be iterable"));
        };

        Ok(JsQuerySolutions {
            solutions: solutions_array,
            variables,
        })
    }

    /// Gets the ordered list of all variables that could appear in the query results
    #[wasm_bindgen(getter)]
    pub fn variables(&self) -> Box<[JsValue]> {
        self.variables
            .iter()
            .map(|v| JsValue::from_str(v))
            .collect::<Vec<_>>()
            .into_boxed_slice()
    }

    /// Serializes the query solutions to a string
    pub fn serialize(&self, format: &str) -> Result<String, JsValue> {
        serialize_query_solutions(&self.solutions, self.variables.clone(), format)
    }

    /// Returns the solutions as an array (for iteration)
    pub fn values(&self) -> Array {
        self.solutions.clone()
    }

    // Symbol.iterator implementation - must be manually wired up in JavaScript
    // as wasm-bindgen doesn't support computed property names
    #[wasm_bindgen(skip_typescript)]
    pub fn __iterator(&self) -> JsValue {
        self.solutions.values().into()
    }
}

/// Result of a SPARQL ASK query
#[wasm_bindgen(js_name = QueryBoolean, skip_typescript)]
pub struct JsQueryBoolean {
    value: bool,
}

#[wasm_bindgen(js_class = QueryBoolean)]
impl JsQueryBoolean {
    /// Creates a new QueryBoolean instance
    #[wasm_bindgen(constructor)]
    pub fn new(value: bool) -> JsQueryBoolean {
        console_error_panic_hook::set_once();
        JsQueryBoolean { value }
    }

    /// Serializes the boolean result to a string
    pub fn serialize(&self, format: &str) -> Result<String, JsValue> {
        serialize_query_boolean(self.value, format)
    }

    /// Returns the boolean value (allows the object to be used in boolean contexts)
    #[wasm_bindgen(js_name = valueOf)]
    pub fn value_of(&self) -> bool {
        self.value
    }
}

/// Result of a SPARQL CONSTRUCT or DESCRIBE query
#[wasm_bindgen(js_name = QueryTriples, skip_typescript)]
pub struct JsQueryTriples {
    triples: Array,
}

#[wasm_bindgen(js_class = QueryTriples)]
impl JsQueryTriples {
    /// Creates a new QueryTriples instance
    #[wasm_bindgen(constructor)]
    pub fn new(triples: &JsValue) -> Result<JsQueryTriples, JsValue> {
        console_error_panic_hook::set_once();

        // Convert triples to an Array
        let triples_array = if let Some(iter) = js_sys::try_iter(triples)? {
            let arr = Array::new();
            for item in iter {
                arr.push(&item?);
            }
            arr
        } else {
            return Err(format_err!("triples must be iterable"));
        };

        Ok(JsQueryTriples {
            triples: triples_array,
        })
    }

    /// Serializes the triples to a string
    pub fn serialize(&self, format: &str) -> Result<String, JsValue> {
        let rdf_format = rdf_format(format)?;
        let mut serializer = RdfSerializer::from_format(rdf_format).for_writer(Vec::new());

        // Iterate over triples and serialize them
        for i in 0..self.triples.length() {
            let triple_js = self.triples.get(i);
            let quad = crate::model::FROM_JS.with(|c| c.to_quad(&triple_js))?;
            // try_from returns Result<Triple, Infallible> which never fails, so we can use expect
            let triple =
                oxigraph::model::Triple::try_from(quad).expect("Triple conversion should not fail");
            serializer
                .serialize_triple(&triple)
                .map_err(JsError::from)?;
        }

        let bytes = serializer.finish().map_err(JsError::from)?;
        Ok(String::from_utf8(bytes).map_err(JsError::from)?)
    }

    /// Returns the triples as an array (for iteration)
    pub fn values(&self) -> Array {
        self.triples.clone()
    }

    // Symbol.iterator implementation - must be manually wired up in JavaScript
    // as wasm-bindgen doesn't support computed property names
    #[wasm_bindgen(skip_typescript)]
    pub fn __iterator(&self) -> JsValue {
        self.triples.values().into()
    }
}
