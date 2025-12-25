#![allow(clippy::inherent_to_string)]

use crate::format_err;
use crate::model::JsTerm;
use js_sys::{Array, Map};
use oxigraph::sparql::results::{
    QueryResultsFormat, QueryResultsParser, QueryResultsSerializer,
    ReaderQueryResultsParserOutput,
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
                let solution = solution.map_err(|e| format_err!("Error reading solution: {}", e))?;
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
            let solution =
                oxigraph::sparql::QuerySolution::from((vars.clone(), values));
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
