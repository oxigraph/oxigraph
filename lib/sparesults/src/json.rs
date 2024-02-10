//! Implementation of [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)

use crate::error::{QueryResultsParseError, QueryResultsSyntaxError};
use json_event_parser::{FromReadJsonReader, JsonEvent, ToWriteJsonWriter};
#[cfg(feature = "async-tokio")]
use json_event_parser::{FromTokioAsyncReadJsonReader, ToTokioAsyncWriteJsonWriter};
use oxrdf::vocab::rdf;
use oxrdf::*;
use std::collections::BTreeMap;
use std::io::{self, Read, Write};
use std::mem::take;
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncWrite};

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

pub enum FromReadJsonQueryResultsReader<R: Read> {
    Solutions {
        variables: Vec<Variable>,
        solutions: FromReadJsonSolutionsReader<R>,
    },
    Boolean(bool),
}

impl<R: Read> FromReadJsonQueryResultsReader<R> {
    pub fn read(read: R) -> Result<Self, QueryResultsParseError> {
        let mut reader = FromReadJsonReader::new(read);
        let mut inner = JsonInnerReader::new();
        loop {
            if let Some(result) = inner.read_event(reader.read_next_event()?)? {
                return match result {
                    JsonInnerQueryResults::Solutions {
                        variables,
                        solutions,
                    } => Ok(Self::Solutions {
                        variables,
                        solutions: FromReadJsonSolutionsReader {
                            inner: solutions,
                            reader,
                        },
                    }),
                    JsonInnerQueryResults::Boolean(value) => Ok(Self::Boolean(value)),
                };
            }
        }
    }
}

pub struct FromReadJsonSolutionsReader<R: Read> {
    inner: JsonInnerSolutions,
    reader: FromReadJsonReader<R>,
}

impl<R: Read> FromReadJsonSolutionsReader<R> {
    pub fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, QueryResultsParseError> {
        match &mut self.inner {
            JsonInnerSolutions::Reader(reader) => loop {
                let event = self.reader.read_next_event()?;
                if event == JsonEvent::Eof {
                    return Ok(None);
                }
                if let Some(result) = reader.read_event(event)? {
                    return Ok(Some(result));
                }
            },
            JsonInnerSolutions::Iterator(iter) => iter.next(),
        }
    }
}

#[cfg(feature = "async-tokio")]
pub enum FromTokioAsyncReadJsonQueryResultsReader<R: AsyncRead + Unpin> {
    Solutions {
        variables: Vec<Variable>,
        solutions: FromTokioAsyncReadJsonSolutionsReader<R>,
    },
    Boolean(bool),
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> FromTokioAsyncReadJsonQueryResultsReader<R> {
    pub async fn read(read: R) -> Result<Self, QueryResultsParseError> {
        let mut reader = FromTokioAsyncReadJsonReader::new(read);
        let mut inner = JsonInnerReader::new();
        loop {
            if let Some(result) = inner.read_event(reader.read_next_event().await?)? {
                return match result {
                    JsonInnerQueryResults::Solutions {
                        variables,
                        solutions,
                    } => Ok(Self::Solutions {
                        variables,
                        solutions: FromTokioAsyncReadJsonSolutionsReader {
                            inner: solutions,
                            reader,
                        },
                    }),
                    JsonInnerQueryResults::Boolean(value) => Ok(Self::Boolean(value)),
                };
            }
        }
    }
}

#[cfg(feature = "async-tokio")]
pub struct FromTokioAsyncReadJsonSolutionsReader<R: AsyncRead + Unpin> {
    inner: JsonInnerSolutions,
    reader: FromTokioAsyncReadJsonReader<R>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> FromTokioAsyncReadJsonSolutionsReader<R> {
    pub async fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, QueryResultsParseError> {
        match &mut self.inner {
            JsonInnerSolutions::Reader(reader) => loop {
                let event = self.reader.read_next_event().await?;
                if event == JsonEvent::Eof {
                    return Ok(None);
                }
                if let Some(result) = reader.read_event(event)? {
                    return Ok(Some(result));
                }
            },
            JsonInnerSolutions::Iterator(iter) => iter.next(),
        }
    }
}

enum JsonInnerQueryResults {
    Solutions {
        variables: Vec<Variable>,
        solutions: JsonInnerSolutions,
    },
    Boolean(bool),
}

enum JsonInnerSolutions {
    Reader(JsonInnerSolutionsReader),
    Iterator(JsonBufferedSolutionsIterator),
}

struct JsonInnerReader {
    state: JsonInnerReaderState,
    variables: Vec<Variable>,
    current_solution_variables: Vec<String>,
    current_solution_values: Vec<Term>,
    solutions: Vec<(Vec<String>, Vec<Term>)>,
    vars_read: bool,
    solutions_read: bool,
}

enum JsonInnerReaderState {
    Start,
    InRootObject,
    BeforeHead,
    InHead,
    BeforeVars,
    InVars,
    BeforeLinks,
    InLinks,
    BeforeResults,
    InResults,
    BeforeBindings,
    BeforeSolution,
    BetweenSolutionTerms,
    Term {
        reader: JsonInnerTermReader,
        variable: String,
    },
    AfterBindings,
    BeforeBoolean,
    Ignore {
        level: usize,
        after: JsonInnerReaderStateAfterIgnore,
    },
}

#[allow(clippy::enum_variant_names)]
#[derive(Clone, Copy)]
enum JsonInnerReaderStateAfterIgnore {
    InRootObject,
    InHead,
    InResults,
    AfterBindings,
}

impl JsonInnerReader {
    fn new() -> Self {
        Self {
            state: JsonInnerReaderState::Start,
            variables: Vec::new(),
            current_solution_variables: Vec::new(),
            current_solution_values: Vec::new(),
            solutions: Vec::new(),
            vars_read: false,
            solutions_read: false,
        }
    }

