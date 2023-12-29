//! Implementation of [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)

use crate::error::{ParseError, SyntaxError};
#[cfg(feature = "async-tokio")]
use json_event_parser::ToTokioAsyncWriteJsonWriter;
use json_event_parser::{FromReadJsonReader, JsonEvent, ToWriteJsonWriter};
use oxrdf::vocab::rdf;
use oxrdf::Variable;
use oxrdf::*;
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::mem::take;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncWrite;

/// This limit is set in order to avoid stack overflow error when parsing nested triples due to too many recursive calls.
/// The actual limit value is a wet finger compromise between not failing to parse valid files and avoiding to trigger stack overflow errors.
const MAX_NUMBER_OF_NESTED_TRIPLES: usize = 128;

pub fn write_boolean_json_result<W: Write>(write: W, value: bool) -> io::Result<W> {
    let mut writer = ToWriteJsonWriter::new(write);
    for event in inner_write_boolean_json_result(value) {
        writer.write_event(event)?;
    }
    writer.finish()
}

#[cfg(feature = "async-tokio")]
pub async fn tokio_async_write_boolean_json_result<W: AsyncWrite + Unpin>(
    write: W,
    value: bool,
) -> io::Result<W> {
    let mut writer = ToTokioAsyncWriteJsonWriter::new(write);
    for event in inner_write_boolean_json_result(value) {
        writer.write_event(event).await?;
    }
    writer.finish()
}

fn inner_write_boolean_json_result(value: bool) -> [JsonEvent<'static>; 7] {
    [
        JsonEvent::StartObject,
        JsonEvent::ObjectKey("head".into()),
        JsonEvent::StartObject,
        JsonEvent::EndObject,
        JsonEvent::ObjectKey("boolean".into()),
        JsonEvent::Boolean(value),
        JsonEvent::EndObject,
    ]
}

pub struct ToWriteJsonSolutionsWriter<W: Write> {
    inner: InnerJsonSolutionsWriter,
    writer: ToWriteJsonWriter<W>,
}

impl<W: Write> ToWriteJsonSolutionsWriter<W> {
    pub fn start(write: W, variables: &[Variable]) -> io::Result<Self> {
        let mut writer = ToWriteJsonWriter::new(write);
        let mut buffer = Vec::with_capacity(48);
        let inner = InnerJsonSolutionsWriter::start(&mut buffer, variables);
        Self::do_write(&mut writer, buffer)?;
        Ok(Self { inner, writer })
    }

    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> io::Result<()> {
        let mut buffer = Vec::with_capacity(48);
        self.inner.write(&mut buffer, solution);
        Self::do_write(&mut self.writer, buffer)
    }

    pub fn finish(mut self) -> io::Result<W> {
        let mut buffer = Vec::with_capacity(4);
        self.inner.finish(&mut buffer);
        Self::do_write(&mut self.writer, buffer)?;
        self.writer.finish()
    }

    fn do_write(writer: &mut ToWriteJsonWriter<W>, output: Vec<JsonEvent<'_>>) -> io::Result<()> {
        for event in output {
            writer.write_event(event)?;
        }
        Ok(())
    }
}

