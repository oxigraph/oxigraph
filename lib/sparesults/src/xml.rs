//! Implementation of [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/)

use crate::error::{QueryResultsParseError, QueryResultsSyntaxError};
use oxrdf::vocab::rdf;
use oxrdf::*;
use quick_xml::escape::unescape;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Decoder, Reader, Writer};
use std::collections::BTreeMap;
use std::io::{self, BufReader, Read, Write};
use std::mem::take;
use std::sync::Arc;
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncWrite, BufReader as AsyncBufReader};

pub fn write_boolean_xml_result<W: Write>(write: W, value: bool) -> io::Result<W> {
    let mut writer = Writer::new(write);
    for event in inner_write_boolean_xml_result(value) {
        writer.write_event(event).map_err(map_xml_error)?;
    }
    Ok(writer.into_inner())
}

#[cfg(feature = "async-tokio")]
pub async fn tokio_async_write_boolean_xml_result<W: AsyncWrite + Unpin>(
    write: W,
    value: bool,
) -> io::Result<W> {
    let mut writer = Writer::new(write);
    for event in inner_write_boolean_xml_result(value) {
        writer
            .write_event_async(event)
            .await
            .map_err(map_xml_error)?;
    }
    Ok(writer.into_inner())
}

fn inner_write_boolean_xml_result(value: bool) -> [Event<'static>; 8] {
    [
        Event::Decl(BytesDecl::new("1.0", None, None)),
        Event::Start(
            BytesStart::new("sparql")
                .with_attributes([("xmlns", "http://www.w3.org/2005/sparql-results#")]),
        ),
        Event::Start(BytesStart::new("head")),
        Event::End(BytesEnd::new("head")),
        Event::Start(BytesStart::new("boolean")),
        Event::Text(BytesText::new(if value { "true" } else { "false" })),
        Event::End(BytesEnd::new("boolean")),
        Event::End(BytesEnd::new("sparql")),
    ]
}

pub struct ToWriteXmlSolutionsWriter<W: Write> {
    inner: InnerXmlSolutionsWriter,
    writer: Writer<W>,
}

impl<W: Write> ToWriteXmlSolutionsWriter<W> {
    pub fn start(write: W, variables: &[Variable]) -> io::Result<Self> {
        let mut writer = Writer::new(write);
        let mut buffer = Vec::with_capacity(48);
        let inner = InnerXmlSolutionsWriter::start(&mut buffer, variables);
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
        Ok(self.writer.into_inner())
    }

    fn do_write(writer: &mut Writer<W>, output: Vec<Event<'_>>) -> io::Result<()> {
        for event in output {
            writer.write_event(event).map_err(map_xml_error)?;
        }
        Ok(())
    }
}

#[cfg(feature = "async-tokio")]
pub struct ToTokioAsyncWriteXmlSolutionsWriter<W: AsyncWrite + Unpin> {
    inner: InnerXmlSolutionsWriter,
    writer: Writer<W>,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> ToTokioAsyncWriteXmlSolutionsWriter<W> {
    pub async fn start(write: W, variables: &[Variable]) -> io::Result<Self> {
        let mut writer = Writer::new(write);
        let mut buffer = Vec::with_capacity(48);
        let inner = InnerXmlSolutionsWriter::start(&mut buffer, variables);
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
        Ok(self.writer.into_inner())
    }

    async fn do_write(writer: &mut Writer<W>, output: Vec<Event<'_>>) -> io::Result<()> {
        for event in output {
            writer
                .write_event_async(event)
                .await
                .map_err(map_xml_error)?;
        }
        Ok(())
    }
}

struct InnerXmlSolutionsWriter;

impl InnerXmlSolutionsWriter {
    fn start<'a>(output: &mut Vec<Event<'a>>, variables: &'a [Variable]) -> Self {
        output.push(Event::Decl(BytesDecl::new("1.0", None, None)));
        output.push(Event::Start(BytesStart::new("sparql").with_attributes([(
            "xmlns",
            "http://www.w3.org/2005/sparql-results#",
        )])));
        output.push(Event::Start(BytesStart::new("head")));
        for variable in variables {
            output.push(Event::Empty(
                BytesStart::new("variable").with_attributes([("name", variable.as_str())]),
            ));
        }
        output.push(Event::End(BytesEnd::new("head")));
        output.push(Event::Start(BytesStart::new("results")));
        Self {}
    }

