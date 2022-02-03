//! Implementation of [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/)

use crate::error::{ParseError, SyntaxError};
use oxrdf::vocab::rdf;
use oxrdf::Variable;
use oxrdf::*;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

pub fn write_boolean_xml_result<W: Write>(sink: W, value: bool) -> io::Result<W> {
    do_write_boolean_xml_result(sink, value).map_err(map_xml_error)
}

fn do_write_boolean_xml_result<W: Write>(sink: W, value: bool) -> Result<W, quick_xml::Error> {
    let mut writer = Writer::new(sink);
    writer.write_event(Event::Decl(BytesDecl::new(b"1.0", None, None)))?;
    let mut sparql_open = BytesStart::borrowed_name(b"sparql");
    sparql_open.push_attribute(("xmlns", "http://www.w3.org/2005/sparql-results#"));
    writer.write_event(Event::Start(sparql_open))?;
    writer.write_event(Event::Start(BytesStart::borrowed_name(b"head")))?;
    writer.write_event(Event::End(BytesEnd::borrowed(b"head")))?;
    writer.write_event(Event::Start(BytesStart::borrowed_name(b"boolean")))?;
    writer.write_event(Event::Text(BytesText::from_plain_str(if value {
        "true"
    } else {
        "false"
    })))?;
    writer.write_event(Event::End(BytesEnd::borrowed(b"boolean")))?;
    writer.write_event(Event::End(BytesEnd::borrowed(b"sparql")))?;
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
        writer.write_event(Event::Decl(BytesDecl::new(b"1.0", None, None)))?;
        let mut sparql_open = BytesStart::borrowed_name(b"sparql");
        sparql_open.push_attribute(("xmlns", "http://www.w3.org/2005/sparql-results#"));
        writer.write_event(Event::Start(sparql_open))?;
        writer.write_event(Event::Start(BytesStart::borrowed_name(b"head")))?;
        for variable in &variables {
            let mut variable_tag = BytesStart::borrowed_name(b"variable");
            variable_tag.push_attribute(("name", variable.as_str()));
            writer.write_event(Event::Empty(variable_tag))?;
        }
        writer.write_event(Event::End(BytesEnd::borrowed(b"head")))?;
        writer.write_event(Event::Start(BytesStart::borrowed_name(b"results")))?;
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
            .write_event(Event::Start(BytesStart::borrowed_name(b"result")))?;
        for (variable, value) in solution {
            let mut binding_tag = BytesStart::borrowed_name(b"binding");
            binding_tag.push_attribute(("name", variable.as_str()));
            self.writer.write_event(Event::Start(binding_tag))?;
            write_xml_term(value, &mut self.writer)?;
            self.writer
                .write_event(Event::End(BytesEnd::borrowed(b"binding")))?;
        }
        self.writer
            .write_event(Event::End(BytesEnd::borrowed(b"result")))
    }

    pub fn finish(self) -> io::Result<W> {
        let mut inner = self.do_finish().map_err(map_xml_error)?;
        inner.flush()?;
        Ok(inner)
    }

    fn do_finish(mut self) -> Result<W, quick_xml::Error> {
        self.writer
            .write_event(Event::End(BytesEnd::borrowed(b"results")))?;
        self.writer
            .write_event(Event::End(BytesEnd::borrowed(b"sparql")))?;
        Ok(self.writer.into_inner())
    }
}