#[cfg(feature = "async-tokio")]
pub struct ToTokioAsyncWriteJsonSolutionsWriter<W: AsyncWrite + Unpin> {
    inner: InnerJsonSolutionsWriter,
    writer: ToTokioAsyncWriteJsonWriter<W>,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> ToTokioAsyncWriteJsonSolutionsWriter<W> {
    pub async fn start(write: W, variables: &[Variable]) -> io::Result<Self> {
        let mut writer = ToTokioAsyncWriteJsonWriter::new(write);
        let mut buffer = Vec::with_capacity(48);
        let inner = InnerJsonSolutionsWriter::start(&mut buffer, variables);
        Self::do_write(&mut writer, buffer).await?;
        Ok(Self { inner, writer })
    }

    pub async fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> io::Result<()> {
        let mut buffer = Vec::with_capacity(48);
        self.inner.write(&mut buffer, solution);
        Self::do_write(&mut self.writer, buffer).await
    }

    pub async fn finish(mut self) -> io::Result<W> {
        let mut buffer = Vec::with_capacity(4);
        self.inner.finish(&mut buffer);
        Self::do_write(&mut self.writer, buffer).await?;
        self.writer.finish()
    }

    async fn do_write(
        writer: &mut ToTokioAsyncWriteJsonWriter<W>,
        output: Vec<JsonEvent<'_>>,
    ) -> io::Result<()> {
        for event in output {
            writer.write_event(event).await?;
        }
        Ok(())
    }
}

struct InnerJsonSolutionsWriter;

impl InnerJsonSolutionsWriter {
    fn start<'a>(output: &mut Vec<JsonEvent<'a>>, variables: &'a [Variable]) -> Self {
        output.push(JsonEvent::StartObject);
        output.push(JsonEvent::ObjectKey("head".into()));
        output.push(JsonEvent::StartObject);
        output.push(JsonEvent::ObjectKey("vars".into()));
        output.push(JsonEvent::StartArray);
        for variable in variables {
            output.push(JsonEvent::String(variable.as_str().into()));
        }
        output.push(JsonEvent::EndArray);
        output.push(JsonEvent::EndObject);
        output.push(JsonEvent::ObjectKey("results".into()));
        output.push(JsonEvent::StartObject);
        output.push(JsonEvent::ObjectKey("bindings".into()));
        output.push(JsonEvent::StartArray);
        Self {}
    }

    #[allow(clippy::unused_self)]
    fn write<'a>(
        &self,
        output: &mut Vec<JsonEvent<'a>>,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) {
        output.push(JsonEvent::StartObject);
        for (variable, value) in solution {
            output.push(JsonEvent::ObjectKey(variable.as_str().into()));
            write_json_term(output, value);
        }
        output.push(JsonEvent::EndObject);
    }

    #[allow(clippy::unused_self)]
    fn finish(self, output: &mut Vec<JsonEvent<'_>>) {
        output.push(JsonEvent::EndArray);
        output.push(JsonEvent::EndObject);
        output.push(JsonEvent::EndObject);
    }
}

fn write_json_term<'a>(output: &mut Vec<JsonEvent<'a>>, term: TermRef<'a>) {
    match term {
        TermRef::NamedNode(uri) => {
            output.push(JsonEvent::StartObject);
            output.push(JsonEvent::ObjectKey("type".into()));
            output.push(JsonEvent::String("uri".into()));
            output.push(JsonEvent::ObjectKey("value".into()));
            output.push(JsonEvent::String(uri.as_str().into()));
            output.push(JsonEvent::EndObject);
        }
        TermRef::BlankNode(bnode) => {
            output.push(JsonEvent::StartObject);
            output.push(JsonEvent::ObjectKey("type".into()));
            output.push(JsonEvent::String("bnode".into()));
            output.push(JsonEvent::ObjectKey("value".into()));
            output.push(JsonEvent::String(bnode.as_str().into()));
            output.push(JsonEvent::EndObject);
        }
        TermRef::Literal(literal) => {
            output.push(JsonEvent::StartObject);
            output.push(JsonEvent::ObjectKey("type".into()));
            output.push(JsonEvent::String("literal".into()));
            output.push(JsonEvent::ObjectKey("value".into()));
            output.push(JsonEvent::String(literal.value().into()));
            if let Some(language) = literal.language() {
                output.push(JsonEvent::ObjectKey("xml:lang".into()));
                output.push(JsonEvent::String(language.into()));
            } else if !literal.is_plain() {
                output.push(JsonEvent::ObjectKey("datatype".into()));
                output.push(JsonEvent::String(literal.datatype().as_str().into()));
            }
            output.push(JsonEvent::EndObject);
        }
        #[cfg(feature = "rdf-star")]
        TermRef::Triple(triple) => {
            output.push(JsonEvent::StartObject);
            output.push(JsonEvent::ObjectKey("type".into()));
            output.push(JsonEvent::String("triple".into()));
            output.push(JsonEvent::ObjectKey("value".into()));
            output.push(JsonEvent::StartObject);
            output.push(JsonEvent::ObjectKey("subject".into()));
            write_json_term(output, triple.subject.as_ref().into());
            output.push(JsonEvent::ObjectKey("predicate".into()));
            write_json_term(output, triple.predicate.as_ref().into());
            output.push(JsonEvent::ObjectKey("object".into()));
            write_json_term(output, triple.object.as_ref());
            output.push(JsonEvent::EndObject);
            output.push(JsonEvent::EndObject);
        }
    }
}

