//! Implementation of [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)

use crate::error::{invalid_data_error, invalid_input_error};
use crate::model::*;
use crate::sparql::error::EvaluationError;
use crate::sparql::model::*;
use json_event_parser::{JsonEvent, JsonReader, JsonWriter};
use std::collections::BTreeMap;
use std::io;
use std::io::{BufRead, Write};
use std::rc::Rc;

pub fn write_json_results(results: QueryResults, sink: impl Write) -> Result<(), EvaluationError> {
    let mut writer = JsonWriter::from_writer(sink);
    match results {
        QueryResults::Boolean(value) => {
            writer.write_event(JsonEvent::StartObject)?;
            writer.write_event(JsonEvent::ObjectKey("head"))?;
            writer.write_event(JsonEvent::StartObject)?;
            writer.write_event(JsonEvent::EndObject)?;
            writer.write_event(JsonEvent::ObjectKey("boolean"))?;
            writer.write_event(JsonEvent::Boolean(value))?;
            writer.write_event(JsonEvent::EndObject)?;
            Ok(())
        }
        QueryResults::Solutions(solutions) => {
            writer.write_event(JsonEvent::StartObject)?;
            writer.write_event(JsonEvent::ObjectKey("head"))?;
            writer.write_event(JsonEvent::StartObject)?;
            writer.write_event(JsonEvent::ObjectKey("vars"))?;
            writer.write_event(JsonEvent::StartArray)?;
            for variable in solutions.variables() {
                writer.write_event(JsonEvent::String(variable.as_str()))?;
            }
            writer.write_event(JsonEvent::EndArray)?;
            writer.write_event(JsonEvent::EndObject)?;
            writer.write_event(JsonEvent::ObjectKey("results"))?;
            writer.write_event(JsonEvent::StartObject)?;
            writer.write_event(JsonEvent::ObjectKey("bindings"))?;
            writer.write_event(JsonEvent::StartArray)?;
            for solution in solutions {
                writer.write_event(JsonEvent::StartObject)?;

                let solution = solution?;
                for (variable, value) in solution.iter() {
                    writer.write_event(JsonEvent::ObjectKey(variable.as_str()))?;
                    write_json_term(value.as_ref(), &mut writer)?;
                }
                writer.write_event(JsonEvent::EndObject)?;
            }
            writer.write_event(JsonEvent::EndArray)?;
            writer.write_event(JsonEvent::EndObject)?;
            writer.write_event(JsonEvent::EndObject)?;
            Ok(())
        }
        QueryResults::Graph(_) => Err(invalid_input_error(
            "Graphs could not be formatted to SPARQL query results XML format",
        )
        .into()),
    }
}

fn write_json_term(
    term: TermRef<'_>,
    writer: &mut JsonWriter<impl Write>,
) -> Result<(), EvaluationError> {
    match term {
        TermRef::NamedNode(uri) => {
            writer.write_event(JsonEvent::StartObject)?;
            writer.write_event(JsonEvent::ObjectKey("type"))?;
            writer.write_event(JsonEvent::String("uri"))?;
            writer.write_event(JsonEvent::ObjectKey("value"))?;
            writer.write_event(JsonEvent::String(uri.as_str()))?;
            writer.write_event(JsonEvent::EndObject)?;
        }
        TermRef::BlankNode(bnode) => {
            writer.write_event(JsonEvent::StartObject)?;
            writer.write_event(JsonEvent::ObjectKey("type"))?;
            writer.write_event(JsonEvent::String("bnode"))?;
            writer.write_event(JsonEvent::ObjectKey("value"))?;
            writer.write_event(JsonEvent::String(bnode.as_str()))?;
            writer.write_event(JsonEvent::EndObject)?;
        }
        TermRef::Literal(literal) => {
            writer.write_event(JsonEvent::StartObject)?;
            writer.write_event(JsonEvent::ObjectKey("type"))?;
            writer.write_event(JsonEvent::String("literal"))?;
            writer.write_event(JsonEvent::ObjectKey("value"))?;
            writer.write_event(JsonEvent::String(literal.value()))?;
            if let Some(language) = literal.language() {
                writer.write_event(JsonEvent::ObjectKey("xml:lang"))?;
                writer.write_event(JsonEvent::String(language))?;
            } else if !literal.is_plain() {
                writer.write_event(JsonEvent::ObjectKey("datatype"))?;
                writer.write_event(JsonEvent::String(literal.datatype().as_str()))?;
            }
            writer.write_event(JsonEvent::EndObject)?;
        }
        TermRef::Triple(triple) => {
            writer.write_event(JsonEvent::StartObject)?;
            writer.write_event(JsonEvent::ObjectKey("type"))?;
            writer.write_event(JsonEvent::String("triple"))?;
            writer.write_event(JsonEvent::ObjectKey("value"))?;
            writer.write_event(JsonEvent::StartObject)?;
            writer.write_event(JsonEvent::ObjectKey("subject"))?;
            write_json_term(triple.subject.as_ref().into(), writer)?;
            writer.write_event(JsonEvent::ObjectKey("predicate"))?;
            write_json_term(triple.predicate.as_ref().into(), writer)?;
            writer.write_event(JsonEvent::ObjectKey("object"))?;
            write_json_term(triple.object.as_ref(), writer)?;
            writer.write_event(JsonEvent::EndObject)?;
            writer.write_event(JsonEvent::EndObject)?;
        }
    }
    Ok(())
}

