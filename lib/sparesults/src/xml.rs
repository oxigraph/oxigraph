//! Implementation of [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/)

use crate::error::{ParseError, SyntaxError};
use oxrdf::vocab::rdf;
use oxrdf::Variable;
use oxrdf::*;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::str;

pub fn write_boolean_xml_result<W: Write>(sink: W, value: bool) -> io::Result<W> {
    do_write_boolean_xml_result(sink, value).map_err(map_xml_error)
}

fn do_write_boolean_xml_result<W: Write>(sink: W, value: bool) -> Result<W, quick_xml::Error> {
    let mut writer = Writer::new(sink);
    writer.write_event(Event::Decl(BytesDecl::new("1.0", None, None)))?;
    let mut sparql_open = BytesStart::new("sparql");
    sparql_open.push_attribute(("xmlns", "http://www.w3.org/2005/sparql-results#"));
    writer.write_event(Event::Start(sparql_open))?;
    writer.write_event(Event::Start(BytesStart::new("head")))?;
    writer.write_event(Event::End(BytesEnd::new("head")))?;
    writer.write_event(Event::Start(BytesStart::new("boolean")))?;
    writer.write_event(Event::Text(BytesText::new(if value {
        "true"
    } else {
        "false"
    })))?;
    writer.write_event(Event::End(BytesEnd::new("boolean")))?;
    writer.write_event(Event::End(BytesEnd::new("sparql")))?;
    Ok(writer.into_inner())
}

pub struct XmlSolutionsWriter<W: Write> {
    writer: Writer<W>,
}

impl<W: Write> XmlSolutionsWriter<W> {
    pub fn start(sink: W, variables: Vec<Variable>) -> io::Result<Self> {
        Self::do_start(sink, variables).map_err(map_xml_error)
    }

    fn do_start(sink: W, variables: Vec<Variable>) -> Result<Self, quick_xml::Error> {
        let mut writer = Writer::new(sink);
        writer.write_event(Event::Decl(BytesDecl::new("1.0", None, None)))?;
        let mut sparql_open = BytesStart::new("sparql");
        sparql_open.push_attribute(("xmlns", "http://www.w3.org/2005/sparql-results#"));
        writer.write_event(Event::Start(sparql_open))?;
        writer.write_event(Event::Start(BytesStart::new("head")))?;
        for variable in &variables {
            let mut variable_tag = BytesStart::new("variable");
            variable_tag.push_attribute(("name", variable.as_str()));
            writer.write_event(Event::Empty(variable_tag))?;
        }
        writer.write_event(Event::End(BytesEnd::new("head")))?;
        writer.write_event(Event::Start(BytesStart::new("results")))?;
        Ok(Self { writer })
    }

    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> io::Result<()> {
        self.do_write(solution).map_err(map_xml_error)
    }

    fn do_write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> Result<(), quick_xml::Error> {
        self.writer
            .write_event(Event::Start(BytesStart::new("result")))?;
        for (variable, value) in solution {
            let mut binding_tag = BytesStart::new("binding");
            binding_tag.push_attribute(("name", variable.as_str()));
            self.writer.write_event(Event::Start(binding_tag))?;
            write_xml_term(value, &mut self.writer)?;
            self.writer
                .write_event(Event::End(BytesEnd::new("binding")))?;
        }
        self.writer.write_event(Event::End(BytesEnd::new("result")))
    }

    pub fn finish(self) -> io::Result<W> {
        let mut inner = self.do_finish().map_err(map_xml_error)?;
        inner.flush()?;
        Ok(inner)
    }

    fn do_finish(mut self) -> Result<W, quick_xml::Error> {
        self.writer
            .write_event(Event::End(BytesEnd::new("results")))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("sparql")))?;
        Ok(self.writer.into_inner())
    }
}