pub enum JsonQueryResultsReader<R: Read> {
    Solutions {
        variables: Vec<Variable>,
        solutions: JsonSolutionsReader<R>,
    },
    Boolean(bool),
}

impl<R: Read> JsonQueryResultsReader<R> {
    pub fn read(read: R) -> Result<Self, ParseError> {
        let mut reader = FromReadJsonReader::new(read);
        let mut variables = None;
        let mut buffered_bindings: Option<Vec<_>> = None;
        let mut output_iter = None;

        if reader.read_next_event()? != JsonEvent::StartObject {
            return Err(SyntaxError::msg("SPARQL JSON results should be an object").into());
        }

        loop {
            let event = reader.read_next_event()?;
            match event {
                JsonEvent::ObjectKey(key) => match key.as_ref() {
                    "head" => {
                        let extracted_variables = read_head(&mut reader)?;
                        if let Some(buffered_bindings) = buffered_bindings.take() {
                            let mut mapping = BTreeMap::default();
                            for (i, var) in extracted_variables.iter().enumerate() {
                                mapping.insert(var.as_str().to_owned(), i);
                            }
                            output_iter = Some(Self::Solutions {
                                variables: extracted_variables,
                                solutions: JsonSolutionsReader {
                                    kind: JsonSolutionsReaderKind::Buffered {
                                        bindings: buffered_bindings.into_iter(),
                                    },
                                    mapping,
                                },
                            });
                        } else {
                            variables = Some(extracted_variables);
                        }
                    }
                    "results" => {
                        if reader.read_next_event()? != JsonEvent::StartObject {
                            return Err(SyntaxError::msg("'results' should be an object").into());
                        }
                        loop {
                            match reader.read_next_event()? {
                                JsonEvent::ObjectKey(k) if k == "bindings" => break, // Found
                                JsonEvent::ObjectKey(_) => ignore_value(&mut reader)?,
                                _ => {
                                    return Err(SyntaxError::msg(
                                        "'results' should contain a 'bindings' key",
                                    )
                                    .into())
                                }
                            }
                        }
                        if reader.read_next_event()? != JsonEvent::StartArray {
                            return Err(SyntaxError::msg("'bindings' should be an object").into());
                        }
                        if let Some(variables) = variables {
                            let mut mapping = BTreeMap::default();
                            for (i, var) in variables.iter().enumerate() {
                                mapping.insert(var.as_str().to_owned(), i);
                            }
                            return Ok(Self::Solutions {
                                variables,
                                solutions: JsonSolutionsReader {
                                    kind: JsonSolutionsReaderKind::Streaming { reader },
                                    mapping,
                                },
                            });
                        }
                        // We buffer all results before being able to read the header
                        let mut bindings = Vec::new();
                        let mut variables = Vec::new();
                        let mut values = Vec::new();
                        loop {
                            match reader.read_next_event()? {
                                JsonEvent::StartObject => (),
                                JsonEvent::EndObject => {
                                    bindings.push((take(&mut variables), take(&mut values)));
                                }
                                JsonEvent::EndArray | JsonEvent::Eof => {
                                    buffered_bindings = Some(bindings);
                                    break;
                                }
                                JsonEvent::ObjectKey(key) => {
                                    variables.push(key.into_owned());
                                    values.push(read_value(&mut reader, 0)?);
                                }
                                _ => {
                                    return Err(
                                        SyntaxError::msg("Invalid result serialization").into()
                                    )
                                }
                            }
                        }
                    }
                    "boolean" => {
                        return if let JsonEvent::Boolean(v) = reader.read_next_event()? {
                            Ok(Self::Boolean(v))
                        } else {
                            Err(SyntaxError::msg("Unexpected boolean value").into())
                        }
                    }
                    _ => {
                        return Err(SyntaxError::msg(format!(
                            "Expecting head or result key, found {key}"
                        ))
                        .into());
                    }
                },
                JsonEvent::EndObject => (),
                JsonEvent::Eof => {
                    return if let Some(output_iter) = output_iter {
                        Ok(output_iter)
                    } else {
                        Err(SyntaxError::msg(
                            "Unexpected end of JSON object without 'results' or 'boolean' key",
                        )
                        .into())
                    }
                }
                _ => return Err(SyntaxError::msg("Invalid SPARQL results serialization").into()),
            }
        }
    }
}

