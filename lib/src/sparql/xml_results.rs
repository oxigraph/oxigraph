//! Implementation of [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/)

use crate::error::{invalid_data_error, invalid_input_error};
use crate::model::vocab::rdf;
use crate::model::*;
use crate::sparql::error::EvaluationError;
use crate::sparql::model::*;
use quick_xml::events::BytesDecl;
use quick_xml::events::BytesEnd;
use quick_xml::events::BytesStart;
use quick_xml::events::BytesText;
use quick_xml::events::Event;
use quick_xml::Reader;
use quick_xml::Writer;
use std::collections::BTreeMap;
use std::io;
use std::io::BufRead;
use std::io::Write;
use std::iter::empty;
use std::rc::Rc;

pub fn write_xml_results(results: QueryResults, sink: impl Write) -> Result<(), EvaluationError> {
    match results {
        QueryResults::Boolean(value) => {
            write_boolean(value, sink).map_err(map_xml_error)?;
            Ok(())
        }
        QueryResults::Solutions(solutions) => write_solutions(solutions, sink),
        QueryResults::Graph(_) => Err(invalid_input_error(
            "Graphs could not be formatted to SPARQL query results XML format",
        )
        .into()),
    }
}

fn write_boolean(value: bool, sink: impl Write) -> Result<(), quick_xml::Error> {
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
    Ok(())
}

fn write_solutions(solutions: QuerySolutionIter, sink: impl Write) -> Result<(), EvaluationError> {
    let mut writer = Writer::new(sink);
    writer
        .write_event(Event::Decl(BytesDecl::new(b"1.0", None, None)))
        .map_err(map_xml_error)?;
    let mut sparql_open = BytesStart::borrowed_name(b"sparql");
    sparql_open.push_attribute(("xmlns", "http://www.w3.org/2005/sparql-results#"));
    writer
        .write_event(Event::Start(sparql_open))
        .map_err(map_xml_error)?;
    writer
        .write_event(Event::Start(BytesStart::borrowed_name(b"head")))
        .map_err(map_xml_error)?;
    for variable in solutions.variables() {
        let mut variable_tag = BytesStart::borrowed_name(b"variable");
        variable_tag.push_attribute(("name", variable.as_str()));
        writer
            .write_event(Event::Empty(variable_tag))
            .map_err(map_xml_error)?;
    }
    writer
        .write_event(Event::End(BytesEnd::borrowed(b"head")))
        .map_err(map_xml_error)?;
    writer
        .write_event(Event::Start(BytesStart::borrowed_name(b"results")))
        .map_err(map_xml_error)?;
    for solution in solutions {
        let solution = solution?;
        writer
            .write_event(Event::Start(BytesStart::borrowed_name(b"result")))
            .map_err(map_xml_error)?;
        for (variable, value) in solution.iter() {
            let mut binding_tag = BytesStart::borrowed_name(b"binding");
            binding_tag.push_attribute(("name", variable.as_str()));
            writer
                .write_event(Event::Start(binding_tag))
                .map_err(map_xml_error)?;
            write_xml_term(value.as_ref(), &mut writer)?;
            writer
                .write_event(Event::End(BytesEnd::borrowed(b"binding")))
                .map_err(map_xml_error)?;
        }
        writer
            .write_event(Event::End(BytesEnd::borrowed(b"result")))
            .map_err(map_xml_error)?;
    }
    writer
        .write_event(Event::End(BytesEnd::borrowed(b"results")))
        .map_err(map_xml_error)?;
    writer
        .write_event(Event::End(BytesEnd::borrowed(b"sparql")))
        .map_err(map_xml_error)?;
    Ok(())
}