fn write_xml_term(
    term: TermRef<'_>,
    writer: &mut Writer<impl Write>,
) -> Result<(), quick_xml::Error> {
    match term {
        TermRef::NamedNode(uri) => {
            writer.write_event(Event::Start(BytesStart::new("uri")))?;
            writer.write_event(Event::Text(BytesText::new(uri.as_str())))?;
            writer.write_event(Event::End(BytesEnd::new("uri")))?;
        }
        TermRef::BlankNode(bnode) => {
            writer.write_event(Event::Start(BytesStart::new("bnode")))?;
            writer.write_event(Event::Text(BytesText::new(bnode.as_str())))?;
            writer.write_event(Event::End(BytesEnd::new("bnode")))?;
        }
        TermRef::Literal(literal) => {
            let mut literal_tag = BytesStart::new("literal");
            if let Some(language) = literal.language() {
                literal_tag.push_attribute(("xml:lang", language));
            } else if !literal.is_plain() {
                literal_tag.push_attribute(("datatype", literal.datatype().as_str()));
            }
            writer.write_event(Event::Start(literal_tag))?;
            writer.write_event(Event::Text(BytesText::new(literal.value())))?;
            writer.write_event(Event::End(BytesEnd::new("literal")))?;
        }
        #[cfg(feature = "rdf-star")]
        TermRef::Triple(triple) => {
            writer.write_event(Event::Start(BytesStart::new("triple")))?;
            writer.write_event(Event::Start(BytesStart::new("subject")))?;
            write_xml_term(triple.subject.as_ref().into(), writer)?;
            writer.write_event(Event::End(BytesEnd::new("subject")))?;
            writer.write_event(Event::Start(BytesStart::new("predicate")))?;
            write_xml_term(triple.predicate.as_ref().into(), writer)?;
            writer.write_event(Event::End(BytesEnd::new("predicate")))?;
            writer.write_event(Event::Start(BytesStart::new("object")))?;
            write_xml_term(triple.object.as_ref(), writer)?;
            writer.write_event(Event::End(BytesEnd::new("object")))?;
            writer.write_event(Event::End(BytesEnd::new("triple")))?;
        }
    }
    Ok(())
}

pub enum XmlQueryResultsReader<R: BufRead> {
    Solutions {
        variables: Vec<Variable>,
        solutions: XmlSolutionsReader<R>,
    },
    Boolean(bool),
}

