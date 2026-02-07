use crate::format_err;
use crate::model::{FROM_JS, JsQuad, JsTerm};
use crate::utils::{
    IntoAsyncIter, make_async_iterator_iterable, make_iterator_iterable, try_async_iter,
};
use js_sys::{IntoIter, Reflect, Uint8Array, try_iter};
use oxigraph::io::{RdfFormat, RdfParseError, RdfParser, ReaderQuadParser};
use oxigraph::model::GraphName;
use oxrdfio::TokioAsyncReaderQuadParser;
use std::error::Error;
use std::fmt::{Debug, Formatter};
use std::io::{Cursor, Read};
use std::pin::{Pin, pin};
use std::task::{Context, Poll, ready};
use std::{fmt, io};
use tokio::io::{AsyncRead, ReadBuf};
use wasm_bindgen::prelude::*;

#[wasm_bindgen(typescript_custom_section)]
const TYPESCRIPT_CUSTOM_SECTION: &str = r###"
function parse(
    input: string | UInt8Array,
    options: {
        base_iri?: NamedNode | string;
        format: string;
        to_graph_name?: BlankNode | DefaultGraph | NamedNode;
        lenient?: boolean;
    }
): Quad[];
function parse(
    input: Iterable<string | UInt8Array>,
    options: {
        base_iri?: NamedNode | string;
        format: string;
        to_graph_name?: BlankNode | DefaultGraph | NamedNode;
        lenient?: boolean;
    }
): IterableIterator<Quad>;
function parse(
    input: AsyncIterable<string | UInt8Array>,
    options: {
        base_iri?: NamedNode | string;
        format: string;
        to_graph_name?: BlankNode | DefaultGraph | NamedNode;
        lenient?: boolean;
    }
): AsyncIterableIterator<Quad>;
"###;

#[wasm_bindgen(skip_typescript)]
pub fn parse(input: &JsValue, options: &JsValue) -> Result<JsValue, JsValue> {
    // Parsing options
    let mut format = None;
    let mut base_iri = None;
    let mut to_graph_name = None;
    let mut lenient = false;
    if !options.is_undefined() && !options.is_null() {
        if let Some(format_str) = Reflect::get(options, &JsValue::from_str("format"))?.as_string() {
            format = Some(rdf_format(&format_str)?);
        }
        base_iri = convert_base_iri(&Reflect::get(options, &JsValue::from_str("base_iri"))?)?;
        let to_graph_name_js = Reflect::get(options, &JsValue::from_str("to_graph_name"))?;
        to_graph_name = FROM_JS.with(|c| c.to_optional_term(&to_graph_name_js))?;
        lenient = Reflect::get(options, &JsValue::from_str("lenient"))?.is_truthy();
    }
    let format = format
        .ok_or_else(|| format_err!("The format option should be provided as a second argument of Store.load like parse(my_content, {{format: 'nt'}}"))?;

    let mut parser = RdfParser::from_format(format);
    if let Some(to_graph_name) = to_graph_name {
        parser = parser.with_default_graph(GraphName::try_from(to_graph_name)?);
    }
    if let Some(base_iri) = base_iri {
        parser = parser.with_base_iri(base_iri).map_err(JsError::from)?;
    }
    if lenient {
        parser = parser.lenient();
    }
    if let Some(buffer) = buffer_from_js_value(input) {
        Ok(parser
            .for_slice(&buffer)
            .map(|v| v.map(JsQuad::from))
            .collect::<Result<Vec<_>, _>>()
            .map_err(JsError::from)?
            .into())
    } else if let Some(iterator) = try_iter(input)? {
        make_iterator_iterable(ParserIterator {
            parser: parser.for_reader(BytesInput::from(iterator)),
        })
    } else if let Some(iterator) = try_async_iter(input)? {
        make_async_iterator_iterable(AsyncParserIterator {
            parser: parser.for_tokio_async_reader(AsyncBytesInput::from(iterator)),
        })
    } else {
        Err(format_err!(
            "The input must be a string, Uint8Array or a sync or async iterator of string or Uint8Array"
        ))
    }
}

pub fn rdf_format(format: &str) -> Result<RdfFormat, JsValue> {
    if format.contains('/') {
        RdfFormat::from_media_type(format)
            .ok_or_else(|| format_err!("Not supported RDF format media type: {}", format))
    } else {
        RdfFormat::from_extension(format)
            .ok_or_else(|| format_err!("Not supported RDF format extension: {}", format))
    }
}