fn write_xml_term(
    term: TermRef<'_>,
    writer: &mut Writer<impl Write>,
) -> Result<(), EvaluationError> {
    match term {
        TermRef::NamedNode(uri) => {
            writer
                .write_event(Event::Start(BytesStart::borrowed_name(b"uri")))
                .map_err(map_xml_error)?;
            writer
                .write_event(Event::Text(BytesText::from_plain_str(uri.as_str())))
                .map_err(map_xml_error)?;
            writer
                .write_event(Event::End(BytesEnd::borrowed(b"uri")))
                .map_err(map_xml_error)?;
        }
        TermRef::BlankNode(bnode) => {
            writer
                .write_event(Event::Start(BytesStart::borrowed_name(b"bnode")))
                .map_err(map_xml_error)?;
            writer
                .write_event(Event::Text(BytesText::from_plain_str(bnode.as_str())))
                .map_err(map_xml_error)?;
            writer
                .write_event(Event::End(BytesEnd::borrowed(b"bnode")))
                .map_err(map_xml_error)?;
        }
        TermRef::Literal(literal) => {
            let mut literal_tag = BytesStart::borrowed_name(b"literal");
            if let Some(language) = literal.language() {
                literal_tag.push_attribute(("xml:lang", language));
            } else if !literal.is_plain() {
                literal_tag.push_attribute(("datatype", literal.datatype().as_str()));
            }
            writer
                .write_event(Event::Start(literal_tag))
                .map_err(map_xml_error)?;
            writer
                .write_event(Event::Text(BytesText::from_plain_str(literal.value())))
                .map_err(map_xml_error)?;
            writer
                .write_event(Event::End(BytesEnd::borrowed(b"literal")))
                .map_err(map_xml_error)?;
        }
        TermRef::Triple(triple) => {
            writer
                .write_event(Event::Start(BytesStart::borrowed_name(b"triple")))
                .map_err(map_xml_error)?;
            writer
                .write_event(Event::Start(BytesStart::borrowed_name(b"subject")))
                .map_err(map_xml_error)?;
            write_xml_term(triple.subject.as_ref().into(), writer)?;
            writer
                .write_event(Event::End(BytesEnd::borrowed(b"subject")))
                .map_err(map_xml_error)?;
            writer
                .write_event(Event::Start(BytesStart::borrowed_name(b"predicate")))
                .map_err(map_xml_error)?;
            write_xml_term(triple.predicate.as_ref().into(), writer)?;
            writer
                .write_event(Event::End(BytesEnd::borrowed(b"predicate")))
                .map_err(map_xml_error)?;
            writer
                .write_event(Event::Start(BytesStart::borrowed_name(b"object")))
                .map_err(map_xml_error)?;
            write_xml_term(triple.object.as_ref(), writer)?;
            writer
                .write_event(Event::End(BytesEnd::borrowed(b"object")))
                .map_err(map_xml_error)?;
            writer
                .write_event(Event::End(BytesEnd::borrowed(b"triple")))
                .map_err(map_xml_error)?;
        }
    }
    Ok(())
}