pub fn read_json_results(source: impl BufRead + 'static) -> io::Result<QueryResults> {
    let mut reader = JsonReader::from_reader(source);
    let mut buffer = Vec::default();
    let mut variables = None;

    if reader.read_event(&mut buffer)? != JsonEvent::StartObject {
        return Err(invalid_data_error(
            "SPARQL JSON results should be an object",
        ));
    }

    loop {
        let event = reader.read_event(&mut buffer)?;
        match event {
            JsonEvent::ObjectKey(key) => match key {
                "head" => variables = Some(read_head(&mut reader, &mut buffer)?),
                "results" => {
                    if reader.read_event(&mut buffer)? != JsonEvent::StartObject {
                        return Err(invalid_data_error("'results' should be an object"));
                    }
                    if reader.read_event(&mut buffer)? != JsonEvent::ObjectKey("bindings") {
                        return Err(invalid_data_error(
                            "'results' should contain a 'bindings' key",
                        ));
                    }
                    if reader.read_event(&mut buffer)? != JsonEvent::StartArray {
                        return Err(invalid_data_error("'bindings' should be an object"));
                    }
                    return if let Some(variables) = variables {
                        let mut mapping = BTreeMap::default();
                        for (i, var) in variables.iter().enumerate() {
                            mapping.insert(var.clone(), i);
                        }
                        Ok(QueryResults::Solutions(QuerySolutionIter::new(
                            Rc::new(
                                variables
                                    .into_iter()
                                    .map(Variable::new)
                                    .collect::<Result<Vec<_>, _>>()
                                    .map_err(invalid_data_error)?,
                            ),
                            Box::new(ResultsIterator {
                                reader,
                                buffer,
                                mapping,
                            }),
                        )))
                    } else {
                        Err(invalid_data_error(
                            "SPARQL tuple query results should contain a head key",
                        ))
                    };
                }
                "boolean" => {
                    return if let JsonEvent::Boolean(v) = reader.read_event(&mut buffer)? {
                        Ok(QueryResults::Boolean(v))
                    } else {
                        Err(invalid_data_error("Unexpected boolean value"))
                    }
                }
                _ => {
                    return Err(invalid_data_error(format!(
                        "Expecting head or result key, found {}",
                        key
                    )));
                }
            },
            JsonEvent::EndObject => {
                return Err(invalid_data_error(
                    "SPARQL results should contain a bindings key or a boolean key",
                ))
            }
            JsonEvent::Eof => return Err(io::Error::from(io::ErrorKind::UnexpectedEof)),
            _ => return Err(invalid_data_error("Invalid SPARQL results serialization")),
        }
    }
}