fn write_xml_term(
    term: TermRef<'_>,
    writer: &mut Writer<impl Write>,
) -> Result<(), quick_xml::Error> {
    match term {
        TermRef::NamedNode(uri) => {
            writer.write_event(Event::Start(BytesStart::borrowed_name(b"uri")))?;
            writer.write_event(Event::Text(BytesText::from_plain_str(uri.as_str())))?;
            writer.write_event(Event::End(BytesEnd::borrowed(b"uri")))?;
        }
        TermRef::BlankNode(bnode) => {
            writer.write_event(Event::Start(BytesStart::borrowed_name(b"bnode")))?;
            writer.write_event(Event::Text(BytesText::from_plain_str(bnode.as_str())))?;
            writer.write_event(Event::End(BytesEnd::borrowed(b"bnode")))?;
        }
        TermRef::Literal(literal) => {
            let mut literal_tag = BytesStart::borrowed_name(b"literal");
            if let Some(language) = literal.language() {
                literal_tag.push_attribute(("xml:lang", language));
            } else if !literal.is_plain() {
                literal_tag.push_attribute(("datatype", literal.datatype().as_str()));
            }
            writer.write_event(Event::Start(literal_tag))?;
            writer.write_event(Event::Text(BytesText::from_plain_str(literal.value())))?;
            writer.write_event(Event::End(BytesEnd::borrowed(b"literal")))?;
        }
        #[cfg(feature = "rdf-star")]
        TermRef::Triple(triple) => {
            writer.write_event(Event::Start(BytesStart::borrowed_name(b"triple")))?;
            writer.write_event(Event::Start(BytesStart::borrowed_name(b"subject")))?;
            write_xml_term(triple.subject.as_ref().into(), writer)?;
            writer.write_event(Event::End(BytesEnd::borrowed(b"subject")))?;
            writer.write_event(Event::Start(BytesStart::borrowed_name(b"predicate")))?;
            write_xml_term(triple.predicate.as_ref().into(), writer)?;
            writer.write_event(Event::End(BytesEnd::borrowed(b"predicate")))?;
            writer.write_event(Event::Start(BytesStart::borrowed_name(b"object")))?;
            write_xml_term(triple.object.as_ref(), writer)?;
            writer.write_event(Event::End(BytesEnd::borrowed(b"object")))?;
            writer.write_event(Event::End(BytesEnd::borrowed(b"triple")))?;
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
        let mut namespace_buffer = Vec::default();
        let mut variables = Vec::default();
        let mut state = State::Start;

        //Read header
        loop {
            let event = {
                let (ns, event) =
                    reader.read_namespaced_event(&mut buffer, &mut namespace_buffer)?;
                if let Some(ns) = ns {
                    if ns != b"http://www.w3.org/2005/sparql-results#".as_ref() {
                        return Err(SyntaxError::msg(format!(
                            "Unexpected namespace found in RDF/XML query result: {}",
                            reader.decode(ns)?
                        ))
                        .into());
                    }
                }
                event
            };
            match event {
                Event::Start(event) => match state {
                    State::Start => {
                        if event.name() == b"sparql" {
                            state = State::Sparql;
                        } else {
                            return Err(SyntaxError::msg(format!("Expecting <sparql> tag, found {}", reader.decode(event.name())?)).into());
                        }
                    }
                    State::Sparql => {
                        if event.name() == b"head" {
                            state = State::Head;
                        } else {
                            return Err(SyntaxError::msg(format!("Expecting <head> tag, found {}", reader.decode(event.name())?)).into());
                        }
                    }
                    State::Head => {
                        if event.name() == b"variable" {
                            let name = event.attributes()
                                .filter_map(std::result::Result::ok)
                                .find(|attr| attr.key == b"name")
                                .ok_or_else(|| SyntaxError::msg("No name attribute found for the <variable> tag"))?
                                .unescape_and_decode_value(&reader)?;
                            let variable = Variable::new(name).map_err(|e| SyntaxError::msg(format!("Invalid variable name: {}", e)))?;
                            if variables.contains(&variable) {
                                return Err(SyntaxError::msg(format!(
                                    "The variable {} is declared twice",
                                    variable
                                ))
                                    .into());
                            }
                            variables.push(variable);
                        } else if event.name() == b"link" {
                            // no op
                        } else {
                            return Err(SyntaxError::msg(format!("Expecting <variable> or <link> tag, found {}", reader.decode(event.name())?)).into());
                        }
                    }
                    State::AfterHead => {
                        if event.name() == b"boolean" {
                            state = State::Boolean
                        } else if event.name() == b"results" {
                            let mut mapping = BTreeMap::default();
                            for (i, var) in variables.iter().enumerate() {
                                mapping.insert(var.as_str().as_bytes().to_vec(), i);
                            }
                            return Ok(Self::Solutions { variables,
                                solutions: XmlSolutionsReader {
                                    reader,
                                    buffer,
                                    namespace_buffer,
                                    mapping,
                                    stack: Vec::new(),
                                    subject_stack: Vec::new(),
                                    predicate_stack: Vec::new(),
                                    object_stack: Vec::new(),
                                }});
                        } else if event.name() != b"link" && event.name() != b"results" && event.name() != b"boolean" {
                            return Err(SyntaxError::msg(format!("Expecting sparql tag, found {}", reader.decode(event.name())?)).into());
                        }
                    }
                    State::Boolean => return Err(SyntaxError::msg(format!("Unexpected tag inside of <boolean> tag: {}", reader.decode(event.name())?)).into())
                },
                Event::Text(event) => {
                    let value = event.unescaped()?;
                    return match state {
                        State::Boolean => {
                            return if value.as_ref() == b"true" {
                                Ok(Self::Boolean(true))
                            } else if value.as_ref() == b"false" {
                                Ok(Self::Boolean(false))
                            } else {
                                Err(SyntaxError::msg(format!("Unexpected boolean value. Found {}", reader.decode(&value)?)).into())
                            };
                        }
                        _ => Err(SyntaxError::msg(format!("Unexpected textual value found: {}", reader.decode(&value)?)).into())
                    };
                },
                Event::End(event) => {
                    if let State::Head = state {
                        if event.name() == b"head" {
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
    namespace_buffer: Vec<u8>,
    mapping: BTreeMap<Vec<u8>, usize>,
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
            let (ns, event) = self
                .reader
                .read_namespaced_event(&mut self.buffer, &mut self.namespace_buffer)?;
            if let Some(ns) = ns {
                if ns != b"http://www.w3.org/2005/sparql-results#".as_ref() {
                    return Err(SyntaxError::msg(format!(
                        "Unexpected namespace found in RDF/XML query result: {}",
                        self.reader.decode(ns)?
                    ))
                    .into());
                }
            }
            match event {
                Event::Start(event) => match state {
                    State::Start => {
                        if event.name() == b"result" {
                            state = State::Result;
                        } else {
                            return Err(SyntaxError::msg(format!(
                                "Expecting <result>, found {}",
                                self.reader.decode(event.name())?
                            ))
                            .into());
                        }
                    }
                    State::Result => {
                        if event.name() == b"binding" {
                            match event
                                .attributes()
                                .filter_map(std::result::Result::ok)
                                .find(|attr| attr.key == b"name")
                            {
                                Some(attr) => current_var = Some(attr.unescaped_value()?.to_vec()),
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
                                "Expecting <binding>, found {}",
                                self.reader.decode(event.name())?
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
                        if event.name() == b"uri" {
                            state = State::Uri;
                        } else if event.name() == b"bnode" {
                            state = State::BNode;
                        } else if event.name() == b"literal" {
                            for attr in event.attributes().flatten() {
                                if attr.key == b"xml:lang" {
                                    lang = Some(attr.unescape_and_decode_value(&self.reader)?);
                                } else if attr.key == b"datatype" {
                                    let iri = attr.unescape_and_decode_value(&self.reader)?;
                                    datatype = Some(NamedNode::new(&iri).map_err(|e| {
                                        SyntaxError::msg(format!(
                                            "Invalid datatype IRI '{}': {}",
                                            iri, e
                                        ))
                                    })?);
                                }
                            }
                            state = State::Literal;
                        } else if event.name() == b"triple" {
                            state = State::Triple;
                        } else {
                            return Err(SyntaxError::msg(format!(
                                "Expecting <uri>, <bnode> or <literal> found {}",
                                self.reader.decode(event.name())?
                            ))
                            .into());
                        }
                    }
                    State::Triple => {
                        if event.name() == b"subject" {
                            state = State::Subject
                        } else if event.name() == b"predicate" {
                            state = State::Predicate
                        } else if event.name() == b"object" {
                            state = State::Object
                        } else {
                            return Err(SyntaxError::msg(format!(
                                "Expecting <subject>, <predicate> or <object> found {}",
                                self.reader.decode(event.name())?
                            ))
                            .into());
                        }
                    }
                    _ => (),
                },
                Event::Text(event) => {
                    let data = event.unescaped()?;
                    match state {
                        State::Uri => {
                            let iri = self.reader.decode(&data)?;
                            term = Some(
                                NamedNode::new(iri)
                                    .map_err(|e| {
                                        SyntaxError::msg(format!(
                                            "Invalid IRI value '{}': {}",
                                            iri, e
                                        ))
                                    })?
                                    .into(),
                            )
                        }
                        State::BNode => {
                            let bnode = self.reader.decode(&data)?;
                            term = Some(
                                BlankNode::new(bnode)
                                    .map_err(|e| {
                                        SyntaxError::msg(format!(
                                            "Invalid blank node value '{}': {}",
                                            bnode, e
                                        ))
                                    })?
                                    .into(),
                            )
                        }
                        State::Literal => {
                            term = Some(
                                build_literal(
                                    self.reader.decode(&data)?,
                                    lang.take(),
                                    datatype.take(),
                                )?
                                .into(),
                            );
                        }
                        _ => {
                            return Err(SyntaxError::msg(format!(
                                "Unexpected textual value found: {}",
                                self.reader.decode(&data)?
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
                                    SyntaxError::msg(format!("The variable '{}' is used in a binding but not declared in the variables list",  self.reader.decode(var)?)).into()
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
                        "xml:lang value '{}' provided with the datatype {}",
                        lang, datatype
                    ))
                    .into());
                }
            }
            Literal::new_language_tagged_literal(value, &lang).map_err(|e| {
                SyntaxError::msg(format!("Invalid xml:lang value '{}': {}", lang, e)).into()
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
        quick_xml::Error::Io(error) => error,
        quick_xml::Error::UnexpectedEof(_) => io::Error::new(io::ErrorKind::UnexpectedEof, error),
        _ => io::Error::new(io::ErrorKind::InvalidData, error),
    }
}