    #[allow(clippy::unused_self)]
    fn write<'a>(
        &self,
        output: &mut Vec<Event<'a>>,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) {
        output.push(Event::Start(BytesStart::new("result")));
        for (variable, value) in solution {
            output.push(Event::Start(
                BytesStart::new("binding").with_attributes([("name", variable.as_str())]),
            ));
            write_xml_term(output, value);
            output.push(Event::End(BytesEnd::new("binding")));
        }
        output.push(Event::End(BytesEnd::new("result")));
    }

    #[allow(clippy::unused_self)]
    fn finish(self, output: &mut Vec<Event<'_>>) {
        output.push(Event::End(BytesEnd::new("results")));
        output.push(Event::End(BytesEnd::new("sparql")));
    }
}

fn write_xml_term<'a>(output: &mut Vec<Event<'a>>, term: TermRef<'a>) {
    match term {
        TermRef::NamedNode(uri) => {
            output.push(Event::Start(BytesStart::new("uri")));
            output.push(Event::Text(BytesText::new(uri.as_str())));
            output.push(Event::End(BytesEnd::new("uri")));
        }
        TermRef::BlankNode(bnode) => {
            output.push(Event::Start(BytesStart::new("bnode")));
            output.push(Event::Text(BytesText::new(bnode.as_str())));
            output.push(Event::End(BytesEnd::new("bnode")));
        }
        TermRef::Literal(literal) => {
            let mut start = BytesStart::new("literal");
            if let Some(language) = literal.language() {
                start.push_attribute(("xml:lang", language));
            } else if !literal.is_plain() {
                start.push_attribute(("datatype", literal.datatype().as_str()))
            }
            output.push(Event::Start(start));
            output.push(Event::Text(BytesText::new(literal.value())));
            output.push(Event::End(BytesEnd::new("literal")));
        }
        #[cfg(feature = "rdf-star")]
        TermRef::Triple(triple) => {
            output.push(Event::Start(BytesStart::new("triple")));
            output.push(Event::Start(BytesStart::new("subject")));
            write_xml_term(output, triple.subject.as_ref().into());
            output.push(Event::End(BytesEnd::new("subject")));
            output.push(Event::Start(BytesStart::new("predicate")));
            write_xml_term(output, triple.predicate.as_ref().into());
            output.push(Event::End(BytesEnd::new("predicate")));
            output.push(Event::Start(BytesStart::new("object")));
            write_xml_term(output, triple.object.as_ref());
            output.push(Event::End(BytesEnd::new("object")));
            output.push(Event::End(BytesEnd::new("triple")));
        }
    }
}

pub enum FromReadXmlQueryResultsReader<R: Read> {
    Solutions {
        variables: Vec<Variable>,
        solutions: FromReadXmlSolutionsReader<R>,
    },
    Boolean(bool),
}

impl<R: Read> FromReadXmlQueryResultsReader<R> {
    pub fn read(read: R) -> Result<Self, QueryResultsParseError> {
        let mut reader = Reader::from_reader(BufReader::new(read));
        reader.trim_text(true);
        reader.expand_empty_elements(true);
        let mut reader_buffer = Vec::new();
        let mut inner = XmlInnerQueryResultsReader {
            state: ResultsState::Start,
            variables: Vec::new(),
            decoder: reader.decoder(),
        };
        loop {
            reader_buffer.clear();
            let event = reader.read_event_into(&mut reader_buffer)?;
            if let Some(result) = inner.read_event(event)? {
                return Ok(match result {
                    XmlInnerQueryResults::Solutions {
                        variables,
                        solutions,
                    } => Self::Solutions {
                        variables,
                        solutions: FromReadXmlSolutionsReader {
                            reader,
                            inner: solutions,
                            reader_buffer,
                        },
                    },
                    XmlInnerQueryResults::Boolean(value) => Self::Boolean(value),
                });
            }
        }
    }
}