fn read_head<R: BufRead>(
    reader: &mut JsonReader<R>,
    buffer: &mut Vec<u8>,
) -> io::Result<Vec<String>> {
    if reader.read_event(buffer)? != JsonEvent::StartObject {
        return Err(invalid_data_error("head should be an object"));
    }
    let mut variables = None;
    loop {
        match reader.read_event(buffer)? {
            JsonEvent::ObjectKey(key) => match key {
                "vars" => variables = Some(read_string_array(reader, buffer)?),
                "link" => {
                    read_string_array(reader, buffer)?;
                }
                _ => {
                    return Err(invalid_data_error(format!(
                        "Unexpected key in head: '{}'",
                        key
                    )))
                }
            },
            JsonEvent::EndObject => return Ok(variables.unwrap_or_else(Vec::new)),
            _ => return Err(invalid_data_error("Invalid head serialization")),
        }
    }
}

fn read_string_array<R: BufRead>(
    reader: &mut JsonReader<R>,
    buffer: &mut Vec<u8>,
) -> io::Result<Vec<String>> {
    if reader.read_event(buffer)? != JsonEvent::StartArray {
        return Err(invalid_data_error("Variable list should be an array"));
    }
    let mut elements = Vec::new();
    loop {
        match reader.read_event(buffer)? {
            JsonEvent::String(s) => {
                elements.push(s.into());
            }
            JsonEvent::EndArray => return Ok(elements),
            _ => return Err(invalid_data_error("Variable names should be strings")),
        }
    }
}

struct ResultsIterator<R: BufRead> {
    reader: JsonReader<R>,
    buffer: Vec<u8>,
    mapping: BTreeMap<String, usize>,
}

impl<R: BufRead> Iterator for ResultsIterator<R> {
    type Item = Result<Vec<Option<Term>>, EvaluationError>;

    fn next(&mut self) -> Option<Result<Vec<Option<Term>>, EvaluationError>> {
        self.read_next().map_err(EvaluationError::from).transpose()
    }
}