impl<R: BufRead> XmlQueryResultsReader<R> {
    pub fn read(source: R) -> Result<Self, ParseError> {
        enum State {
            Start,
            Sparql,
            Head,
            AfterHead,
            Boolean,
        }

        let mut reader = Reader::from_reader(source);
        reader.trim_text(true);
        reader.expand_empty_elements(true);

        let mut buffer = Vec::default();
        let mut variables = Vec::default();
        let mut state = State::Start;

        //Read header
        loop {
            let event = reader.read_event_into(&mut buffer)?;
            match event {
                Event::Start(event) => match state {
                    State::Start => {
                        if event.local_name().as_ref() == b"sparql" {
                            state = State::Sparql;
                        } else {
                            return Err(SyntaxError::msg(format!("Expecting <sparql> tag, found <{}>", decode(&reader, &event.name())?)).into());
                        }
                    }
                    State::Sparql => {
                        if event.local_name().as_ref() == b"head" {
                            state = State::Head;
                        } else {
                            return Err(SyntaxError::msg(format!("Expecting <head> tag, found <{}>",decode(&reader, &event.name())?)).into());
                        }
                    }
                    State::Head => {
                        if event.local_name().as_ref() == b"variable" {
                            let name = event.attributes()
                                .filter_map(Result::ok)
                                .find(|attr| attr.key.local_name().as_ref() == b"name")
                                .ok_or_else(|| SyntaxError::msg("No name attribute found for the <variable> tag"))?
                                .decode_and_unescape_value(&reader)?;
                            let variable = Variable::new(name).map_err(|e| SyntaxError::msg(format!("Invalid variable name: {e}")))?;
                            if variables.contains(&variable) {
                                return Err(SyntaxError::msg(format!(
                                    "The variable {variable} is declared twice"
                                ))
                                    .into());
                            }
                            variables.push(variable);
                        } else if event.local_name().as_ref() == b"link" {
                            // no op
                        } else {
                            return Err(SyntaxError::msg(format!("Expecting <variable> or <link> tag, found <{}>", decode(&reader, &event.name())?)).into());
                        }
                    }
                    State::AfterHead => {
                        if event.local_name().as_ref() == b"boolean" {
                            state = State::Boolean
                        } else if event.local_name().as_ref() == b"results" {
                            let mut mapping = BTreeMap::default();
                            for (i, var) in variables.iter().enumerate() {
                                mapping.insert(var.clone().into_string(), i);
                            }
                            return Ok(Self::Solutions { variables,
                                solutions: XmlSolutionsReader {
                                    reader,
                                    buffer,
                                    mapping,
                                    stack: Vec::new(),
                                    subject_stack: Vec::new(),
                                    predicate_stack: Vec::new(),
                                    object_stack: Vec::new(),
                                }});
                        } else if event.local_name().as_ref() != b"link" && event.local_name().as_ref() != b"results" && event.local_name().as_ref() != b"boolean" {
                            return Err(SyntaxError::msg(format!("Expecting sparql tag, found <{}>", decode(&reader, &event.name())?)).into());
                        }
                    }
                    State::Boolean => return Err(SyntaxError::msg(format!("Unexpected tag inside of <boolean> tag: <{}>", decode(&reader, &event.name())?)).into())
                },
                Event::Text(event) => {
                    let value = event.unescape()?;
                    return match state {
                        State::Boolean => {
                            return if value == "true" {
                                Ok(Self::Boolean(true))
                            } else if value == "false" {
                                Ok(Self::Boolean(false))
                            } else {
                                Err(SyntaxError::msg(format!("Unexpected boolean value. Found '{value}'")).into())
                            };
                        }
                        _ => Err(SyntaxError::msg(format!("Unexpected textual value found: '{value}'")).into())
                    };
                },
                Event::End(event) => {
                    if let State::Head = state {
                        if event.local_name().as_ref() == b"head" {
                            state = State::AfterHead
                        }
                    } else {
                        return Err(SyntaxError::msg("Unexpected early file end. All results file should have a <head> and a <result> or <boolean> tag").into());
                    }
                },
                Event::Eof => return Err(SyntaxError::msg("Unexpected early file end. All results file should have a <head> and a <result> or <boolean> tag").into()),
                _ => (),
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
    End,
}

pub struct XmlSolutionsReader<R: BufRead> {
    reader: Reader<R>,
    buffer: Vec<u8>,
    mapping: BTreeMap<String, usize>,
    stack: Vec<State>,
    subject_stack: Vec<Term>,
    predicate_stack: Vec<Term>,
    object_stack: Vec<Term>,
}

impl<R: BufRead> XmlSolutionsReader<R> {
    pub fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, ParseError> {
        let mut state = State::Start;

        let mut new_bindings = vec![None; self.mapping.len()];

        let mut current_var = None;
        let mut term: Option<Term> = None;
        let mut lang = None;
        let mut datatype = None;
        loop {
            let event = self.reader.read_event_into(&mut self.buffer)?;
            match event {
                Event::Start(event) => match state {
                    State::Start => {
                        if event.local_name().as_ref() == b"result" {
                            state = State::Result;
                        } else {
                            return Err(SyntaxError::msg(format!(
                                "Expecting <result>, found <{}>",
                                decode(&self.reader, &event.name())?
                            ))
                            .into());
                        }
                    }
                    State::Result => {
                        if event.local_name().as_ref() == b"binding" {
                            match event
                                .attributes()
                                .filter_map(Result::ok)
                                .find(|attr| attr.key.local_name().as_ref() == b"name")
                            {
                                Some(attr) => {
                                    current_var = Some(
                                        attr.decode_and_unescape_value(&self.reader)?.to_string(),
                                    )
                                }
                                None => {
                                    return Err(SyntaxError::msg(
                                        "No name attribute found for the <binding> tag",
                                    )
                                    .into());
                                }
                            }
                            state = State::Binding;
                        } else {
                            return Err(SyntaxError::msg(format!(
                                "Expecting <binding>, found <{}>",
                                decode(&self.reader, &event.name())?
                            ))
                            .into());
                        }
                    }
                    State::Binding | State::Subject | State::Predicate | State::Object => {
                        if term.is_some() {
                            return Err(SyntaxError::msg(
                                "There is already a value for the current binding",
                            )
                            .into());
                        }
                        self.stack.push(state);
                        if event.local_name().as_ref() == b"uri" {
                            state = State::Uri;
                        } else if event.local_name().as_ref() == b"bnode" {
                            state = State::BNode;
                        } else if event.local_name().as_ref() == b"literal" {
                            for attr in event.attributes().flatten() {
                                if attr.key.as_ref() == b"xml:lang" {
                                    lang = Some(
                                        attr.decode_and_unescape_value(&self.reader)?.to_string(),
                                    );
                                } else if attr.key.local_name().as_ref() == b"datatype" {
                                    let iri = attr.decode_and_unescape_value(&self.reader)?;
                                    datatype =
                                        Some(NamedNode::new(iri.to_string()).map_err(|e| {
                                            SyntaxError::msg(format!(
                                                "Invalid datatype IRI '{iri}': {e}"
                                            ))
                                        })?);
                                }
                            }
                            state = State::Literal;
                        } else if event.local_name().as_ref() == b"triple" {
                            state = State::Triple;
                        } else {
                            return Err(SyntaxError::msg(format!(
                                "Expecting <uri>, <bnode> or <literal> found <{}>",
                                decode(&self.reader, &event.name())?
                            ))
                            .into());
                        }
                    }
                    State::Triple => {
                        if event.local_name().as_ref() == b"subject" {
                            state = State::Subject
                        } else if event.local_name().as_ref() == b"predicate" {
                            state = State::Predicate
                        } else if event.local_name().as_ref() == b"object" {
                            state = State::Object
                        } else {
                            return Err(SyntaxError::msg(format!(
                                "Expecting <subject>, <predicate> or <object> found <{}>",
                                decode(&self.reader, &event.name())?
                            ))
                            .into());
                        }
                    }
                    _ => (),
                },
                Event::Text(event) => {
                    let data = event.unescape()?;
                    match state {
                        State::Uri => {
                            term = Some(
                                NamedNode::new(data.to_string())
                                    .map_err(|e| {
                                        SyntaxError::msg(format!("Invalid IRI value '{data}': {e}"))
                                    })?
                                    .into(),
                            )
                        }
                        State::BNode => {
                            term = Some(
                                BlankNode::new(data.to_string())
                                    .map_err(|e| {
                                        SyntaxError::msg(format!(
                                            "Invalid blank node value '{data}': {e}"
                                        ))
                                    })?
                                    .into(),
                            )
                        }
                        State::Literal => {
                            term = Some(build_literal(data, lang.take(), datatype.take())?.into());
                        }
                        _ => {
                            return Err(SyntaxError::msg(format!(
                                "Unexpected textual value found: {data}"
                            ))
                            .into());
                        }
                    }
                }
                Event::End(_) => match state {
                    State::Start => state = State::End,
                    State::Result => return Ok(Some(new_bindings)),
                    State::Binding => {
                        if let Some(var) = &current_var {
                            if let Some(var) = self.mapping.get(var) {
                                new_bindings[*var] = term.take()
                            } else {
                                return Err(
                                    SyntaxError::msg(format!("The variable '{var}' is used in a binding but not declared in the variables list")).into()
                                );
                            }
                        } else {
                            return Err(SyntaxError::msg("No name found for <binding> tag").into());
                        }
                        state = State::Result;
                    }
                    State::Subject => {
                        if let Some(subject) = term.take() {
                            self.subject_stack.push(subject)
                        }
                        state = State::Triple;
                    }
                    State::Predicate => {
                        if let Some(predicate) = term.take() {
                            self.predicate_stack.push(predicate)
                        }
                        state = State::Triple;
                    }
                    State::Object => {
                        if let Some(object) = term.take() {
                            self.object_stack.push(object)
                        }
                        state = State::Triple;
                    }
                    State::Uri => state = self.stack.pop().unwrap(),
                    State::BNode => {
                        if term.is_none() {
                            //We default to a random bnode
                            term = Some(BlankNode::default().into())
                        }
                        state = self.stack.pop().unwrap()
                    }
                    State::Literal => {
                        if term.is_none() {
                            //We default to the empty literal
                            term = Some(build_literal("", lang.take(), datatype.take())?.into())
                        }
                        state = self.stack.pop().unwrap();
                    }
                    State::Triple => {
                        #[cfg(feature = "rdf-star")]
                        if let (Some(subject), Some(predicate), Some(object)) = (
                            self.subject_stack.pop(),
                            self.predicate_stack.pop(),
                            self.object_stack.pop(),
                        ) {
                            term = Some(
                                Triple::new(
                                    match subject {
                                        Term::NamedNode(subject) => subject.into(),
                                        Term::BlankNode(subject) => subject.into(),
                                        Term::Triple(subject) => Subject::Triple(subject),
                                        Term::Literal(_) => {
                                            return Err(SyntaxError::msg(
                                                "The <subject> value should not be a <literal>",
                                            )
                                            .into())
                                        }
                                    },
                                    match predicate {
                                        Term::NamedNode(predicate) => predicate,
                                        _ => {
                                            return Err(SyntaxError::msg(
                                                "The <predicate> value should be an <uri>",
                                            )
                                            .into())
                                        }
                                    },
                                    object,
                                )
                                .into(),
                            );
                            state = self.stack.pop().unwrap();
                        } else {
                            return Err(
                                SyntaxError::msg("A <triple> should contain a <subject>, a <predicate> and an <object>").into()
                            );
                        }
                        #[cfg(not(feature = "rdf-star"))]
                        {
                            return Err(SyntaxError::msg(
                                "The <triple> tag is only supported with RDF-star",
                            )
                            .into());
                        }
                    }
                    State::End => (),
                },
                Event::Eof => return Ok(None),
                _ => (),
            }
        }
    }
}

fn build_literal(
    value: impl Into<String>,
    lang: Option<String>,
    datatype: Option<NamedNode>,
) -> Result<Literal, ParseError> {
    match lang {
        Some(lang) => {
            if let Some(datatype) = datatype {
                if datatype.as_ref() != rdf::LANG_STRING {
                    return Err(SyntaxError::msg(format!(
                        "xml:lang value '{lang}' provided with the datatype {datatype}"
                    ))
                    .into());
                }
            }
            Literal::new_language_tagged_literal(value, &lang).map_err(|e| {
                SyntaxError::msg(format!("Invalid xml:lang value '{lang}': {e}")).into()
            })
        }
        None => Ok(if let Some(datatype) = datatype {
            Literal::new_typed_literal(value, datatype)
        } else {
            Literal::new_simple_literal(value)
        }),
    }
}

fn decode<'a, T>(
    reader: &Reader<T>,
    data: &'a impl AsRef<[u8]>,
) -> Result<Cow<'a, str>, ParseError> {
    Ok(reader.decoder().decode(data.as_ref())?)
}

fn map_xml_error(error: quick_xml::Error) -> io::Error {
    match error {
        quick_xml::Error::Io(error) => io::Error::new(error.kind(), error),
        quick_xml::Error::UnexpectedEof(_) => io::Error::new(io::ErrorKind::UnexpectedEof, error),
        _ => io::Error::new(io::ErrorKind::InvalidData, error),
    }
}