    fn read_event(
        &mut self,
        event: JsonEvent<'_>,
    ) -> Result<Option<JsonInnerQueryResults>, QueryResultsSyntaxError> {
        match &mut self.state {
            JsonInnerReaderState::Start => {
                if event == JsonEvent::StartObject {
                    self.state = JsonInnerReaderState::InRootObject;
                    Ok(None)
                } else {
                    Err(QueryResultsSyntaxError::msg(
                        "SPARQL JSON results must be an object",
                    ))
                }
            }
            JsonInnerReaderState::InRootObject => match event {
                JsonEvent::ObjectKey(key) => match key.as_ref() {
                    "head" => {
                        self.state = JsonInnerReaderState::BeforeHead;
                        Ok(None)
                    }
                    "results" => {
                        self.state = JsonInnerReaderState::BeforeResults;
                        Ok(None)
                    }
                    "boolean" => {
                        self.state = JsonInnerReaderState::BeforeBoolean;
                        Ok(None)
                    }
                    _ => {
                        self.state = JsonInnerReaderState::Ignore {
                            level: 0,
                            after: JsonInnerReaderStateAfterIgnore::InRootObject,
                        };
                        Ok(None)
                    }
                },
                JsonEvent::EndObject => Err(QueryResultsSyntaxError::msg(
                    "SPARQL JSON results must contain a 'boolean' or a 'results' key",
                )),
                _ => unreachable!(),
            },
            JsonInnerReaderState::BeforeHead => {
                if event == JsonEvent::StartObject {
                    self.state = JsonInnerReaderState::InHead;
                    Ok(None)
                } else {
                    Err(QueryResultsSyntaxError::msg(
                        "SPARQL JSON results head must be an object",
                    ))
                }
            }
            JsonInnerReaderState::InHead => match event {
                JsonEvent::ObjectKey(key) => match key.as_ref() {
                    "vars" => {
                        self.state = JsonInnerReaderState::BeforeVars;
                        self.vars_read = true;
                        Ok(None)
                    }
                    "links" => {
                        self.state = JsonInnerReaderState::BeforeLinks;
                        Ok(None)
                    }
                    _ => {
                        self.state = JsonInnerReaderState::Ignore {
                            level: 0,
                            after: JsonInnerReaderStateAfterIgnore::InHead,
                        };
                        Ok(None)
                    }
                },
                JsonEvent::EndObject => {
                    self.state = JsonInnerReaderState::InRootObject;
                    Ok(None)
                }
                _ => unreachable!(),
            },
            JsonInnerReaderState::BeforeVars => {
                if event == JsonEvent::StartArray {
                    self.state = JsonInnerReaderState::InVars;
                    Ok(None)
                } else {
                    Err(QueryResultsSyntaxError::msg(
                        "SPARQL JSON results vars must be an array",
                    ))
                }
            }
            JsonInnerReaderState::InVars => match event {
                JsonEvent::String(variable) => match Variable::new(variable.clone()) {
                    Ok(var) => {
                        if self.variables.contains(&var) {
                            return Err(QueryResultsSyntaxError::msg(format!(
                                "The variable {var} is declared twice"
                            )));
                        }
                        self.variables.push(var);
                        Ok(None)
                    }
                    Err(e) => Err(QueryResultsSyntaxError::msg(format!(
                        "Invalid variable name '{variable}': {e}"
                    ))),
                },
                JsonEvent::EndArray => {
                    if self.solutions_read {
                        let mut mapping = BTreeMap::default();
                        for (i, var) in self.variables.iter().enumerate() {
                            mapping.insert(var.as_str().to_owned(), i);
                        }
                        Ok(Some(JsonInnerQueryResults::Solutions {
                            variables: take(&mut self.variables),
                            solutions: JsonInnerSolutions::Iterator(
                                JsonBufferedSolutionsIterator {
                                    mapping,
                                    bindings: take(&mut self.solutions).into_iter(),
                                },
                            ),
                        }))
                    } else {
                        self.state = JsonInnerReaderState::InHead;
                        Ok(None)
                    }
                }
                _ => Err(QueryResultsSyntaxError::msg(
                    "Variables name in the vars array must be strings",
                )),
            },
            JsonInnerReaderState::BeforeLinks => {
                if event == JsonEvent::StartArray {
                    self.state = JsonInnerReaderState::InLinks;
                    Ok(None)
                } else {
                    Err(QueryResultsSyntaxError::msg(
                        "SPARQL JSON results links must be an array",
                    ))
                }
            }
            JsonInnerReaderState::InLinks => match event {
                JsonEvent::String(_) => Ok(None),
                JsonEvent::EndArray => {
                    self.state = JsonInnerReaderState::InHead;
                    Ok(None)
                }
                _ => Err(QueryResultsSyntaxError::msg(
                    "Links in the links array must be strings",
                )),
            },
            JsonInnerReaderState::BeforeResults => {
                if event == JsonEvent::StartObject {
                    self.state = JsonInnerReaderState::InResults;
                    Ok(None)
                } else {
                    Err(QueryResultsSyntaxError::msg(
                        "SPARQL JSON results result must be an object",
                    ))
                }
            }
            JsonInnerReaderState::InResults => match event {
                JsonEvent::ObjectKey(key) => {
                    if key == "bindings" {
                        self.state = JsonInnerReaderState::BeforeBindings;
                        Ok(None)
                    } else {
                        self.state = JsonInnerReaderState::Ignore {
                            level: 0,
                            after: JsonInnerReaderStateAfterIgnore::InResults,
                        };
                        Ok(None)
                    }
                }
                JsonEvent::EndObject => Err(QueryResultsSyntaxError::msg(
                    "The results object must contains a 'bindings' key",
                )),
                _ => unreachable!(),
            },
            JsonInnerReaderState::BeforeBindings => {
                if event == JsonEvent::StartArray {
                    self.solutions_read = true;
                    if self.vars_read {
                        let mut mapping = BTreeMap::default();
                        for (i, var) in self.variables.iter().enumerate() {
                            mapping.insert(var.as_str().to_owned(), i);
                        }
                        Ok(Some(JsonInnerQueryResults::Solutions {
                            variables: take(&mut self.variables),
                            solutions: JsonInnerSolutions::Reader(JsonInnerSolutionsReader {
                                state: JsonInnerSolutionsReaderState::BeforeSolution,
                                mapping,
                                new_bindings: Vec::new(),
                            }),
                        }))
                    } else {
                        self.state = JsonInnerReaderState::BeforeSolution;
                        Ok(None)
                    }
                } else {
                    Err(QueryResultsSyntaxError::msg(
                        "SPARQL JSON results bindings must be an array",
                    ))
                }
            }
            JsonInnerReaderState::BeforeSolution => match event {
                JsonEvent::StartObject => {
                    self.state = JsonInnerReaderState::BetweenSolutionTerms;
                    Ok(None)
                }
                JsonEvent::EndArray => {
                    self.state = JsonInnerReaderState::AfterBindings;
                    Ok(None)
                }
                _ => Err(QueryResultsSyntaxError::msg(
                    "Expecting a new solution object",
                )),
            },
            JsonInnerReaderState::BetweenSolutionTerms => match event {
                JsonEvent::ObjectKey(key) => {
                    self.state = JsonInnerReaderState::Term {
                        reader: JsonInnerTermReader::default(),
                        variable: key.into(),
                    };
                    Ok(None)
                }
                JsonEvent::EndObject => {
                    self.state = JsonInnerReaderState::BeforeSolution;
                    self.solutions.push((
                        take(&mut self.current_solution_variables),
                        take(&mut self.current_solution_values),
                    ));
                    Ok(None)
                }
                _ => unreachable!(),
            },
            JsonInnerReaderState::Term {
                ref mut reader,
                variable,
            } => {
                let result = reader.read_event(event);
                if let Some(term) = result? {
                    self.current_solution_variables.push(take(variable));
                    self.current_solution_values.push(term);
                    self.state = JsonInnerReaderState::BetweenSolutionTerms;
                }
                Ok(None)
            }
            JsonInnerReaderState::AfterBindings => {
                if event == JsonEvent::EndObject {
                    self.state = JsonInnerReaderState::InRootObject;
                } else {
                    self.state = JsonInnerReaderState::Ignore {
                        level: 0,
                        after: JsonInnerReaderStateAfterIgnore::AfterBindings,
                    }
                }
                Ok(None)
            }
            JsonInnerReaderState::BeforeBoolean => {
                if let JsonEvent::Boolean(v) = event {
                    Ok(Some(JsonInnerQueryResults::Boolean(v)))
                } else {
                    Err(QueryResultsSyntaxError::msg("Unexpected boolean value"))
                }
            }
            #[allow(clippy::ref_patterns)]
            JsonInnerReaderState::Ignore { level, ref after } => {
                let level = match event {
                    JsonEvent::StartArray | JsonEvent::StartObject => *level + 1,
                    JsonEvent::EndArray | JsonEvent::EndObject => *level - 1,
                    JsonEvent::String(_)
                    | JsonEvent::Number(_)
                    | JsonEvent::Boolean(_)
                    | JsonEvent::Null
                    | JsonEvent::ObjectKey(_)
                    | JsonEvent::Eof => *level,
                };
                self.state = if level == 0 {
                    match after {
                        JsonInnerReaderStateAfterIgnore::InRootObject => {
                            JsonInnerReaderState::InRootObject
                        }
                        JsonInnerReaderStateAfterIgnore::InHead => JsonInnerReaderState::InHead,
                        JsonInnerReaderStateAfterIgnore::InResults => {
                            JsonInnerReaderState::InResults
                        }
                        JsonInnerReaderStateAfterIgnore::AfterBindings => {
                            JsonInnerReaderState::AfterBindings
                        }
                    }
                } else {
                    JsonInnerReaderState::Ignore {
                        level,
                        after: *after,
                    }
                };
                Ok(None)
            }
        }
    }
}

struct JsonInnerSolutionsReader {
    state: JsonInnerSolutionsReaderState,
    mapping: BTreeMap<String, usize>,
    new_bindings: Vec<Option<Term>>,
}

enum JsonInnerSolutionsReaderState {
    BeforeSolution,
    BetweenSolutionTerms,
    Term {
        reader: JsonInnerTermReader,
        key: usize,
    },
    AfterEnd,
}

impl JsonInnerSolutionsReader {
    fn read_event(
        &mut self,
        event: JsonEvent<'_>,
    ) -> Result<Option<Vec<Option<Term>>>, QueryResultsSyntaxError> {
        match &mut self.state {
            JsonInnerSolutionsReaderState::BeforeSolution => match event {
                JsonEvent::StartObject => {
                    self.state = JsonInnerSolutionsReaderState::BetweenSolutionTerms;
                    self.new_bindings = vec![None; self.mapping.len()];
                    Ok(None)
                }
                JsonEvent::EndArray => {
                    self.state = JsonInnerSolutionsReaderState::AfterEnd;
                    Ok(None)
                }
                _ => Err(QueryResultsSyntaxError::msg(
                    "Expecting a new solution object",
                )),
            },
            JsonInnerSolutionsReaderState::BetweenSolutionTerms => match event {
                JsonEvent::ObjectKey(key) => {
                    let key = *self.mapping.get(key.as_ref()).ok_or_else(|| {
                        QueryResultsSyntaxError::msg(format!(
                            "The variable {key} has not been defined in the header"
                        ))
                    })?;
                    self.state = JsonInnerSolutionsReaderState::Term {
                        reader: JsonInnerTermReader::default(),
                        key,
                    };
                    Ok(None)
                }
                JsonEvent::EndObject => {
                    self.state = JsonInnerSolutionsReaderState::BeforeSolution;
                    Ok(Some(take(&mut self.new_bindings)))
                }
                _ => unreachable!(),
            },
            JsonInnerSolutionsReaderState::Term {
                ref mut reader,
                key,
            } => {
                let result = reader.read_event(event);
                if let Some(term) = result? {
                    self.new_bindings[*key] = Some(term);
                    self.state = JsonInnerSolutionsReaderState::BetweenSolutionTerms;
                }
                Ok(None)
            }
            JsonInnerSolutionsReaderState::AfterEnd => {
                if event == JsonEvent::EndObject {
                    Ok(None)
                } else {
                    Err(QueryResultsSyntaxError::msg(
                        "Unexpected JSON after the end of the bindings array",
                    ))
                }
            }
        }
    }
}

#[derive(Default)]
struct JsonInnerTermReader {
    state: JsonInnerTermReaderState,
    term_type: Option<TermType>,
    value: Option<String>,
    lang: Option<String>,
    datatype: Option<NamedNode>,
    #[cfg(feature = "rdf-star")]
    subject: Option<Term>,
    #[cfg(feature = "rdf-star")]
    predicate: Option<Term>,
    #[cfg(feature = "rdf-star")]
    object: Option<Term>,
}

#[derive(Default)]
enum JsonInnerTermReaderState {
    #[default]
    Start,
    Middle,
    TermType,
    Value,
    Lang,
    Datatype,
    #[cfg(feature = "rdf-star")]
    InValue,
    #[cfg(feature = "rdf-star")]
    Subject(Box<JsonInnerTermReader>),
    #[cfg(feature = "rdf-star")]
    Predicate(Box<JsonInnerTermReader>),
    #[cfg(feature = "rdf-star")]
    Object(Box<JsonInnerTermReader>),
}

enum TermType {
    Uri,
    BNode,
    Literal,
    #[cfg(feature = "rdf-star")]
    Triple,
}

impl JsonInnerTermReader {
    fn read_event(
        &mut self,
        event: JsonEvent<'_>,
    ) -> Result<Option<Term>, QueryResultsSyntaxError> {
        match &mut self.state {
            JsonInnerTermReaderState::Start => {
                if event == JsonEvent::StartObject {
                    self.state = JsonInnerTermReaderState::Middle;
                    Ok(None)
                } else {
                    Err(QueryResultsSyntaxError::msg(
                        "RDF terms must be encoded using objects",
                    ))
                }
            }
            JsonInnerTermReaderState::Middle => match event {
                JsonEvent::ObjectKey(object_key) => {
                    self.state = match object_key.as_ref() {
                        "type" => JsonInnerTermReaderState::TermType,
                        "value" => JsonInnerTermReaderState::Value,
                        "datatype" => JsonInnerTermReaderState::Datatype,
                        "xml:lang" => JsonInnerTermReaderState::Lang,
                        _ => {
                            return Err(QueryResultsSyntaxError::msg(format!(
                                "Unsupported term key: {object_key}"
                            )));
                        }
                    };
                    Ok(None)
                }
                JsonEvent::EndObject => {
                    self.state = JsonInnerTermReaderState::Start;
                    match self.term_type.take() {
                        None => Err(QueryResultsSyntaxError::msg(
                            "Term serialization should have a 'type' key",
                        )),
                        Some(TermType::Uri) => Ok(Some(
                            NamedNode::new(self.value.take().ok_or_else(|| {
                                QueryResultsSyntaxError::msg(
                                    "uri serialization should have a 'value' key",
                                )
                            })?)
                            .map_err(|e| {
                                QueryResultsSyntaxError::msg(format!("Invalid uri value: {e}"))
                            })?
                            .into(),
                        )),
                        Some(TermType::BNode) => Ok(Some(
                            BlankNode::new(self.value.take().ok_or_else(|| {
                                QueryResultsSyntaxError::msg(
                                    "bnode serialization should have a 'value' key",
                                )
                            })?)
                            .map_err(|e| {
                                QueryResultsSyntaxError::msg(format!("Invalid bnode value: {e}"))
                            })?
                            .into(),
                        )),
                        Some(TermType::Literal) => {
                            let value = self.value.take().ok_or_else(|| {
                                QueryResultsSyntaxError::msg(
                                    "literal serialization should have a 'value' key",
                                )
                            })?;
                            Ok(Some(match self.lang.take() {
                                    Some(lang) => {
                                        if let Some(datatype) = &self.datatype {
                                            if datatype.as_ref() != rdf::LANG_STRING {
                                                return Err(QueryResultsSyntaxError::msg(format!(
                                                    "xml:lang value '{lang}' provided with the datatype {datatype}"
                                                )));
                                            }
                                        }
                                        Literal::new_language_tagged_literal(value, &*lang)
                                            .map_err(|e| {
                                                QueryResultsSyntaxError::msg(format!(
                                                    "Invalid xml:lang value '{lang}': {e}"
                                                ))
                                            })?
                                    }
                                    None => {
                                        if let Some(datatype) = self.datatype.take() {
                                            Literal::new_typed_literal(value, datatype)
                                        } else {
                                            Literal::new_simple_literal(value)
                                        }
                                    }
                                }.into()))
                        }
                        #[cfg(feature = "rdf-star")]
                        Some(TermType::Triple) => Ok(Some(
                            Triple::new(
                                match self.subject.take().ok_or_else(|| {
                                    QueryResultsSyntaxError::msg(
                                        "triple serialization should have a 'subject' key",
                                    )
                                })? {
                                    Term::NamedNode(subject) => subject.into(),
                                    Term::BlankNode(subject) => subject.into(),
                                    Term::Triple(subject) => Subject::Triple(subject),
                                    Term::Literal(_) => {
                                        return Err(QueryResultsSyntaxError::msg(
                                            "The 'subject' value should not be a literal",
                                        ));
                                    }
                                },
                                match self.predicate.take().ok_or_else(|| {
                                    QueryResultsSyntaxError::msg(
                                        "triple serialization should have a 'predicate' key",
                                    )
                                })? {
                                    Term::NamedNode(predicate) => predicate,
                                    _ => {
                                        return Err(QueryResultsSyntaxError::msg(
                                            "The 'predicate' value should be a uri",
                                        ));
                                    }
                                },
                                self.object.take().ok_or_else(|| {
                                    QueryResultsSyntaxError::msg(
                                        "triple serialization should have a 'object' key",
                                    )
                                })?,
                            )
                            .into(),
                        )),
                    }
                }
                _ => unreachable!(),
            },
            JsonInnerTermReaderState::TermType => {
                self.state = JsonInnerTermReaderState::Middle;
                if let JsonEvent::String(value) = event {
                    match value.as_ref() {
                        "uri" => {
                            self.term_type = Some(TermType::Uri);
                            Ok(None)
                        }
                        "bnode" => {
                            self.term_type = Some(TermType::BNode);
                            Ok(None)
                        }
                        "literal" | "typed-literal" => {
                            self.term_type = Some(TermType::Literal);
                            Ok(None)
                        }
                        #[cfg(feature = "rdf-star")]
                        "triple" => {
                            self.term_type = Some(TermType::Triple);
                            Ok(None)
                        }
                        _ => Err(QueryResultsSyntaxError::msg(format!(
                            "Unexpected term type: '{value}'"
                        ))),
                    }
                } else {
                    Err(QueryResultsSyntaxError::msg("Term type must be a string"))
                }
            }
            JsonInnerTermReaderState::Value => match event {
                JsonEvent::String(value) => {
                    self.value = Some(value.into_owned());
                    self.state = JsonInnerTermReaderState::Middle;
                    Ok(None)
                }
                #[cfg(feature = "rdf-star")]
                JsonEvent::StartObject => {
                    self.state = JsonInnerTermReaderState::InValue;
                    Ok(None)
                }
                _ => {
                    self.state = JsonInnerTermReaderState::Middle;

                    Err(QueryResultsSyntaxError::msg("Term value must be a string"))
                }
            },
            JsonInnerTermReaderState::Lang => {
                let result = if let JsonEvent::String(value) = event {
                    self.lang = Some(value.into_owned());
                    Ok(None)
                } else {
                    Err(QueryResultsSyntaxError::msg("Term lang must be strings"))
                };
                self.state = JsonInnerTermReaderState::Middle;

                result
            }
            JsonInnerTermReaderState::Datatype => {
                let result = if let JsonEvent::String(value) = event {
                    match NamedNode::new(value) {
                        Ok(datatype) => {
                            self.datatype = Some(datatype);
                            Ok(None)
                        }
                        Err(e) => Err(QueryResultsSyntaxError::msg(format!(
                            "Invalid datatype: {e}"
                        ))),
                    }
                } else {
                    Err(QueryResultsSyntaxError::msg("Term lang must be strings"))
                };
                self.state = JsonInnerTermReaderState::Middle;

                result
            }
            #[cfg(feature = "rdf-star")]
            JsonInnerTermReaderState::InValue => match event {
                JsonEvent::ObjectKey(object_key) => {
                    self.state = match object_key.as_ref() {
                        "subject" => JsonInnerTermReaderState::Subject(Box::default()),
                        "predicate" => JsonInnerTermReaderState::Predicate(Box::default()),
                        "object" => JsonInnerTermReaderState::Object(Box::default()),
                        _ => {
                            return Err(QueryResultsSyntaxError::msg(format!(
                                "Unsupported value key: {object_key}"
                            )));
                        }
                    };
                    Ok(None)
                }
                JsonEvent::EndObject => {
                    self.state = JsonInnerTermReaderState::Middle;
                    Ok(None)
                }
                _ => unreachable!(),
            },
            #[cfg(feature = "rdf-star")]
            JsonInnerTermReaderState::Subject(ref mut inner_state) => {
                if let Some(term) = inner_state.read_event(event)? {
                    self.state = JsonInnerTermReaderState::InValue;
                    self.subject = Some(term);
                }
                Ok(None)
            }
            #[cfg(feature = "rdf-star")]
            JsonInnerTermReaderState::Predicate(ref mut inner_state) => {
                if let Some(term) = inner_state.read_event(event)? {
                    self.state = JsonInnerTermReaderState::InValue;
                    self.predicate = Some(term);
                }
                Ok(None)
            }
            #[cfg(feature = "rdf-star")]
            JsonInnerTermReaderState::Object(ref mut inner_state) => {
                if let Some(term) = inner_state.read_event(event)? {
                    self.state = JsonInnerTermReaderState::InValue;
                    self.object = Some(term);
                }
                Ok(None)
            }
        }
    }
}

pub struct JsonBufferedSolutionsIterator {
    mapping: BTreeMap<String, usize>,
    bindings: std::vec::IntoIter<(Vec<String>, Vec<Term>)>,
}

impl JsonBufferedSolutionsIterator {
    fn next(&mut self) -> Result<Option<Vec<Option<Term>>>, QueryResultsParseError> {
        let Some((variables, values)) = self.bindings.next() else {
            return Ok(None);
        };
        let mut new_bindings = vec![None; self.mapping.len()];
        for (variable, value) in variables.into_iter().zip(values) {
            let k = *self.mapping.get(&variable).ok_or_else(|| {
                QueryResultsSyntaxError::msg(format!(
                    "The variable {variable} has not been defined in the header"
                ))
            })?;
            new_bindings[k] = Some(value);
        }
        Ok(Some(new_bindings))
    }
}