impl<R: BufRead> ResultsIterator<R> {
    fn read_next(&mut self) -> io::Result<Option<Vec<Option<Term>>>> {
        let mut new_bindings = vec![None; self.mapping.len()];
        loop {
            match self.reader.read_event(&mut self.buffer)? {
                JsonEvent::StartObject => (),
                JsonEvent::EndObject => return Ok(Some(new_bindings)),
                JsonEvent::EndArray | JsonEvent::Eof => return Ok(None),
                JsonEvent::ObjectKey(key) => {
                    let k = *self.mapping.get(key).ok_or_else(|| {
                        invalid_data_error(format!(
                            "The variable {} has not been defined in the header",
                            key
                        ))
                    })?;
                    new_bindings[k] = Some(self.read_value()?)
                }
                _ => return Err(invalid_data_error("Invalid result serialization")),
            }
        }
    }
    fn read_value(&mut self) -> io::Result<Term> {
        enum Type {
            Uri,
            BNode,
            Literal,
            Triple,
        }
        #[derive(Eq, PartialEq)]
        enum State {
            Type,
            Value,
            Lang,
            Datatype,
        }
        let mut state = None;
        let mut t = None;
        let mut value = None;
        let mut lang = None;
        let mut datatype = None;
        let mut subject = None;
        let mut predicate = None;
        let mut object = None;
        if self.reader.read_event(&mut self.buffer)? != JsonEvent::StartObject {
            return Err(invalid_data_error(
                "Term serializations should be an object",
            ));
        }
        loop {
            match self.reader.read_event(&mut self.buffer)? {
                JsonEvent::ObjectKey(key) => match key {
                    "type" => state = Some(State::Type),
                    "value" => state = Some(State::Value),
                    "xml:lang" => state = Some(State::Lang),
                    "datatype" => state = Some(State::Datatype),
                    "subject" => subject = Some(self.read_value()?),
                    "predicate" => predicate = Some(self.read_value()?),
                    "object" => object = Some(self.read_value()?),
                    _ => {
                        return Err(invalid_data_error(format!(
                            "Unexpected key in term serialization: '{}'",
                            key
                        )))
                    }
                },
                JsonEvent::StartObject => {
                    if state != Some(State::Value) {
                        return Err(invalid_data_error(
                            "Unexpected nested object in term serialization",
                        ));
                    }
                }
                JsonEvent::String(s) => match state {
                    Some(State::Type) => {
                        match s {
                            "uri" => t = Some(Type::Uri),
                            "bnode" => t = Some(Type::BNode),
                            "literal" => t = Some(Type::Literal),
                            "triple" => t = Some(Type::Triple),
                            _ => {
                                return Err(invalid_data_error(format!(
                                    "Unexpected term type: '{}'",
                                    s
                                )))
                            }
                        };
                        state = None;
                    }
                    Some(State::Value) => {
                        value = Some(s.to_owned());
                        state = None;
                    }
                    Some(State::Lang) => {
                        lang = Some(s.to_owned());
                        state = None;
                    }
                    Some(State::Datatype) => {
                        datatype = Some(s.to_owned());
                        state = None;
                    }
                    _ => (), // impossible
                },
                JsonEvent::EndObject => {
                    if let Some(s) = state {
                        if s == State::Value {
                            state = None; //End of triple
                        } else {
                            return Err(invalid_data_error(
                                "Term description values should be string",
                            ));
                        }
                    } else {
                        return match t {
                            None => Err(invalid_data_error(
                                "Term serialization should have a 'type' key",
                            )),
                            Some(Type::Uri) => Ok(NamedNode::new(value.ok_or_else(|| {
                                invalid_data_error("uri serialization should have a 'value' key")
                            })?)
                            .map_err(|e| invalid_data_error(format!("Invalid uri value: {}", e)))?
                            .into()),
                            Some(Type::BNode) => Ok(BlankNode::new(value.ok_or_else(|| {
                                invalid_data_error("bnode serialization should have a 'value' key")
                            })?)
                            .map_err(|e| invalid_data_error(format!("Invalid bnode value: {}", e)))?
                            .into()),
                            Some(Type::Literal) => {
                                let value = value.ok_or_else(|| {
                                    invalid_data_error(
                                        "uri serialization should have a 'value' key",
                                    )
                                })?;
                                Ok(match datatype {
                                    Some(datatype) => Literal::new_typed_literal(
                                        value,
                                        NamedNode::new(datatype).map_err(|e| {
                                            invalid_data_error(format!(
                                                "Invalid datatype value: {}",
                                                e
                                            ))
                                        })?,
                                    ),
                                    None => match lang {
                                        Some(lang) => {
                                            Literal::new_language_tagged_literal(value, lang)
                                                .map_err(|e| {
                                                    invalid_data_error(format!(
                                                        "Invalid xml:lang value: {}",
                                                        e
                                                    ))
                                                })?
                                        }
                                        None => Literal::new_simple_literal(value),
                                    },
                                }
                                .into())
                            }
                            Some(Type::Triple) => Ok(Triple::new(
                                match subject.ok_or_else(|| {
                                    invalid_data_error(
                                        "triple serialization should have a 'subject' key",
                                    )
                                })? {
                                    Term::NamedNode(subject) => subject.into(),
                                    Term::BlankNode(subject) => subject.into(),
                                    Term::Triple(subject) => Subject::Triple(subject),
                                    Term::Literal(_) => {
                                        return Err(invalid_data_error(
                                            "The 'subject' value should not be a literal",
                                        ))
                                    }
                                },
                                match predicate.ok_or_else(|| {
                                    invalid_data_error(
                                        "triple serialization should have a 'predicate' key",
                                    )
                                })? {
                                    Term::NamedNode(predicate) => predicate,
                                    _ => {
                                        return Err(invalid_data_error(
                                            "The 'predicate' value should be a uri",
                                        ))
                                    }
                                },
                                object.ok_or_else(|| {
                                    invalid_data_error(
                                        "triple serialization should have a 'object' key",
                                    )
                                })?,
                            )
                            .into()),
                        };
                    }
                }
                _ => return Err(invalid_data_error("Invalid term serialization")),
            }
        }
    }
}
