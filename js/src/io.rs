use crate::format_err;
use crate::model::*;
use js_sys::{Reflect, try_iter};
use oxigraph::io::{RdfFormat, RdfParser, RdfSerializer};
use oxigraph::sparql::results::QueryResultsFormat;
use std::collections::BTreeMap;
use wasm_bindgen::prelude::*;

// TypeScript definitions for the I/O module
#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CUSTOM_SECTION: &str = r###"
/**
 * RDF serialization format
 */
export class RdfFormat {
    static readonly N3: RdfFormat;
    static readonly N_QUADS: RdfFormat;
    static readonly N_TRIPLES: RdfFormat;
    static readonly RDF_XML: RdfFormat;
    static readonly TRIG: RdfFormat;
    static readonly TURTLE: RdfFormat;
    static readonly JSON_LD: RdfFormat;

    readonly iri: string;
    readonly media_type: string;
    readonly file_extension: string;
    readonly name: string;
    readonly supports_datasets: boolean;

    static from_media_type(media_type: string): RdfFormat | null;
    static from_extension(extension: string): RdfFormat | null;
    toString(): string;
}

/**
 * SPARQL query results serialization format
 */
export class QueryResultsFormat {
    static readonly CSV: QueryResultsFormat;
    static readonly JSON: QueryResultsFormat;
    static readonly TSV: QueryResultsFormat;
    static readonly XML: QueryResultsFormat;

    readonly iri: string;
    readonly media_type: string;
    readonly file_extension: string;
    readonly name: string;

    static from_media_type(media_type: string): QueryResultsFormat | null;
    static from_extension(extension: string): QueryResultsFormat | null;
    toString(): string;
}

/**
 * Parses RDF data and returns an array of quads
 */
export function parse(
    data: string,
    format: RdfFormat,
    options?: {
        base_iri?: NamedNode | string;
        without_named_graphs?: boolean;
        rename_blank_nodes?: boolean;
        lenient?: boolean;
    }
): Quad[];

/**
 * Serializes an iterable of quads/triples to a string
 */
export function serialize(
    quads: Iterable<Quad>,
    format: RdfFormat,
    options?: {
        prefixes?: Record<string, string>;
        base_iri?: NamedNode | string;
    }
): string;
"###;

/// RDF serialization formats.
///
/// It currently supports the following formats:
///
/// * N-Triples
/// * N-Quads
/// * Turtle
/// * TriG
/// * N3
/// * RDF/XML
/// * JSON-LD
#[wasm_bindgen(js_name = RdfFormat, skip_typescript)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct JsRdfFormat {
    inner: RdfFormat,
}

#[wasm_bindgen(js_class = RdfFormat)]
impl JsRdfFormat {
    /// N3 format
    #[wasm_bindgen(getter, static_method_of = JsRdfFormat, js_name = N3)]
    pub fn n3() -> Self {
        Self {
            inner: RdfFormat::N3,
        }
    }

    /// N-Quads format
    #[wasm_bindgen(getter, static_method_of = JsRdfFormat, js_name = N_QUADS)]
    pub fn n_quads() -> Self {
        Self {
            inner: RdfFormat::NQuads,
        }
    }

    /// N-Triples format
    #[wasm_bindgen(getter, static_method_of = JsRdfFormat, js_name = N_TRIPLES)]
    pub fn n_triples() -> Self {
        Self {
            inner: RdfFormat::NTriples,
        }
    }

    /// RDF/XML format
    #[wasm_bindgen(getter, static_method_of = JsRdfFormat, js_name = RDF_XML)]
    pub fn rdf_xml() -> Self {
        Self {
            inner: RdfFormat::RdfXml,
        }
    }

    /// TriG format
    #[wasm_bindgen(getter, static_method_of = JsRdfFormat, js_name = TRIG)]
    pub fn trig() -> Self {
        Self {
            inner: RdfFormat::TriG,
        }
    }

    /// Turtle format
    #[wasm_bindgen(getter, static_method_of = JsRdfFormat, js_name = TURTLE)]
    pub fn turtle() -> Self {
        Self {
            inner: RdfFormat::Turtle,
        }
    }

    /// JSON-LD format
    #[wasm_bindgen(getter, static_method_of = JsRdfFormat, js_name = JSON_LD)]
    pub fn json_ld() -> Self {
        Self {
            inner: RdfFormat::JsonLd {
                profile: Default::default(),
            },
        }
    }

    /// The format canonical IRI according to the Unique URIs for file formats registry
    #[wasm_bindgen(getter)]
    pub fn iri(&self) -> String {
        self.inner.iri().to_owned()
    }

    /// The format IANA media type
    #[wasm_bindgen(getter)]
    pub fn media_type(&self) -> String {
        self.inner.media_type().to_owned()
    }

    /// The format IANA-registered file extension
    #[wasm_bindgen(getter)]
    pub fn file_extension(&self) -> String {
        self.inner.file_extension().to_owned()
    }