pub struct FromReadXmlSolutionsReader<R: Read> {
    reader: Reader<BufReader<R>>,
    inner: XmlInnerSolutionsReader,
    reader_buffer: Vec<u8>,
}

impl<R: Read> FromReadXmlSolutionsReader<R> {
    pub fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, QueryResultsParseError> {
        loop {
            self.reader_buffer.clear();
            let event = self.reader.read_event_into(&mut self.reader_buffer)?;
            if event == Event::Eof {
                return Ok(None);
            }
            if let Some(solution) = self.inner.read_event(event)? {
                return Ok(Some(solution));
            }
        }
    }
}

#[cfg(feature = "async-tokio")]
pub enum FromTokioAsyncReadXmlQueryResultsReader<R: AsyncRead + Unpin> {
    Solutions {
        variables: Vec<Variable>,
        solutions: FromTokioAsyncReadXmlSolutionsReader<R>,
    },
    Boolean(bool),
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> FromTokioAsyncReadXmlQueryResultsReader<R> {
    pub async fn read(read: R) -> Result<Self, QueryResultsParseError> {
        let mut reader = Reader::from_reader(AsyncBufReader::new(read));
        reader.trim_text(true);
        reader.expand_empty_elements(true);
        let mut reader_buffer = Vec::new();
        let mut inner = XmlInnerQueryResultsReader {
            state: ResultsState::Start,
            variables: Vec::new(),
            decoder: reader.decoder(),
        };
        loop {
            reader_buffer.clear();
            let event = reader.read_event_into_async(&mut reader_buffer).await?;
            if let Some(result) = inner.read_event(event)? {
                return Ok(match result {
                    XmlInnerQueryResults::Solutions {
                        variables,
                        solutions,
                    } => Self::Solutions {
                        variables,
                        solutions: FromTokioAsyncReadXmlSolutionsReader {
                            reader,
                            inner: solutions,
                            reader_buffer,
                        },
                    },
                    XmlInnerQueryResults::Boolean(value) => Self::Boolean(value),
                });
            }
        }
    }
}

#[cfg(feature = "async-tokio")]
pub struct FromTokioAsyncReadXmlSolutionsReader<R: AsyncRead + Unpin> {
    reader: Reader<AsyncBufReader<R>>,
    inner: XmlInnerSolutionsReader,
    reader_buffer: Vec<u8>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> FromTokioAsyncReadXmlSolutionsReader<R> {
    pub async fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, QueryResultsParseError> {
        loop {
            self.reader_buffer.clear();
            let event = self
                .reader
                .read_event_into_async(&mut self.reader_buffer)
                .await?;
            if event == Event::Eof {
                return Ok(None);
            }
            if let Some(solution) = self.inner.read_event(event)? {
                return Ok(Some(solution));
            }
        }
    }
}

enum XmlInnerQueryResults {
    Solutions {
        variables: Vec<Variable>,
        solutions: XmlInnerSolutionsReader,
    },
    Boolean(bool),
}

#[derive(Clone, Copy)]
enum ResultsState {
    Start,
    Sparql,
    Head,
    AfterHead,
    Boolean,
}

struct XmlInnerQueryResultsReader {
    state: ResultsState,
    variables: Vec<Variable>,
    decoder: Decoder,
}

impl XmlInnerQueryResultsReader {
    pub fn read_event(
        &mut self,
        event: Event<'_>,
    ) -> Result<Option<XmlInnerQueryResults>, QueryResultsParseError> {
        match event {
            Event::Start(event) => match self.state {
                ResultsState::Start => {
                    if event.local_name().as_ref() == b"sparql" {
                        self.state = ResultsState::Sparql;
                        Ok(None)
                    } else {
                        Err(QueryResultsSyntaxError::msg(format!("Expecting <sparql> tag, found <{}>", self.decoder.decode(event.name().as_ref())?)).into())
                    }
                }
                ResultsState::Sparql => {
                    if event.local_name().as_ref() == b"head" {
                        self.state = ResultsState::Head;
                        Ok(None)
                    } else {
                        Err(QueryResultsSyntaxError::msg(format!("Expecting <head> tag, found <{}>", self.decoder.decode(event.name().as_ref())?)).into())
                    }
                }
                ResultsState::Head => {
                    if event.local_name().as_ref() == b"variable" {
                        let name = event.attributes()
                            .filter_map(Result::ok)
                            .find(|attr| attr.key.local_name().as_ref() == b"name")
                            .ok_or_else(|| QueryResultsSyntaxError::msg("No name attribute found for the <variable> tag"))?;
                        let name = unescape(&self.decoder.decode(&name.value)?)?.into_owned();
                        let variable = Variable::new(name).map_err(|e| QueryResultsSyntaxError::msg(format!("Invalid variable name: {e}")))?;
                        if self.variables.contains(&variable) {
                            return Err(QueryResultsSyntaxError::msg(format!(
                                "The variable {variable} is declared twice"
                            ))
                                .into());
                        }
                        self.variables.push(variable);
                        Ok(None)
                    } else if event.local_name().as_ref() == b"link" {
                        // no op
                        Ok(None)
                    } else {
                        Err(QueryResultsSyntaxError::msg(format!("Expecting <variable> or <link> tag, found <{}>", self.decoder.decode(event.name().as_ref())?)).into())
                    }
                }
                ResultsState::AfterHead => {
                    if event.local_name().as_ref() == b"boolean" {
                        self.state = ResultsState::Boolean;
                        Ok(None)
                    } else if event.local_name().as_ref() == b"results" {
                        let mut mapping = BTreeMap::default();
                        for (i, var) in self.variables.iter().enumerate() {
                            mapping.insert(var.clone().into_string(), i);
                        }
                        Ok(Some(XmlInnerQueryResults::Solutions {
                            variables: take(&mut self.variables),
                            solutions: XmlInnerSolutionsReader {
                                decoder: self.decoder,
                                mapping,
                                state_stack: vec![State::Start, State::Start],
                                new_bindings: Vec::new(),
                                current_var: None,
                                term: None,
                                lang: None,
                                datatype: None,
                                subject_stack: Vec::new(),
                                predicate_stack: Vec::new(),
                                object_stack: Vec::new(),
                            },
                        }))
                    } else if event.local_name().as_ref() != b"link" && event.local_name().as_ref() != b"results" && event.local_name().as_ref() != b"boolean" {
                        Err(QueryResultsSyntaxError::msg(format!("Expecting sparql tag, found <{}>", self.decoder.decode(event.name().as_ref())?)).into())
                    } else {
                        Ok(None)
                    }
                }
                ResultsState::Boolean => Err(QueryResultsSyntaxError::msg(format!("Unexpected tag inside of <boolean> tag: <{}>", self.decoder.decode(event.name().as_ref())?)).into())
            },
            Event::Text(event) => {
                let value = event.unescape()?;
                match self.state {
                    ResultsState::Boolean => {
                        if value == "true" {
                            Ok(Some(XmlInnerQueryResults::Boolean(true)))
                        } else if value == "false" {
                            Ok(Some(XmlInnerQueryResults::Boolean(false)))
                        } else {
                            Err(QueryResultsSyntaxError::msg(format!("Unexpected boolean value. Found '{value}'")).into())
                        }
                    }
                    _ => Err(QueryResultsSyntaxError::msg(format!("Unexpected textual value found: '{value}'")).into())
                }
            }
            Event::End(event) => {
                if let ResultsState::Head = self.state {
                    if event.local_name().as_ref() == b"head" {
                        self.state = ResultsState::AfterHead
                    }
                    Ok(None)
                } else {
                    Err(QueryResultsSyntaxError::msg("Unexpected early file end. All results file should have a <head> and a <result> or <boolean> tag").into())
                }
            }
            Event::Eof => Err(QueryResultsSyntaxError::msg("Unexpected early file end. All results file should have a <head> and a <result> or <boolean> tag").into()),
            Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {
                Ok(None)
            }
            Event::Empty(_) => unreachable!("Empty events are expended"),
            Event::CData(_) => {
                Err(QueryResultsSyntaxError::msg(
                    "<![CDATA[...]]> are not supported in SPARQL XML results",
                )
                    .into())
            }
        }
    }
}

enum State {
    Start,
    Result,
    Binding,
    Uri,
    BNode,
    Literal,
    Triple,
    Subject,
    Predicate,
    Object,
}

struct XmlInnerSolutionsReader {
    decoder: Decoder,
    mapping: BTreeMap<String, usize>,
    state_stack: Vec<State>,
    new_bindings: Vec<Option<Term>>,
    current_var: Option<String>,
    term: Option<Term>,
    lang: Option<String>,
    datatype: Option<NamedNode>,
    subject_stack: Vec<Term>,
    predicate_stack: Vec<Term>,
    object_stack: Vec<Term>,
}

impl XmlInnerSolutionsReader {
    #[allow(clippy::unwrap_in_result)]
    pub fn read_event(
        &mut self,
        event: Event<'_>,
    ) -> Result<Option<Vec<Option<Term>>>, QueryResultsParseError> {
        match event {
            Event::Start(event) => match self.state_stack.last().unwrap() {
                State::Start => {
                    if event.local_name().as_ref() == b"result" {
                        self.new_bindings = vec![None; self.mapping.len()];
                        self.state_stack.push(State::Result);
                        Ok(None)
                    } else {
                        Err(QueryResultsSyntaxError::msg(format!(
                            "Expecting <result>, found <{}>",
                            self.decoder.decode(event.name().as_ref())?
                        ))
                        .into())
                    }
                }
                State::Result => {
                    if event.local_name().as_ref() == b"binding" {
                        let Some(attr) = event
                            .attributes()
                            .filter_map(Result::ok)
                            .find(|attr| attr.key.local_name().as_ref() == b"name")
                        else {
                            return Err(QueryResultsSyntaxError::msg(
                                "No name attribute found for the <binding> tag",
                            )
                            .into());
                        };
                        self.current_var =
                            Some(unescape(&self.decoder.decode(&attr.value)?)?.into_owned());
                        self.state_stack.push(State::Binding);
                        Ok(None)
                    } else {
                        Err(QueryResultsSyntaxError::msg(format!(
                            "Expecting <binding>, found <{}>",
                            self.decoder.decode(event.name().as_ref())?
                        ))
                        .into())
                    }
                }
                State::Binding | State::Subject | State::Predicate | State::Object => {
                    if self.term.is_some() {
                        return Err(QueryResultsSyntaxError::msg(
                            "There is already a value for the current binding",
                        )
                        .into());
                    }
                    if event.local_name().as_ref() == b"uri" {
                        self.state_stack.push(State::Uri);
                        Ok(None)
                    } else if event.local_name().as_ref() == b"bnode" {
                        self.state_stack.push(State::BNode);
                        Ok(None)
                    } else if event.local_name().as_ref() == b"literal" {
                        for attr in event.attributes() {
                            let attr = attr.map_err(quick_xml::Error::from)?;
                            if attr.key.as_ref() == b"xml:lang" {
                                self.lang = Some(
                                    unescape(&self.decoder.decode(&attr.value)?)?.into_owned(),
                                );
                            } else if attr.key.local_name().as_ref() == b"datatype" {
                                let iri = self.decoder.decode(&attr.value)?;
                                let iri = unescape(&iri)?;
                                self.datatype =
                                    Some(NamedNode::new(iri.as_ref()).map_err(|e| {
                                        QueryResultsSyntaxError::msg(format!(
                                            "Invalid datatype IRI '{iri}': {e}"
                                        ))
                                    })?);
                            }
                        }
                        self.state_stack.push(State::Literal);
                        Ok(None)
                    } else if event.local_name().as_ref() == b"triple" {
                        self.state_stack.push(State::Triple);
                        Ok(None)
                    } else {
                        Err(QueryResultsSyntaxError::msg(format!(
                            "Expecting <uri>, <bnode> or <literal> found <{}>",
                            self.decoder.decode(event.name().as_ref())?
                        ))
                        .into())
                    }
                }
                State::Triple => {
                    if event.local_name().as_ref() == b"subject" {
                        self.state_stack.push(State::Subject);
                        Ok(None)
                    } else if event.local_name().as_ref() == b"predicate" {
                        self.state_stack.push(State::Predicate);
                        Ok(None)
                    } else if event.local_name().as_ref() == b"object" {
                        self.state_stack.push(State::Object);
                        Ok(None)
                    } else {
                        Err(QueryResultsSyntaxError::msg(format!(
                            "Expecting <subject>, <predicate> or <object> found <{}>",
                            self.decoder.decode(event.name().as_ref())?
                        ))
                        .into())
                    }
                }
                State::Uri => Err(QueryResultsSyntaxError::msg(format!(
                    "<uri> must only contain a string, found <{}>",
                    self.decoder.decode(event.name().as_ref())?
                ))
                .into()),
                State::BNode => Err(QueryResultsSyntaxError::msg(format!(
                    "<uri> must only contain a string, found <{}>",
                    self.decoder.decode(event.name().as_ref())?
                ))
                .into()),
                State::Literal => Err(QueryResultsSyntaxError::msg(format!(
                    "<uri> must only contain a string, found <{}>",
                    self.decoder.decode(event.name().as_ref())?
                ))
                .into()),
            },
            Event::Text(event) => {
                let data = event.unescape()?;
                match self.state_stack.last().unwrap() {
                    State::Uri => {
                        self.term = Some(
                            NamedNode::new(data.to_string())
                                .map_err(|e| {
                                    QueryResultsSyntaxError::msg(format!(
                                        "Invalid IRI value '{data}': {e}"
                                    ))
                                })?
                                .into(),
                        );
                        Ok(None)
                    }
                    State::BNode => {
                        self.term = Some(
                            BlankNode::new(data.to_string())
                                .map_err(|e| {
                                    QueryResultsSyntaxError::msg(format!(
                                        "Invalid blank node value '{data}': {e}"
                                    ))
                                })?
                                .into(),
                        );
                        Ok(None)
                    }
                    State::Literal => {
                        self.term = Some(
                            build_literal(data, self.lang.take(), self.datatype.take())?.into(),
                        );
                        Ok(None)
                    }
                    _ => Err(QueryResultsSyntaxError::msg(format!(
                        "Unexpected textual value found: {data}"
                    ))
                    .into()),
                }
            }
            Event::End(_) => match self.state_stack.pop().unwrap() {
                State::Start | State::Uri => Ok(None),
                State::Result => Ok(Some(take(&mut self.new_bindings))),
                State::Binding => {
                    if let Some(var) = &self.current_var {
                        if let Some(var) = self.mapping.get(var) {
                            self.new_bindings[*var] = self.term.take()
                        } else {
                            return Err(
                                QueryResultsSyntaxError::msg(format!("The variable '{var}' is used in a binding but not declared in the variables list")).into()
                            );
                        }
                    } else {
                        return Err(QueryResultsSyntaxError::msg(
                            "No name found for <binding> tag",
                        )
                        .into());
                    }
                    Ok(None)
                }
                State::Subject => {
                    if let Some(subject) = self.term.take() {
                        self.subject_stack.push(subject)
                    }
                    Ok(None)
                }
                State::Predicate => {
                    if let Some(predicate) = self.term.take() {
                        self.predicate_stack.push(predicate)
                    }
                    Ok(None)
                }
                State::Object => {
                    if let Some(object) = self.term.take() {
                        self.object_stack.push(object)
                    }
                    Ok(None)
                }
                State::BNode => {
                    if self.term.is_none() {
                        // We default to a random bnode
                        self.term = Some(BlankNode::default().into())
                    }
                    Ok(None)
                }
                State::Literal => {
                    if self.term.is_none() {
                        // We default to the empty literal
                        self.term =
                            Some(build_literal("", self.lang.take(), self.datatype.take())?.into())
                    }
                    Ok(None)
                }
                State::Triple => {
                    #[cfg(feature = "rdf-star")]
                    if let (Some(subject), Some(predicate), Some(object)) = (
                        self.subject_stack.pop(),
                        self.predicate_stack.pop(),
                        self.object_stack.pop(),
                    ) {
                        self.term = Some(
                            Triple::new(
                                match subject {
                                    Term::NamedNode(subject) => subject.into(),
                                    Term::BlankNode(subject) => subject.into(),
                                    Term::Triple(subject) => Subject::Triple(subject),
                                    Term::Literal(_) => {
                                        return Err(QueryResultsSyntaxError::msg(
                                            "The <subject> value should not be a <literal>",
                                        )
                                        .into());
                                    }
                                },
                                match predicate {
                                    Term::NamedNode(predicate) => predicate,
                                    _ => {
                                        return Err(QueryResultsSyntaxError::msg(
                                            "The <predicate> value should be an <uri>",
                                        )
                                        .into());
                                    }
                                },
                                object,
                            )
                            .into(),
                        );
                        Ok(None)
                    } else {
                        Err(QueryResultsSyntaxError::msg(
                            "A <triple> should contain a <subject>, a <predicate> and an <object>",
                        )
                        .into())
                    }
                    #[cfg(not(feature = "rdf-star"))]
                    {
                        Err(QueryResultsSyntaxError::msg(
                            "The <triple> tag is only supported with RDF-star",
                        )
                        .into())
                    }
                }
            },
            Event::Eof | Event::Comment(_) | Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {
                Ok(None)
            }
            Event::Empty(_) => unreachable!("Empty events are expended"),
            Event::CData(_) => Err(QueryResultsSyntaxError::msg(
                "<![CDATA[...]]> are not supported in SPARQL XML results",
            )
            .into()),
        }
    }
}

fn build_literal(
    value: impl Into<String>,
    lang: Option<String>,
    datatype: Option<NamedNode>,
) -> Result<Literal, QueryResultsParseError> {
    match lang {
        Some(lang) => {
            if let Some(datatype) = datatype {
                if datatype.as_ref() != rdf::LANG_STRING {
                    return Err(QueryResultsSyntaxError::msg(format!(
                        "xml:lang value '{lang}' provided with the datatype {datatype}"
                    ))
                    .into());
                }
            }
            Literal::new_language_tagged_literal(value, &lang).map_err(|e| {
                QueryResultsSyntaxError::msg(format!("Invalid xml:lang value '{lang}': {e}")).into()
            })
        }
        None => Ok(if let Some(datatype) = datatype {
            Literal::new_typed_literal(value, datatype)
        } else {
            Literal::new_simple_literal(value)
        }),
    }
}

fn map_xml_error(error: quick_xml::Error) -> io::Error {
    match error {
        quick_xml::Error::Io(error) => {
            Arc::try_unwrap(error).unwrap_or_else(|error| io::Error::new(error.kind(), error))
        }
        quick_xml::Error::UnexpectedEof(_) => io::Error::new(io::ErrorKind::UnexpectedEof, error),
        _ => io::Error::new(io::ErrorKind::InvalidData, error),
    }
}
