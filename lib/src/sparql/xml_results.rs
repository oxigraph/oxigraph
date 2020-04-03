//! Implementation of [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/)

use crate::model::*;
use crate::sparql::model::*;
use crate::Result;
use anyhow::anyhow;
use quick_xml::events::BytesDecl;
use quick_xml::events::BytesEnd;
use quick_xml::events::BytesStart;
use quick_xml::events::BytesText;
use quick_xml::events::Event;
use quick_xml::Reader;
use quick_xml::Writer;
use std::collections::BTreeMap;
use std::io::BufRead;
use std::io::Write;
use std::iter::empty;

pub fn write_xml_results<W: Write>(results: QueryResult<'_>, sink: W) -> Result<W> {
    let mut writer = Writer::new(sink);
    match results {
        QueryResult::Boolean(value) => {
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
        }
        QueryResult::Bindings(bindings) => {
            let (variables, results) = bindings.destruct();
            writer.write_event(Event::Decl(BytesDecl::new(b"1.0", None, None)))?;
            let mut sparql_open = BytesStart::borrowed_name(b"sparql");
            sparql_open.push_attribute(("xmlns", "http://www.w3.org/2005/sparql-results#"));
            writer.write_event(Event::Start(sparql_open))?;
            writer.write_event(Event::Start(BytesStart::borrowed_name(b"head")))?;
            for variable in &variables {
                let mut variable_tag = BytesStart::borrowed_name(b"variable");
                variable_tag.push_attribute(("name", variable.name()?));
                writer.write_event(Event::Empty(variable_tag))?;
            }
            writer.write_event(Event::End(BytesEnd::borrowed(b"head")))?;
            writer.write_event(Event::Start(BytesStart::borrowed_name(b"results")))?;
            for result in results {
                let result = result?;
                writer.write_event(Event::Start(BytesStart::borrowed_name(b"result")))?;
                for (i, value) in result.into_iter().enumerate() {
                    if let Some(term) = value {
                        let mut binding_tag = BytesStart::borrowed_name(b"binding");
                        binding_tag.push_attribute(("name", variables[i].name()?));
                        writer.write_event(Event::Start(binding_tag))?;
                        match term {
                            Term::NamedNode(uri) => {
                                writer
                                    .write_event(Event::Start(BytesStart::borrowed_name(b"uri")))?;
                                writer.write_event(Event::Text(BytesText::from_plain_str(
                                    uri.as_str(),
                                )))?;
                                writer.write_event(Event::End(BytesEnd::borrowed(b"uri")))?;
                            }
                            Term::BlankNode(bnode) => {
                                writer.write_event(Event::Start(BytesStart::borrowed_name(
                                    b"bnode",
                                )))?;
                                writer.write_event(Event::Text(BytesText::from_plain_str(
                                    bnode.as_str(),
                                )))?;
                                writer.write_event(Event::End(BytesEnd::borrowed(b"bnode")))?;
                            }
                            Term::Literal(literal) => {
                                let mut literal_tag = BytesStart::borrowed_name(b"literal");
                                if let Some(language) = literal.language() {
                                    literal_tag.push_attribute(("xml:lang", language));
                                } else if !literal.is_plain() {
                                    literal_tag
                                        .push_attribute(("datatype", literal.datatype().as_str()));
                                }
                                writer.write_event(Event::Start(literal_tag))?;
                                writer.write_event(Event::Text(BytesText::from_plain_str(
                                    literal.value(),
                                )))?;
                                writer.write_event(Event::End(BytesEnd::borrowed(b"literal")))?;
                            }
                        }
                        writer.write_event(Event::End(BytesEnd::borrowed(b"binding")))?;
                    }
                }
                writer.write_event(Event::End(BytesEnd::borrowed(b"result")))?;
            }
            writer.write_event(Event::End(BytesEnd::borrowed(b"results")))?;
            writer.write_event(Event::End(BytesEnd::borrowed(b"sparql")))?;
        }
        QueryResult::Graph(_) => {
            return Err(anyhow!(
                "Graphs could not be formatted to SPARQL query results XML format"
            ));
        }
    }
    Ok(writer.into_inner())
}