    /// The format name
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.inner.name().to_owned()
    }

    /// Whether the format supports RDF datasets (not just graphs)
    #[wasm_bindgen(getter)]
    pub fn supports_datasets(&self) -> bool {
        self.inner.supports_datasets()
    }

    /// Looks for a known format from a media type
    #[wasm_bindgen(js_name = from_media_type)]
    pub fn from_media_type(media_type: &str) -> Option<JsRdfFormat> {
        RdfFormat::from_media_type(media_type).map(|inner| Self { inner })
    }

    /// Looks for a known format from a file extension
    #[wasm_bindgen(js_name = from_extension)]
    pub fn from_extension(extension: &str) -> Option<JsRdfFormat> {
        RdfFormat::from_extension(extension).map(|inner| Self { inner })
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string(&self) -> String {
        self.inner.name().to_owned()
    }
}

impl From<RdfFormat> for JsRdfFormat {
    fn from(inner: RdfFormat) -> Self {
        Self { inner }
    }
}

impl From<JsRdfFormat> for RdfFormat {
    fn from(format: JsRdfFormat) -> Self {
        format.inner
    }
}

/// SPARQL query results serialization formats.
///
/// It currently supports the following formats:
///
/// * XML
/// * JSON
/// * CSV
/// * TSV
#[wasm_bindgen(js_name = QueryResultsFormat, skip_typescript)]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct JsQueryResultsFormat {
    inner: QueryResultsFormat,
}

#[wasm_bindgen(js_class = QueryResultsFormat)]
impl JsQueryResultsFormat {
    /// CSV format
    #[wasm_bindgen(getter, static_method_of = JsQueryResultsFormat, js_name = CSV)]
    pub fn csv() -> Self {
        Self {
            inner: QueryResultsFormat::Csv,
        }
    }

    /// JSON format
    #[wasm_bindgen(getter, static_method_of = JsQueryResultsFormat, js_name = JSON)]
    pub fn json() -> Self {
        Self {
            inner: QueryResultsFormat::Json,
        }
    }

    /// TSV format
    #[wasm_bindgen(getter, static_method_of = JsQueryResultsFormat, js_name = TSV)]
    pub fn tsv() -> Self {
        Self {
            inner: QueryResultsFormat::Tsv,
        }
    }

    /// XML format
    #[wasm_bindgen(getter, static_method_of = JsQueryResultsFormat, js_name = XML)]
    pub fn xml() -> Self {
        Self {
            inner: QueryResultsFormat::Xml,
        }
    }

    /// The format canonical IRI according to the Unique URIs for file formats registry
    #[wasm_bindgen(getter)]
    pub fn iri(&self) -> String {
        self.inner.iri().to_owned()
    }

    /// The format IANA media type
    #[wasm_bindgen(getter)]
    pub fn media_type(&self) -> String {
        self.inner.media_type().to_owned()
    }

    /// The format IANA-registered file extension
    #[wasm_bindgen(getter)]
    pub fn file_extension(&self) -> String {
        self.inner.file_extension().to_owned()
    }

    /// The format name
    #[wasm_bindgen(getter)]
    pub fn name(&self) -> String {
        self.inner.name().to_owned()
    }

    /// Looks for a known format from a media type
    #[wasm_bindgen(js_name = from_media_type)]
    pub fn from_media_type(media_type: &str) -> Option<JsQueryResultsFormat> {
        QueryResultsFormat::from_media_type(media_type).map(|inner| Self { inner })
    }

    /// Looks for a known format from a file extension
    #[wasm_bindgen(js_name = from_extension)]
    pub fn from_extension(extension: &str) -> Option<JsQueryResultsFormat> {
        QueryResultsFormat::from_extension(extension).map(|inner| Self { inner })
    }

    #[wasm_bindgen(js_name = toString)]
    pub fn to_string(&self) -> String {
        self.inner.name().to_owned()
    }
}

impl From<QueryResultsFormat> for JsQueryResultsFormat {
    fn from(inner: QueryResultsFormat) -> Self {
        Self { inner }
    }
}

impl From<JsQueryResultsFormat> for QueryResultsFormat {
    fn from(format: JsQueryResultsFormat) -> Self {
        format.inner
    }
}