pub struct JsonSolutionsReader<R: Read> {
    mapping: BTreeMap<String, usize>,
    kind: JsonSolutionsReaderKind<R>,
}

enum JsonSolutionsReaderKind<R: Read> {
    Streaming {
        reader: FromReadJsonReader<R>,
    },
    Buffered {
        bindings: std::vec::IntoIter<(Vec<String>, Vec<Term>)>,
    },
}

impl<R: Read> JsonSolutionsReader<R> {
    pub fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, ParseError> {
        match &mut self.kind {
            JsonSolutionsReaderKind::Streaming { reader } => {
                let mut new_bindings = vec![None; self.mapping.len()];
                loop {
                    match reader.read_next_event()? {
                        JsonEvent::StartObject => (),
                        JsonEvent::EndObject => return Ok(Some(new_bindings)),
                        JsonEvent::EndArray | JsonEvent::Eof => return Ok(None),
                        JsonEvent::ObjectKey(key) => {
                            let k = *self.mapping.get(key.as_ref()).ok_or_else(|| {
                                SyntaxError::msg(format!(
                                    "The variable {key} has not been defined in the header"
                                ))
                            })?;
                            new_bindings[k] = Some(read_value(reader, 0)?)
                        }
                        _ => return Err(SyntaxError::msg("Invalid result serialization").into()),
                    }
                }
            }
            JsonSolutionsReaderKind::Buffered { bindings } => {
                Ok(if let Some((variables, values)) = bindings.next() {
                    let mut new_bindings = vec![None; self.mapping.len()];
                    for (variable, value) in variables.into_iter().zip(values) {
                        let k = *self.mapping.get(&variable).ok_or_else(|| {
                            SyntaxError::msg(format!(
                                "The variable {variable} has not been defined in the header"
                            ))
                        })?;
                        new_bindings[k] = Some(value)
                    }
                    Some(new_bindings)
                } else {
                    None
                })
            }
        }
    }
}