pub fn read_xml_results(source: impl BufRead + 'static) -> io::Result<QueryResults> {
    enum State {
        Start,
        Sparql,
        Head,
        AfterHead,
        Boolean,
    }

    let mut reader = Reader::from_reader(source);
    reader.trim_text(true);

    let mut buffer = Vec::default();
    let mut namespace_buffer = Vec::default();
    let mut variables: Vec<String> = Vec::default();
    let mut state = State::Start;

    //Read header
    loop {
        let event = {
            let (ns, event) = reader
                .read_namespaced_event(&mut buffer, &mut namespace_buffer)
                .map_err(map_xml_error)?;
            if let Some(ns) = ns {
                if ns != b"http://www.w3.org/2005/sparql-results#".as_ref() {
                    return Err(invalid_data_error(format!(
                        "Unexpected namespace found in RDF/XML query result: {}",
                        reader.decode(ns).map_err(map_xml_error)?
                    )));
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
                        return Err(invalid_data_error(format!("Expecting <sparql> tag, found {}", reader.decode(event.name()).map_err(map_xml_error)?)));
                    }
                }
                State::Sparql => {
                    if event.name() == b"head" {
                        state = State::Head;
                    } else {
                        return Err(invalid_data_error(format!("Expecting <head> tag, found {}", reader.decode(event.name()).map_err(map_xml_error)?)));
                    }
                }
                State::Head => {
                    if event.name() == b"variable" {
                        let name = event.attributes()
                            .filter_map(std::result::Result::ok)
                            .find(|attr| attr.key == b"name")
                            .ok_or_else(|| invalid_data_error("No name attribute found for the <variable> tag"))?;
                        variables.push(name.unescape_and_decode_value(&reader).map_err(map_xml_error)?);
                    } else if event.name() == b"link" {
                        // no op
                    } else {
                        return Err(invalid_data_error(format!("Expecting <variable> or <link> tag, found {}", reader.decode(event.name()).map_err(map_xml_error)?)));
                    }
                }
                State::AfterHead => {
                    if event.name() == b"boolean" {
                        state = State::Boolean
                    } else if event.name() == b"results" {
                        let mut mapping = BTreeMap::default();
                        for (i,var) in variables.iter().enumerate() {
                            mapping.insert(var.as_bytes().to_vec(), i);
                        }
                        return Ok(QueryResults::Solutions(QuerySolutionIter::new(
                            Rc::new(variables.into_iter().map(Variable::new).collect::<Result<Vec<_>,_>>().map_err(invalid_data_error)?),
                            Box::new(ResultsIterator {
                                reader,
                                buffer,
                                namespace_buffer,
                                mapping,
                                stack: Vec::new(),
                                subject_stack: Vec::new(),
                                predicate_stack: Vec::new(),
                                object_stack:Vec::new(),
                            }),
                        )));
                    } else if event.name() != b"link" && event.name() != b"results" && event.name() != b"boolean" {
                        return Err(invalid_data_error(format!("Expecting sparql tag, found {}", reader.decode(event.name()).map_err(map_xml_error)?)));
                    }
                }
                State::Boolean => return Err(invalid_data_error(format!("Unexpected tag inside of <boolean> tag: {}", reader.decode(event.name()).map_err(map_xml_error)?)))
            },
            Event::Empty(event) => match state {
                State::Sparql => {
                    if event.name() == b"head" {
                        state = State::AfterHead;
                    } else {
                        return Err(invalid_data_error(format!("Expecting <head> tag, found {}", reader.decode(event.name()).map_err(map_xml_error)?)));
                    }
                }
                State::Head => {
                    if event.name() == b"variable" {
                        let name = event.attributes()
                            .filter_map(std::result::Result::ok)
                            .find(|attr| attr.key == b"name")
                            .ok_or_else(|| invalid_data_error("No name attribute found for the <variable> tag"))?;
                        variables.push(name.unescape_and_decode_value(&reader).map_err(map_xml_error)?);
                    } else if event.name() == b"link" {
                        // no op
                    } else {
                        return Err(invalid_data_error(format!("Expecting <variable> or <link> tag, found {}", reader.decode(event.name()).map_err(map_xml_error)?)));
                    }
                },
                State::AfterHead => {
                    return if event.name() == b"results" {
                        Ok(QueryResults::Solutions(QuerySolutionIter::new(
                            Rc::new(variables.into_iter().map(Variable::new).collect::<Result<Vec<_>,_>>().map_err(invalid_data_error)?),
                            Box::new(empty()),
                        )))
                    } else {
                        Err(invalid_data_error(format!("Unexpected autoclosing tag <{}>", reader.decode(event.name()).map_err(map_xml_error)?)))
                    }
                }
                _ => return Err(invalid_data_error(format!("Unexpected autoclosing tag <{}>", reader.decode(event.name()).map_err(map_xml_error)?)))
            },
            Event::Text(event) => {
                let value = event.unescaped().map_err(map_xml_error)?;
                return match state {
                    State::Boolean => {
                        return if value.as_ref() == b"true" {
                            Ok(QueryResults::Boolean(true))
                        } else if value.as_ref() == b"false" {
                            Ok(QueryResults::Boolean(false))
                        } else {
                            Err(invalid_data_error(format!("Unexpected boolean value. Found {}", reader.decode(&value).map_err(map_xml_error)?)))
                        };
                    }
                    _ => Err(invalid_data_error(format!("Unexpected textual value found: {}", reader.decode(&value).map_err(map_xml_error)?)))
                };
            },
            Event::End(_) => if let State::Head = state {
                state = State::AfterHead;
            } else {
                return Err(invalid_data_error("Unexpected early file end. All results file should have a <head> and a <result> or <boolean> tag"));
            },
            Event::Eof => return Err(invalid_data_error("Unexpected early file end. All results file should have a <head> and a <result> or <boolean> tag")),
            _ => (),
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

struct ResultsIterator<R: BufRead> {
    reader: Reader<R>,
    buffer: Vec<u8>,
    namespace_buffer: Vec<u8>,
    mapping: BTreeMap<Vec<u8>, usize>,
    stack: Vec<State>,
    subject_stack: Vec<Term>,
    predicate_stack: Vec<Term>,
    object_stack: Vec<Term>,
}

impl<R: BufRead> Iterator for ResultsIterator<R> {
    type Item = Result<Vec<Option<Term>>, EvaluationError>;

    fn next(&mut self) -> Option<Result<Vec<Option<Term>>, EvaluationError>> {
        self.read_next().transpose()
    }
}

impl<R: BufRead> ResultsIterator<R> {
    fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, EvaluationError> {
        let mut state = State::Start;

        let mut new_bindings = vec![None; self.mapping.len()];

        let mut current_var = None;
        let mut term: Option<Term> = None;
        let mut lang = None;
        let mut datatype = None;
        loop {
            let (ns, event) = self
                .reader
                .read_namespaced_event(&mut self.buffer, &mut self.namespace_buffer)
                .map_err(map_xml_error)?;
            if let Some(ns) = ns {
                if ns != b"http://www.w3.org/2005/sparql-results#".as_ref() {
                    return Err(invalid_data_error(format!(
                        "Unexpected namespace found in RDF/XML query result: {}",
                        self.reader.decode(ns).map_err(map_xml_error)?
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
                            return Err(invalid_data_error(format!(
                                "Expecting <result>, found {}",
                                self.reader.decode(event.name()).map_err(map_xml_error)?
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
                                Some(attr) => {
                                    current_var = Some(
                                        attr.unescaped_value().map_err(map_xml_error)?.to_vec(),
                                    )
                                }
                                None => {
                                    return Err(invalid_data_error(
                                        "No name attribute found for the <binding> tag",
                                    )
                                    .into());
                                }
                            }
                            state = State::Binding;
                        } else {
                            return Err(invalid_data_error(format!(
                                "Expecting <binding>, found {}",
                                self.reader.decode(event.name()).map_err(map_xml_error)?
                            ))
                            .into());
                        }
                    }
                    State::Binding | State::Subject | State::Predicate | State::Object => {
                        if term.is_some() {
                            return Err(invalid_data_error(
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
                                    lang = Some(
                                        attr.unescape_and_decode_value(&self.reader)
                                            .map_err(map_xml_error)?,
                                    );
                                } else if attr.key == b"datatype" {
                                    let iri = attr
                                        .unescape_and_decode_value(&self.reader)
                                        .map_err(map_xml_error)?;
                                    datatype = Some(NamedNode::new(&iri).map_err(|e| {
                                        invalid_data_error(format!(
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
                            return Err(invalid_data_error(format!(
                                "Expecting <uri>, <bnode> or <literal> found {}",
                                self.reader.decode(event.name()).map_err(map_xml_error)?
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
                            return Err(invalid_data_error(format!(
                                "Expecting <subject>, <predicate> or <object> found {}",
                                self.reader.decode(event.name()).map_err(map_xml_error)?
                            ))
                            .into());
                        }
                    }
                    _ => (),
                },
                Event::Text(event) => {
                    let data = event.unescaped().map_err(map_xml_error)?;
                    match state {
                        State::Uri => {
                            let iri = self.reader.decode(&data).map_err(map_xml_error)?;
                            term = Some(
                                NamedNode::new(iri)
                                    .map_err(|e| {
                                        invalid_data_error(format!(
                                            "Invalid IRI value '{}': {}",
                                            iri, e
                                        ))
                                    })?
                                    .into(),
                            )
                        }
                        State::BNode => {
                            let bnode = self.reader.decode(&data).map_err(map_xml_error)?;
                            term = Some(
                                BlankNode::new(bnode)
                                    .map_err(|e| {
                                        invalid_data_error(format!(
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
                                    self.reader.decode(&data).map_err(map_xml_error)?,
                                    lang.take(),
                                    datatype.take(),
                                )?
                                .into(),
                            );
                        }
                        _ => {
                            return Err(invalid_data_error(format!(
                                "Unexpected textual value found: {}",
                                self.reader.decode(&data).map_err(map_xml_error)?
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
                                    invalid_data_error(format!("The variable '{}' is used in a binding but not declared in the variables list",  self.reader.decode(&var).map_err(map_xml_error)?)).into()
                                );
                            }
                        } else {
                            return Err(
                                invalid_data_error("No name found for <binding> tag").into()
                            );
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
                                            return Err(invalid_data_error(
                                                "The <subject> value should not be a <literal>",
                                            )
                                            .into())
                                        }
                                    },
                                    match predicate {
                                        Term::NamedNode(predicate) => predicate,
                                        _ => {
                                            return Err(invalid_data_error(
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
                                invalid_data_error("A <triple> should contain a <subject>, a <predicate> and an <object>").into()
                            );
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
) -> Result<Literal, EvaluationError> {
    match lang {
        Some(lang) => {
            if let Some(datatype) = datatype {
                if datatype.as_ref() != rdf::LANG_STRING {
                    return Err(invalid_data_error(format!(
                        "xml:lang value '{}' provided with the datatype {}",
                        lang, datatype
                    ))
                    .into());
                }
            }
            Literal::new_language_tagged_literal(value, &lang).map_err(|e| {
                invalid_data_error(format!("Invalid xml:lang value '{}': {}", lang, e)).into()
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
        _ => invalid_data_error(error),
    }
}