/// Parses RDF data and returns an array of quads.
///
/// # Arguments
///
/// * `data` - The RDF data to parse as a string
/// * `format` - The RDF format of the data
/// * `options` - Optional parsing options (base_iri, without_named_graphs, rename_blank_nodes, lenient)
///
/// # Returns
///
/// An array of quads parsed from the data
///
/// # Example
///
/// ```javascript
/// import { parse, RdfFormat } from 'oxigraph';
///
/// const quads = parse('<s> <p> <o> .', RdfFormat.TURTLE, {
///   base_iri: 'http://example.com/'
/// });
/// ```
#[wasm_bindgen(skip_typescript)]
pub fn parse(data: &str, format: JsRdfFormat, options: &JsValue) -> Result<Box<[JsValue]>, JsValue> {
    // Parse options
    let mut base_iri = None;
    let mut without_named_graphs = false;
    let mut rename_blank_nodes = false;
    let mut lenient = false;

    if !options.is_undefined() && !options.is_null() {
        let js_base_iri = Reflect::get(options, &JsValue::from_str("base_iri"))?;
        base_iri = convert_base_iri(&js_base_iri)?;

        without_named_graphs =
            Reflect::get(options, &JsValue::from_str("without_named_graphs"))?.is_truthy();
        rename_blank_nodes =
            Reflect::get(options, &JsValue::from_str("rename_blank_nodes"))?.is_truthy();
        lenient = Reflect::get(options, &JsValue::from_str("lenient"))?.is_truthy();
    }

    // Configure parser
    let mut parser = RdfParser::from_format(format.inner);
    if let Some(base_iri) = base_iri {
        parser = parser
            .with_base_iri(base_iri)
            .map_err(|e| format_err!("Invalid base IRI: {}", e))?;
    }
    if without_named_graphs {
        parser = parser.without_named_graphs();
    }
    if rename_blank_nodes {
        parser = parser.rename_blank_nodes();
    }
    if lenient {
        parser = parser.lenient();
    }

    // Parse the data
    let quads = parser
        .for_reader(data.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(JsError::from)?;

    // Convert to JS values
    Ok(quads
        .into_iter()
        .map(|quad| JsQuad::from(quad).into())
        .collect::<Vec<_>>()
        .into_boxed_slice())
}

/// Serializes an iterable of quads/triples to a string.
///
/// # Arguments
///
/// * `quads` - An iterable of quads or triples to serialize
/// * `format` - The RDF format to serialize to
/// * `options` - Optional serialization options (prefixes, base_iri)
///
/// # Returns
///
/// A string containing the serialized RDF data
///
/// # Example
///
/// ```javascript
/// import { serialize, RdfFormat, namedNode, literal, quad } from 'oxigraph';
///
/// const q = quad(
///   namedNode('http://example.com/s'),
///   namedNode('http://example.com/p'),
///   literal('o')
/// );
/// const turtle = serialize([q], RdfFormat.TURTLE, {
///   prefixes: { 'ex': 'http://example.com/' }
/// });
/// ```
#[wasm_bindgen(skip_typescript)]
pub fn serialize(
    quads: &JsValue,
    format: JsRdfFormat,
    options: &JsValue,
) -> Result<String, JsValue> {
    // Parse options
    let mut prefixes = None;
    let mut base_iri = None;

    if !options.is_undefined() && !options.is_null() {
        let js_prefixes = Reflect::get(options, &JsValue::from_str("prefixes"))?;
        if !js_prefixes.is_undefined() && !js_prefixes.is_null() {
            let mut prefix_map = BTreeMap::new();
            let entries = js_sys::Object::entries(&js_sys::Object::from(js_prefixes));
            for i in 0..entries.length() {
                let entry = entries.get(i);
                let key = Reflect::get(&entry, &0.into())?
                    .as_string()
                    .ok_or_else(|| format_err!("Prefix name must be a string"))?;
                let value = Reflect::get(&entry, &1.into())?
                    .as_string()
                    .ok_or_else(|| format_err!("Prefix IRI must be a string"))?;
                prefix_map.insert(key, value);
            }
            prefixes = Some(prefix_map);
        }

        let js_base_iri = Reflect::get(options, &JsValue::from_str("base_iri"))?;
        base_iri = convert_base_iri(&js_base_iri)?;
    }

    // Configure serializer
    let mut serializer = RdfSerializer::from_format(format.inner);
    if let Some(prefixes) = prefixes {
        for (prefix_name, prefix_iri) in &prefixes {
            serializer = serializer
                .with_prefix(prefix_name, prefix_iri)
                .map_err(|e| {
                    format_err!("Invalid prefix {} IRI '{}': {}", prefix_name, prefix_iri, e)
                })?;
        }
    }
    if let Some(base_iri) = base_iri {
        serializer = serializer
            .with_base_iri(base_iri)
            .map_err(|e| format_err!("Invalid base IRI: {}", e))?;
    }

    // Serialize the quads
    let mut writer = Vec::new();
    let mut serializer = serializer.for_writer(&mut writer);

    if let Some(iter) = try_iter(quads)? {
        for item in iter {
            let item = item?;

            // Try to convert to a quad
            let quad = FROM_JS.with(|c| c.to_quad(&item))?;

            // Check if this is a named graph and the format doesn't support it
            if !quad.graph_name.is_default_graph() && !format.inner.supports_datasets() {
                return Err(format_err!(
                    "The {} format does not support named graphs",
                    format.inner.name()
                ));
            }

            serializer
                .serialize_quad(quad.as_ref())
                .map_err(JsError::from)?;
        }
    } else {
        return Err(format_err!("The quads parameter must be iterable"));
    }

    serializer.finish().map_err(JsError::from)?;

    Ok(String::from_utf8(writer).map_err(JsError::from)?)
}

fn convert_base_iri(value: &JsValue) -> Result<Option<String>, JsValue> {
    if value.is_null() || value.is_undefined() {
        Ok(None)
    } else if let Some(value) = value.as_string() {
        Ok(Some(value))
    } else if let JsTerm::NamedNode(value) = FROM_JS.with(|c| c.to_term(value))? {
        Ok(Some(value.value()))
    } else {
        Err(format_err!(
            "If provided, the base IRI must be a NamedNode or a string"
        ))
    }
}