fn read_value<R: Read>(
    reader: &mut FromReadJsonReader<R>,
    number_of_recursive_calls: usize,
) -> Result<Term, ParseError> {
    enum Type {
        Uri,
        BNode,
        Literal,
        #[cfg(feature = "rdf-star")]
        Triple,
    }
    #[derive(Eq, PartialEq)]
    enum State {
        Type,
        Value,
        Lang,
        Datatype,
    }

    if number_of_recursive_calls == MAX_NUMBER_OF_NESTED_TRIPLES {
        return Err(SyntaxError::msg(format!(
            "Too many nested triples ({MAX_NUMBER_OF_NESTED_TRIPLES}). The parser fails here to avoid a stack overflow."
        ))
            .into());
    }
    let mut state = None;
    let mut t = None;
    let mut value = None;
    let mut lang = None;
    let mut datatype = None;
    #[cfg(feature = "rdf-star")]
    let mut subject = None;
    #[cfg(feature = "rdf-star")]
    let mut predicate = None;
    #[cfg(feature = "rdf-star")]
    let mut object = None;
    if reader.read_next_event()? != JsonEvent::StartObject {
        return Err(SyntaxError::msg("Term serializations should be an object").into());
    }
    loop {
        #[allow(unsafe_code)]
        // SAFETY: Borrow checker workaround https://github.com/rust-lang/rust/issues/70255
        let next_event = unsafe {
            let r: *mut FromReadJsonReader<R> = reader;
            &mut *r
        }
        .read_next_event()?;
        match next_event {
            JsonEvent::ObjectKey(key) => match key.as_ref() {
                "type" => state = Some(State::Type),
                "value" => state = Some(State::Value),
                "xml:lang" => state = Some(State::Lang),
                "datatype" => state = Some(State::Datatype),
                #[cfg(feature = "rdf-star")]
                "subject" => subject = Some(read_value(reader, number_of_recursive_calls + 1)?),
                #[cfg(feature = "rdf-star")]
                "predicate" => predicate = Some(read_value(reader, number_of_recursive_calls + 1)?),
                #[cfg(feature = "rdf-star")]
                "object" => object = Some(read_value(reader, number_of_recursive_calls + 1)?),
                _ => {
                    return Err(SyntaxError::msg(format!(
                        "Unexpected key in term serialization: '{key}'"
                    ))
                    .into())
                }
            },
            JsonEvent::StartObject => {
                if state != Some(State::Value) {
                    return Err(
                        SyntaxError::msg("Unexpected nested object in term serialization").into(),
                    );
                }
            }
            JsonEvent::String(s) => match state {
                Some(State::Type) => {
                    match s.as_ref() {
                        "uri" => t = Some(Type::Uri),
                        "bnode" => t = Some(Type::BNode),
                        "literal" | "typed-literal" => t = Some(Type::Literal),
                        #[cfg(feature = "rdf-star")]
                        "triple" => t = Some(Type::Triple),
                        _ => {
                            return Err(
                                SyntaxError::msg(format!("Unexpected term type: '{s}'")).into()
                            )
                        }
                    };
                    state = None;
                }
                Some(State::Value) => {
                    value = Some(s.into_owned());
                    state = None;
                }
                Some(State::Lang) => {
                    lang = Some(s.into_owned());
                    state = None;
                }
                Some(State::Datatype) => {
                    datatype = Some(
                        NamedNode::new(s)
                            .map_err(|e| SyntaxError::msg(format!("Invalid datatype IRI: {e}")))?,
                    );
                    state = None;
                }
                _ => (), // impossible
            },
            JsonEvent::EndObject => {
                if let Some(s) = state {
                    if s == State::Value {
                        state = None; //End of triple
                    } else {
                        return Err(
                            SyntaxError::msg("Term description values should be string").into()
                        );
                    }
                } else {
                    return match t {
                        None => Err(SyntaxError::msg(
                            "Term serialization should have a 'type' key",
                        )
                        .into()),
                        Some(Type::Uri) => Ok(NamedNode::new(value.ok_or_else(|| {
                            SyntaxError::msg("uri serialization should have a 'value' key")
                        })?)
                        .map_err(|e| SyntaxError::msg(format!("Invalid uri value: {e}")))?
                        .into()),
                        Some(Type::BNode) => Ok(BlankNode::new(value.ok_or_else(|| {
                            SyntaxError::msg("bnode serialization should have a 'value' key")
                        })?)
                        .map_err(|e| SyntaxError::msg(format!("Invalid bnode value: {e}")))?
                        .into()),
                        Some(Type::Literal) => {
                            let value = value.ok_or_else(|| {
                                SyntaxError::msg("literal serialization should have a 'value' key")
                            })?;
                            Ok(match lang {
                                Some(lang) => {
                                    if let Some(datatype) = datatype {
                                        if datatype.as_ref() != rdf::LANG_STRING {
                                            return Err(SyntaxError::msg(format!(
                                                "xml:lang value '{lang}' provided with the datatype {datatype}"
                                            )).into())
                                        }
                                    }
                                    Literal::new_language_tagged_literal(value, &*lang).map_err(|e| {
                                        SyntaxError::msg(format!("Invalid xml:lang value '{lang}': {e}"))
                                    })?
                                }
                                None => if let Some(datatype) = datatype {
                                    Literal::new_typed_literal(value, datatype)
                                } else {
                                    Literal::new_simple_literal(value)
                                }
                            }
                                .into())
                        }
                        #[cfg(feature = "rdf-star")]
                        Some(Type::Triple) => Ok(Triple::new(
                            match subject.ok_or_else(|| {
                                SyntaxError::msg("triple serialization should have a 'subject' key")
                            })? {
                                Term::NamedNode(subject) => subject.into(),
                                Term::BlankNode(subject) => subject.into(),
                                Term::Triple(subject) => Subject::Triple(subject),
                                Term::Literal(_) => {
                                    return Err(SyntaxError::msg(
                                        "The 'subject' value should not be a literal",
                                    )
                                    .into())
                                }
                            },
                            match predicate.ok_or_else(|| {
                                SyntaxError::msg(
                                    "triple serialization should have a 'predicate' key",
                                )
                            })? {
                                Term::NamedNode(predicate) => predicate,
                                _ => {
                                    return Err(SyntaxError::msg(
                                        "The 'predicate' value should be a uri",
                                    )
                                    .into())
                                }
                            },
                            object.ok_or_else(|| {
                                SyntaxError::msg("triple serialization should have a 'object' key")
                            })?,
                        )
                        .into()),
                    };
                }
            }
            _ => return Err(SyntaxError::msg("Invalid term serialization").into()),
        }
    }
}

