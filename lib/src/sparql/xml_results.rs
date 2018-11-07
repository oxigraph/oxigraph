use model::*;
use quick_xml::events::Event;
use quick_xml::Reader;
use sparql::algebra::BindingsIterator;
use sparql::algebra::QueryResult;
use sparql::algebra::Variable;
use std::collections::BTreeMap;
use std::io::BufRead;
use std::iter::empty;
use std::str::FromStr;
use Result;

pub fn read_xml_results(source: impl BufRead + 'static) -> Result<QueryResult<'static>> {
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
                    return Err(format_err!(
                        "Unexpected namespace found in RDF/XML query result: {}",
                        reader.decode(ns)
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
                        return Err(format_err!("Expecting <sparql> tag, found {}", reader.decode(event.name())));
                    }
                }
                State::Sparql => {
                    if event.name() == b"head" {
                        state = State::Head;
                    } else {
                        return Err(format_err!("Expecting <head> tag, found {}", reader.decode(event.name())));
                    }
                }
                State::Head => if event.name() == b"variable" || event.name() == b"link" {
                    return Err(format_err!("<variable> and <link> tag should be autoclosing"));
                } else {
                    return Err(format_err!("Expecting <variable> or <link> tag, found {}", reader.decode(event.name())));
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
                        return Err(format_err!("Expecting sparql tag, found {}", reader.decode(event.name())));
                    }
                }
                State::Boolean => return Err(format_err!("Unexpected tag inside of <boolean> tag: {}", reader.decode(event.name())))
            },
            Event::Empty(event) => match state {
                State::Head => {
                    if event.name() == b"variable" {
                        let name = event.attributes()
                            .filter_map(|attr| attr.ok())
                            .find(|attr| attr.key == b"name")
                            .ok_or_else(|| format_err!("No name attribute found for the <variable> tag"))?;
                        variables.push(name.unescape_and_decode_value(&reader)?);
                    } else if event.name() == b"link" {
                        // no op
                    } else {
                        return Err(format_err!("Expecting <variable> or <link> tag, found {}", reader.decode(event.name())));
                    }
                },
                State::AfterHead => {
                    if event.name() == b"results" {
                        return Ok(QueryResult::Bindings(BindingsIterator::new(
                            variables.into_iter().map(Variable::new).collect(),
                            Box::new(empty()),
                        )))
                    } else {
                        return Err(format_err!("Unexpected autoclosing tag <{}>", reader.decode(event.name())))
                    }
                }
                _ => return Err(format_err!("Unexpected autoclosing tag <{}>", reader.decode(event.name())))
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
                            Err(format_err!("Unexpected boolean value. Found {}", reader.decode(&value)))
                        };
                    }
                    _ => Err(format_err!("Unexpected textual value found: {}", reader.decode(&value)))
                };
            },
            Event::End(_) => if let State::Head = state {
                state = State::AfterHead;
            } else {
                    return Err(format_err!("Unexpected early file end. All results file should have a <head> and a <result> or <boolean> tag"));
            },
            Event::Eof => return Err(format_err!("Unexpected early file end. All results file should have a <head> and a <result> or <boolean> tag")),
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
            let (ns, event) = match self
                .reader
                .read_namespaced_event(&mut self.buffer, &mut self.namespace_buffer)
            {
                Ok(v) => v,
                Err(error) => return Some(Err(error.into())),
            };
            if let Some(ns) = ns {
                if ns != b"http://www.w3.org/2005/sparql-results#".as_ref() {
                    return Some(Err(format_err!(
                        "Unexpected namespace found in RDF/XML query result: {}",
                        self.reader.decode(ns)
                    )));
                }
            }
            match event {
                Event::Start(event) => match state {
                    State::Start => if event.name() == b"result" {
                        state = State::Result;
                    } else {
                        return Some(Err(format_err!(
                            "Expecting <result>, found {}",
                            self.reader.decode(event.name())
                        )));
                    },
                    State::Result => if event.name() == b"binding" {
                        match event
                            .attributes()
                            .filter_map(|attr| attr.ok())
                            .find(|attr| attr.key == b"name")
                        {
                            Some(attr) => match attr.unescaped_value() {
                                Ok(var) => current_var = Some(var.to_vec()),
                                Err(error) => return Some(Err(error.into())),
                            },
                            None => {
                                return Some(Err(format_err!(
                                    "No name attribute found for the <binding> tag"
                                )))
                            }
                        }
                        state = State::Binding;
                    } else {
                        return Some(Err(format_err!(
                            "Expecting <binding>, found {}",
                            self.reader.decode(event.name())
                        )));
                    },
                    State::Binding => {
                        if term.is_some() {
                            return Some(Err(format_err!(
                                "There is already a value for the current binding"
                            )));
                        }
                        if event.name() == b"uri" {
                            state = State::Uri;
                        } else if event.name() == b"bnode" {
                            state = State::BNode;
                        } else if event.name() == b"literal" {
                            for attr in event.attributes() {
                                if let Ok(attr) = attr {
                                    if attr.key == b"xml:lang" {
                                        match attr.unescape_and_decode_value(&self.reader) {
                                            Ok(val) => lang = Some(val),
                                            Err(error) => return Some(Err(error.into())),
                                        }
                                    } else if attr.key == b"datatype" {
                                        match attr.unescaped_value() {
                                            Ok(val) => {
                                                match NamedNode::from_str(&self.reader.decode(&val))
                                                {
                                                    Ok(dt) => datatype = Some(dt),
                                                    Err(error) => return Some(Err(error)),
                                                }
                                            }
                                            Err(error) => return Some(Err(error.into())),
                                        }
                                    }
                                }
                            }
                            state = State::Literal;
                        } else {
                            return Some(Err(format_err!(
                                "Expecting <uri>, <bnode> or <literal> found {}",
                                self.reader.decode(event.name())
                            )));
                        }
                    }
                    _ => (),
                },
                Event::Text(event) => match event.unescaped() {
                    Ok(data) => match state {
                        State::Uri => match NamedNode::from_str(&self.reader.decode(&data)) {
                            Ok(named_node) => term = Some(named_node.into()),
                            Err(error) => return Some(Err(error)),
                        },
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
                            let value = self.reader.decode(&data).to_string();
                            term = Some(build_literal(value, &lang, &datatype).into());
                        }
                        _ => {
                            return Some(Err(format_err!(
                                "Unexpected textual value found: {}",
                                self.reader.decode(&data)
                            )))
                        }
                    },
                    Err(error) => return Some(Err(error.into())),
                },
                Event::End(_) => match state {
                    State::Start => state = State::End,
                    State::Result => return Some(Ok(new_bindings)),
                    State::Binding => {
                        match (&current_var, &term) {
                            (Some(var), Some(term)) => {
                                new_bindings[self.mapping[var]] = Some(term.clone())
                            }
                            (Some(var), None) => {
                                return Some(Err(format_err!(
                                    "No variable found for variable {}",
                                    self.reader.decode(&var)
                                )))
                            }
                            _ => return Some(Err(format_err!("No name found for <binding> tag"))),
                        }
                        term = None;
                        state = State::Result;
                    }
                    State::Uri | State::BNode => state = State::Binding,
                    State::Literal => {
                        if term.is_none() {
                            //We default to the empty literal
                            term = Some(build_literal("", &lang, &datatype).into())
                        }
                        state = State::Binding;
                    }
                    _ => (),
                },
                Event::Eof => return None,
                _ => (),
            }
        }
    }
}

fn build_literal(
    value: impl Into<String>,
    lang: &Option<String>,
    datatype: &Option<NamedNode>,
) -> Literal {
    match datatype {
        Some(datatype) => Literal::new_typed_literal(value, datatype.clone()),
        None => match lang {
            Some(lang) => Literal::new_language_tagged_literal(value, lang.clone()),
            None => Literal::new_simple_literal(value),
        },
    }
}