pub fn read_xml_results<'a>(source: impl BufRead + 'a) -> Result<QueryResult<'a>> {
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
            let (ns, event) = reader.read_namespaced_event(&mut buffer, &mut namespace_buffer)?;
            if let Some(ns) = ns {
                if ns != b"http://www.w3.org/2005/sparql-results#".as_ref() {
                    return Err(anyhow!(
                        "Unexpected namespace found in RDF/XML query result: {}",
                        reader.decode(ns)?
                    ));
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
                        return Err(anyhow!("Expecting <sparql> tag, found {}", reader.decode(event.name())?));
                    }
                }
                State::Sparql => {
                    if event.name() == b"head" {
                        state = State::Head;
                    } else {
                        return Err(anyhow!("Expecting <head> tag, found {}", reader.decode(event.name())?));
                    }
                }
                State::Head => {
                    if event.name() == b"variable" {
                        let name = event.attributes()
                            .filter_map(|attr| attr.ok())
                            .find(|attr| attr.key == b"name")
                            .ok_or_else(|| anyhow!("No name attribute found for the <variable> tag"))?;
                        variables.push(name.unescape_and_decode_value(&reader)?);
                    } else if event.name() == b"link" {
                        // no op
                    } else {
                        return Err(anyhow!("Expecting <variable> or <link> tag, found {}", reader.decode(event.name())?));
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
                        return Ok(QueryResult::Bindings(BindingsIterator::new(
                            variables.into_iter().map(Variable::new).collect(),
                            Box::new(ResultsIterator {
                                reader,
                                buffer: Vec::default(),
                                namespace_buffer,
                                mapping,
                                bnodes_map: BTreeMap::default(),
                            }),
                        )));
                    } else if event.name() != b"link" && event.name() != b"results" && event.name() != b"boolean" {
                        return Err(anyhow!("Expecting sparql tag, found {}", reader.decode(event.name())?));
                    }
                }
                State::Boolean => return Err(anyhow!("Unexpected tag inside of <boolean> tag: {}", reader.decode(event.name())?))
            },
            Event::Empty(event) => match state {
                State::Sparql => {
                    if event.name() == b"head" {
                        state = State::AfterHead;
                    } else {
                        return Err(anyhow!("Expecting <head> tag, found {}", reader.decode(event.name())?));
                    }
                }
                State::Head => {
                    if event.name() == b"variable" {
                        let name = event.attributes()
                            .filter_map(|v| v.ok())
                            .find(|attr| attr.key == b"name")
                            .ok_or_else(|| anyhow!("No name attribute found for the <variable> tag"))?;
                        variables.push(name.unescape_and_decode_value(&reader)?);
                    } else if event.name() == b"link" {
                        // no op
                    } else {
                        return Err(anyhow!("Expecting <variable> or <link> tag, found {}", reader.decode(event.name())?));
                    }
                },
                State::AfterHead => {
                    if event.name() == b"results" {
                        return Ok(QueryResult::Bindings(BindingsIterator::new(
                            variables.into_iter().map(Variable::new).collect(),
                            Box::new(empty()),
                        )))
                    } else {
                        return Err(anyhow!("Unexpected autoclosing tag <{}>", reader.decode(event.name())?))
                    }
                }
                _ => return Err(anyhow!("Unexpected autoclosing tag <{}>", reader.decode(event.name())?))
            },
            Event::Text(event) => {
                let value = event.unescaped()?;
                return match state {
                    State::Boolean => {
                        return if value.as_ref() == b"true" {
                            Ok(QueryResult::Boolean(true))
                        } else if value.as_ref() == b"false" {
                            Ok(QueryResult::Boolean(false))
                        } else {
                            Err(anyhow!("Unexpected boolean value. Found {}", reader.decode(&value)?))
                        };
                    }
                    _ => Err(anyhow!("Unexpected textual value found: {}", reader.decode(&value)?))
                };
            },
            Event::End(_) => if let State::Head = state {
                state = State::AfterHead;
            } else {
                return Err(anyhow!("Unexpected early file end. All results file should have a <head> and a <result> or <boolean> tag"));
            },
            Event::Eof => return Err(anyhow!("Unexpected early file end. All results file should have a <head> and a <result> or <boolean> tag")),
            _ => (),
        }
    }
}

struct ResultsIterator<R: BufRead> {
    reader: Reader<R>,
    buffer: Vec<u8>,
    namespace_buffer: Vec<u8>,
    mapping: BTreeMap<Vec<u8>, usize>,
    bnodes_map: BTreeMap<Vec<u8>, BlankNode>,
}