pub fn convert_base_iri(value: &JsValue) -> Result<Option<String>, JsValue> {
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

pub struct BytesInput {
    current: Cursor<Vec<u8>>,
    iterator: IntoIter,
}

impl From<IntoIter> for BytesInput {
    fn from(iterator: IntoIter) -> Self {
        Self {
            current: Cursor::new(Vec::new()),
            iterator,
        }
    }
}

impl Read for BytesInput {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            let count = self.current.read(buf)?;
            if count > 0 {
                return Ok(count);
            }
            let Some(value) = self.iterator.next() else {
                return Ok(0);
            };
            let value = value.map_err(|e| io::Error::other(WrappedJsValue(e)))?;
            self.current = Cursor::new(if let Some(buffer) = buffer_from_js_value(&value) {
                buffer
            } else {
                return Err(io::Error::other(WrappedJsValue(format_err!(
                    "The input iterator must yield a string or a Uint8Array"
                ))));
            });
        }
    }
}

pub struct AsyncBytesInput {
    current: Cursor<Vec<u8>>,
    iterator: IntoAsyncIter,
}

impl From<IntoAsyncIter> for AsyncBytesInput {
    fn from(iterator: IntoAsyncIter) -> Self {
        Self {
            current: Cursor::new(Vec::new()),
            iterator,
        }
    }
}

impl AsyncRead for AsyncBytesInput {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        loop {
            if let Err(e) = ready!(pin!(&mut self.current).poll_read(cx, buf)) {
                return Poll::Ready(Err(e));
            }
            if !buf.filled().is_empty() {
                return Poll::Ready(Ok(()));
            }
            let Some(value) = ready!(self.iterator.poll_next(cx)) else {
                return Poll::Ready(Ok(()));
            };
            let value = value.map_err(|e| io::Error::other(WrappedJsValue(e)))?;
            self.current = Cursor::new(if let Some(buffer) = buffer_from_js_value(&value) {
                buffer
            } else {
                return Poll::Ready(Err(io::Error::other(WrappedJsValue(format_err!(
                    "The input iterator must yield a string or a Uint8Array"
                )))));
            });
        }
    }
}

pub fn buffer_from_js_value(value: &JsValue) -> Option<Vec<u8>> {
    if let Some(value) = value.as_string() {
        Some(value.into_bytes())
    } else if value.has_type::<Uint8Array>() {
        Some(Uint8Array::from(value.clone()).to_vec()) // TODO: we can likely this copy if we read from this type
    } else {
        None
    }
}

#[wasm_bindgen(skip_typescript, private)]
pub struct ParserIterator {
    parser: ReaderQuadParser<BytesInput>,
}

#[wasm_bindgen]
impl ParserIterator {
    pub fn next(&mut self) -> Result<ParserIteratorResult, JsValue> {
        Ok(ParserIteratorResult(
            self.parser
                .next()
                .transpose()
                .map_err(convert_rdf_parse_error)?
                .map(JsQuad::from),
        ))
    }
}

#[wasm_bindgen(skip_typescript, private)]
pub struct AsyncParserIterator {
    parser: TokioAsyncReaderQuadParser<AsyncBytesInput>,
}

#[wasm_bindgen]
impl AsyncParserIterator {
    pub async fn next(&mut self) -> Result<ParserIteratorResult, JsValue> {
        Ok(ParserIteratorResult(
            self.parser
                .next()
                .await
                .transpose()
                .map_err(convert_rdf_parse_error)?
                .map(JsQuad::from),
        ))
    }
}

#[wasm_bindgen(skip_typescript, private, getter_with_clone)]
pub struct ParserIteratorResult(Option<JsQuad>);

#[wasm_bindgen]
impl ParserIteratorResult {
    #[wasm_bindgen(getter)]
    pub fn done(&self) -> bool {
        self.0.is_none()
    }

    #[wasm_bindgen(getter)]
    pub fn value(&self) -> Option<JsQuad> {
        self.0.clone()
    }
}

// Wrap a JsValue in something implementing Error
#[derive(Debug)]
struct WrappedJsValue(JsValue);

impl fmt::Display for WrappedJsValue {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl Error for WrappedJsValue {}

// SAFETY: this is bad, but we are only doing single thread execution
#[expect(unsafe_code)]
unsafe impl Send for WrappedJsValue {}
// SAFETY: this is bad, but we are only doing single thread execution
#[expect(unsafe_code)]
unsafe impl Sync for WrappedJsValue {}

fn convert_rdf_parse_error(error: RdfParseError) -> JsValue {
    match error {
        RdfParseError::Io(error) => convert_io_error(error),
        RdfParseError::Syntax(error) => JsError::from(error).into(),
    }
}

fn convert_io_error(error: io::Error) -> JsValue {
    match error.downcast() {
        Ok(WrappedJsValue(error)) => error,
        Err(error) => JsError::from(error).into(),
    }
}