fn read_head<R: Read>(reader: &mut FromReadJsonReader<R>) -> Result<Vec<Variable>, ParseError> {
    if reader.read_next_event()? != JsonEvent::StartObject {
        return Err(SyntaxError::msg("head should be an object").into());
    }
    let mut variables = Vec::new();
    loop {
        match reader.read_next_event()? {
            JsonEvent::ObjectKey(key) => match key.as_ref() {
                "vars" => {
                    if reader.read_next_event()? != JsonEvent::StartArray {
                        return Err(SyntaxError::msg("Variable list should be an array").into());
                    }
                    loop {
                        match reader.read_next_event()? {
                            JsonEvent::String(s) => {
                                let new_var = Variable::new(s.as_ref()).map_err(|e| {
                                    SyntaxError::msg(format!(
                                        "Invalid variable declaration '{s}': {e}"
                                    ))
                                })?;
                                if variables.contains(&new_var) {
                                    return Err(SyntaxError::msg(format!(
                                        "The variable {new_var} is declared twice"
                                    ))
                                    .into());
                                }
                                variables.push(new_var);
                            }
                            JsonEvent::EndArray => break,
                            _ => {
                                return Err(
                                    SyntaxError::msg("Variable names should be strings").into()
                                )
                            }
                        }
                    }
                }
                "link" => {
                    if reader.read_next_event()? != JsonEvent::StartArray {
                        return Err(SyntaxError::msg("Variable list should be an array").into());
                    }
                    loop {
                        match reader.read_next_event()? {
                            JsonEvent::String(_) => (),
                            JsonEvent::EndArray => break,
                            _ => {
                                return Err(SyntaxError::msg("Link names should be strings").into())
                            }
                        }
                    }
                }
                _ => ignore_value(reader)?,
            },
            JsonEvent::EndObject => return Ok(variables),
            _ => return Err(SyntaxError::msg("Invalid head serialization").into()),
        }
    }
}

fn ignore_value<R: Read>(reader: &mut FromReadJsonReader<R>) -> Result<(), ParseError> {
    let mut nesting = 0;
    loop {
        match reader.read_next_event()? {
            JsonEvent::Boolean(_)
            | JsonEvent::Null
            | JsonEvent::Number(_)
            | JsonEvent::String(_) => {
                if nesting == 0 {
                    return Ok(());
                }
            }
            JsonEvent::ObjectKey(_) => (),
            JsonEvent::StartArray | JsonEvent::StartObject => nesting += 1,
            JsonEvent::EndArray | JsonEvent::EndObject => {
                nesting -= 1;
                if nesting == 0 {
                    return Ok(());
                }
            }
            JsonEvent::Eof => return Err(SyntaxError::msg("Unexpected end of file").into()),
        }
    }
}