impl<R: BufRead> Iterator for ResultsIterator<R> {
    type Item = Result<Vec<Option<Term>>>;

    fn next(&mut self) -> Option<Result<Vec<Option<Term>>>> {
        self.read_next().transpose()
    }
}

impl<R: BufRead> ResultsIterator<R> {
    fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>> {
        enum State {
            Start,
            Result,
            Binding,
            Uri,
            BNode,
            Literal,
            End,
        }
        let mut state = State::Start;

        let mut new_bindings = Vec::default();
        new_bindings.resize(self.mapping.len(), None);

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
                    return Err(anyhow!(
                        "Unexpected namespace found in RDF/XML query result: {}",
                        self.reader.decode(ns)?
                    ));
                }
            }
            match event {
                Event::Start(event) => match state {
                    State::Start => {
                        if event.name() == b"result" {
                            state = State::Result;
                        } else {
                            return Err(anyhow!(
                                "Expecting <result>, found {}",
                                self.reader.decode(event.name())?
                            ));
                        }
                    }
                    State::Result => {
                        if event.name() == b"binding" {
                            match event
                                .attributes()
                                .filter_map(|v| v.ok())
                                .find(|attr| attr.key == b"name")
                            {
                                Some(attr) => current_var = Some(attr.unescaped_value()?.to_vec()),
                                None => {
                                    return Err(anyhow!(
                                        "No name attribute found for the <binding> tag"
                                    ));
                                }
                            }
                            state = State::Binding;
                        } else {
                            return Err(anyhow!(
                                "Expecting <binding>, found {}",
                                self.reader.decode(event.name())?
                            ));
                        }
                    }
                    State::Binding => {
                        if term.is_some() {
                            return Err(anyhow!(
                                "There is already a value for the current binding"
                            ));
                        }
                        if event.name() == b"uri" {
                            state = State::Uri;
                        } else if event.name() == b"bnode" {
                            state = State::BNode;
                        } else if event.name() == b"literal" {
                            for attr in event.attributes() {
                                if let Ok(attr) = attr {
                                    if attr.key == b"xml:lang" {
                                        lang = Some(attr.unescape_and_decode_value(&self.reader)?);
                                    } else if attr.key == b"datatype" {
                                        datatype = Some(NamedNode::parse(
                                            attr.unescape_and_decode_value(&self.reader)?,
                                        )?);
                                    }
                                }
                            }
                            state = State::Literal;
                        } else {
                            return Err(anyhow!(
                                "Expecting <uri>, <bnode> or <literal> found {}",
                                self.reader.decode(event.name())?
                            ));
                        }
                    }
                    _ => (),
                },
                Event::Text(event) => {
                    let data = event.unescaped()?;
                    match state {
                        State::Uri => {
                            term = Some(NamedNode::parse(self.reader.decode(&data)?)?.into())
                        }
                        State::BNode => {
                            term = Some(
                                self.bnodes_map
                                    .entry(data.to_vec())
                                    .or_insert_with(BlankNode::default)
                                    .clone()
                                    .into(),
                            )
                        }
                        State::Literal => {
                            term = Some(
                                build_literal(
                                    self.reader.decode(&data)?,
                                    lang.take(),
                                    datatype.take(),
                                )
                                .into(),
                            );
                        }
                        _ => {
                            return Err(anyhow!(
                                "Unexpected textual value found: {}",
                                self.reader.decode(&data)?
                            ));
                        }
                    }
                }
                Event::End(_) => match state {
                    State::Start => state = State::End,
                    State::Result => return Ok(Some(new_bindings)),
                    State::Binding => {
                        if let Some(var) = &current_var {
                            new_bindings[self.mapping[var]] = term.clone()
                        } else {
                            return Err(anyhow!("No name found for <binding> tag"));
                        }
                        term = None;
                        state = State::Result;
                    }
                    State::Uri | State::BNode => state = State::Binding,
                    State::Literal => {
                        if term.is_none() {
                            //We default to the empty literal
                            term = Some(build_literal("", lang.take(), datatype.take()).into())
                        }
                        state = State::Binding;
                    }
                    _ => (),
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
) -> Literal {
    match datatype {
        Some(datatype) => Literal::new_typed_literal(value, datatype),
        None => match lang {
            Some(lang) => Literal::new_language_tagged_literal(value, lang),
            None => Literal::new_simple_literal(value),
        },
    }
}
