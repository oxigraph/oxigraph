//! Implementation of [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)

use crate::io::read::{ParserError, SyntaxError};
use crate::model::vocab::rdf;
use crate::model::*;
use crate::sparql::error::EvaluationError;
use json_event_parser::{JsonEvent, JsonReader, JsonWriter};
use oxrdf::Variable;
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};

pub fn write_boolean_json_result<W: Write>(sink: W, value: bool) -> io::Result<W> {
    let mut writer = JsonWriter::from_writer(sink);
    writer.write_event(JsonEvent::StartObject)?;
    writer.write_event(JsonEvent::ObjectKey("head"))?;
    writer.write_event(JsonEvent::StartObject)?;
    writer.write_event(JsonEvent::EndObject)?;
    writer.write_event(JsonEvent::ObjectKey("boolean"))?;
    writer.write_event(JsonEvent::Boolean(value))?;
    writer.write_event(JsonEvent::EndObject)?;
    Ok(writer.into_inner())
}

pub struct JsonSolutionsWriter<W: Write> {
    writer: JsonWriter<W>,
    variables: Vec<Variable>,
}

impl<W: Write> JsonSolutionsWriter<W> {
    pub fn start(sink: W, variables: &[Variable]) -> io::Result<Self> {
        let mut writer = JsonWriter::from_writer(sink);
        writer.write_event(JsonEvent::StartObject)?;
        writer.write_event(JsonEvent::ObjectKey("head"))?;
        writer.write_event(JsonEvent::StartObject)?;
        writer.write_event(JsonEvent::ObjectKey("vars"))?;
        writer.write_event(JsonEvent::StartArray)?;
        for variable in variables {
            writer.write_event(JsonEvent::String(variable.as_str()))?;
        }
        writer.write_event(JsonEvent::EndArray)?;
        writer.write_event(JsonEvent::EndObject)?;
        writer.write_event(JsonEvent::ObjectKey("results"))?;
        writer.write_event(JsonEvent::StartObject)?;
        writer.write_event(JsonEvent::ObjectKey("bindings"))?;
        writer.write_event(JsonEvent::StartArray)?;
        Ok(Self {
            writer,
            variables: variables.to_vec(),
        })
    }

    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = Option<TermRef<'a>>>,
    ) -> io::Result<()> {
        self.writer.write_event(JsonEvent::StartObject)?;
        for (value, variable) in solution.into_iter().zip(&self.variables) {
            if let Some(value) = value {
                self.writer
                    .write_event(JsonEvent::ObjectKey(variable.as_str()))?;
                write_json_term(value, &mut self.writer)?;
            }
        }
        self.writer.write_event(JsonEvent::EndObject)?;
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.writer.write_event(JsonEvent::EndArray)?;
        self.writer.write_event(JsonEvent::EndObject)?;
        self.writer.write_event(JsonEvent::EndObject)?;
        Ok(self.writer.into_inner())
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

pub enum JsonQueryResultsReader<R: BufRead> {
    Solutions {
        variables: Vec<Variable>,
        solutions: JsonSolutionsReader<R>,
    },
    Boolean(bool),
}

impl<R: BufRead> JsonQueryResultsReader<R> {
    pub fn read(source: R) -> Result<Self, ParserError> {
        let mut reader = JsonReader::from_reader(source);
        let mut buffer = Vec::default();
        let mut variables = None;

        if reader.read_event(&mut buffer)? != JsonEvent::StartObject {
            return Err(SyntaxError::msg("SPARQL JSON results should be an object").into());
        }

        loop {
            let event = reader.read_event(&mut buffer)?;
            match event {
                JsonEvent::ObjectKey(key) => match key {
                    "head" => variables = Some(read_head(&mut reader, &mut buffer)?),
                    "results" => {
                        if reader.read_event(&mut buffer)? != JsonEvent::StartObject {
                            return Err(SyntaxError::msg("'results' should be an object").into());
                        }
                        if reader.read_event(&mut buffer)? != JsonEvent::ObjectKey("bindings") {
                            return Err(SyntaxError::msg(
                                "'results' should contain a 'bindings' key",
                            )
                            .into());
                        }
                        if reader.read_event(&mut buffer)? != JsonEvent::StartArray {
                            return Err(SyntaxError::msg("'bindings' should be an object").into());
                        }
                        return if let Some(variables) = variables {
                            let mut mapping = BTreeMap::default();
                            for (i, var) in variables.iter().enumerate() {
                                mapping.insert(var.clone(), i);
                            }
                            Ok(Self::Solutions {
                                variables: variables
                                    .into_iter()
                                    .map(|v| {
                                        Variable::new(v).map_err(|e| {
                                            SyntaxError::msg(format!(
                                                "Invalid variable name: {}",
                                                e
                                            ))
                                        })
                                    })
                                    .collect::<Result<Vec<_>, _>>()?,
                                solutions: JsonSolutionsReader {
                                    reader,
                                    buffer,
                                    mapping,
                                },
                            })
                        } else {
                            Err(SyntaxError::msg(
                                "SPARQL tuple query results should contain a head key",
                            )
                            .into())
                        };
                    }
                    "boolean" => {
                        return if let JsonEvent::Boolean(v) = reader.read_event(&mut buffer)? {
                            Ok(Self::Boolean(v))
                        } else {
                            Err(SyntaxError::msg("Unexpected boolean value").into())
                        }
                    }
                    _ => {
                        return Err(SyntaxError::msg(format!(
                            "Expecting head or result key, found {}",
                            key
                        ))
                        .into());
                    }
                },
                JsonEvent::EndObject => {
                    return Err(SyntaxError::msg(
                        "SPARQL results should contain a bindings key or a boolean key",
                    )
                    .into())
                }
                JsonEvent::Eof => {
                    return Err(SyntaxError::msg(
                        "Unexpected end of JSON object without 'results' or 'boolean' key",
                    )
                    .into())
                }
                _ => return Err(SyntaxError::msg("Invalid SPARQL results serialization").into()),
            }
        }
    }
}

pub struct JsonSolutionsReader<R: BufRead> {
    reader: JsonReader<R>,
    buffer: Vec<u8>,
    mapping: BTreeMap<String, usize>,
}

impl<R: BufRead> JsonSolutionsReader<R> {
    pub fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, ParserError> {
        let mut new_bindings = vec![None; self.mapping.len()];
        loop {
            match self.reader.read_event(&mut self.buffer)? {
                JsonEvent::StartObject => (),
                JsonEvent::EndObject => return Ok(Some(new_bindings)),
                JsonEvent::EndArray | JsonEvent::Eof => return Ok(None),
                JsonEvent::ObjectKey(key) => {
                    let k = *self.mapping.get(key).ok_or_else(|| {
                        SyntaxError::msg(format!(
                            "The variable {} has not been defined in the header",
                            key
                        ))
                    })?;
                    new_bindings[k] = Some(self.read_value()?)
                }
                _ => return Err(SyntaxError::msg("Invalid result serialization").into()),
            }
        }
    }

    fn read_value(&mut self) -> Result<Term, ParserError> {
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
            return Err(SyntaxError::msg("Term serializations should be an object").into());
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
                        return Err(SyntaxError::msg(format!(
                            "Unexpected key in term serialization: '{}'",
                            key
                        ))
                        .into())
                    }
                },
                JsonEvent::StartObject => {
                    if state != Some(State::Value) {
                        return Err(SyntaxError::msg(
                            "Unexpected nested object in term serialization",
                        )
                        .into());
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
                                return Err(SyntaxError::msg(format!(
                                    "Unexpected term type: '{}'",
                                    s
                                ))
                                .into())
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
                        datatype = Some(NamedNode::new(s).map_err(|e| {
                            SyntaxError::msg(format!("Invalid datatype IRI: {}", e))
                        })?);
                        state = None;
                    }
                    _ => (), // impossible
                },
                JsonEvent::EndObject => {
                    if let Some(s) = state {
                        if s == State::Value {
                            state = None; //End of triple
                        } else {
                            return Err(SyntaxError::msg(
                                "Term description values should be string",
                            )
                            .into());
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
                            .map_err(|e| SyntaxError::msg(format!("Invalid uri value: {}", e)))?
                            .into()),
                            Some(Type::BNode) => Ok(BlankNode::new(value.ok_or_else(|| {
                                SyntaxError::msg("bnode serialization should have a 'value' key")
                            })?)
                            .map_err(|e| SyntaxError::msg(format!("Invalid bnode value: {}", e)))?
                            .into()),
                            Some(Type::Literal) => {
                                let value = value.ok_or_else(|| {
                                    SyntaxError::msg(
                                        "literal serialization should have a 'value' key",
                                    )
                                })?;
                                Ok(match lang {
                                        Some(lang) => {
                                            if let Some(datatype) = datatype {
                                                if datatype.as_ref() != rdf::LANG_STRING {
                                                    return Err(SyntaxError::msg(format!(
                                                        "xml:lang value '{}' provided with the datatype {}",
                                                        lang, datatype
                                                    )).into())
                                                }
                                            }
                                            Literal::new_language_tagged_literal(value, &lang).map_err(|e| {
                                                SyntaxError::msg(format!("Invalid xml:lang value '{}': {}", lang, e))
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
                            Some(Type::Triple) => Ok(Triple::new(
                                match subject.ok_or_else(|| {
                                    SyntaxError::msg(
                                        "triple serialization should have a 'subject' key",
                                    )
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
                                    SyntaxError::msg(
                                        "triple serialization should have a 'object' key",
                                    )
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
}

fn read_head<R: BufRead>(
    reader: &mut JsonReader<R>,
    buffer: &mut Vec<u8>,
) -> Result<Vec<String>, ParserError> {
    if reader.read_event(buffer)? != JsonEvent::StartObject {
        return Err(SyntaxError::msg("head should be an object").into());
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
                    return Err(
                        SyntaxError::msg(format!("Unexpected key in head: '{}'", key)).into(),
                    )
                }
            },
            JsonEvent::EndObject => return Ok(variables.unwrap_or_else(Vec::new)),
            _ => return Err(SyntaxError::msg("Invalid head serialization").into()),
        }
    }
}

fn read_string_array<R: BufRead>(
    reader: &mut JsonReader<R>,
    buffer: &mut Vec<u8>,
) -> Result<Vec<String>, ParserError> {
    if reader.read_event(buffer)? != JsonEvent::StartArray {
        return Err(SyntaxError::msg("Variable list should be an array").into());
    }
    let mut elements = Vec::new();
    loop {
        match reader.read_event(buffer)? {
            JsonEvent::String(s) => {
                elements.push(s.into());
            }
            JsonEvent::EndArray => return Ok(elements),
            _ => return Err(SyntaxError::msg("Variable names should be strings").into()),
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
    fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, ParserError> {
        let mut new_bindings = vec![None; self.mapping.len()];
        loop {
            match self.reader.read_event(&mut self.buffer)? {
                JsonEvent::StartObject => (),
                JsonEvent::EndObject => return Ok(Some(new_bindings)),
                JsonEvent::EndArray | JsonEvent::Eof => return Ok(None),
                JsonEvent::ObjectKey(key) => {
                    let k = *self.mapping.get(key).ok_or_else(|| {
                        SyntaxError::msg(format!(
                            "The variable {} has not been defined in the header",
                            key
                        ))
                    })?;
                    new_bindings[k] = Some(self.read_value()?)
                }
                _ => return Err(SyntaxError::msg("Invalid result serialization").into()),
            }
        }
    }
    fn read_value(&mut self) -> Result<Term, ParserError> {
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
            return Err(SyntaxError::msg("Term serializations should be an object").into());
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
                        return Err(SyntaxError::msg(format!(
                            "Unexpected key in term serialization: '{}'",
                            key
                        ))
                        .into())
                    }
                },
                JsonEvent::StartObject => {
                    if state != Some(State::Value) {
                        return Err(SyntaxError::msg(
                            "Unexpected nested object in term serialization",
                        )
                        .into());
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
                                return Err(SyntaxError::msg(format!(
                                    "Unexpected term type: '{}'",
                                    s
                                ))
                                .into())
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
                        datatype = Some(NamedNode::new(s).map_err(|e| {
                            SyntaxError::msg(format!("Invalid datatype value: {}", e))
                        })?);
                        state = None;
                    }
                    _ => (), // impossible
                },
                JsonEvent::EndObject => {
                    if let Some(s) = state {
                        if s == State::Value {
                            state = None; //End of triple
                        } else {
                            return Err(SyntaxError::msg(
                                "Term description values should be string",
                            )
                            .into());
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
                            .map_err(|e| SyntaxError::msg(format!("Invalid uri value: {}", e)))?
                            .into()),
                            Some(Type::BNode) => Ok(BlankNode::new(value.ok_or_else(|| {
                                SyntaxError::msg("bnode serialization should have a 'value' key")
                            })?)
                            .map_err(|e| SyntaxError::msg(format!("Invalid bnode value: {}", e)))?
                            .into()),
                            Some(Type::Literal) => {
                                let value = value.ok_or_else(|| {
                                    SyntaxError::msg(
                                        "literal serialization should have a 'value' key",
                                    )
                                })?;
                                Ok(match lang {
                                    Some(lang) => {
                                        if let Some(datatype) = datatype {
                                            if datatype.as_ref() != rdf::LANG_STRING {
                                                return Err(SyntaxError::msg(format!(
                                                    "xml:lang value '{}' provided with the datatype {}",
                                                    lang, datatype
                                                )).into())
                                            }
                                        }
                                        Literal::new_language_tagged_literal(value, &lang).map_err(|e| {
                                            SyntaxError::msg(format!("Invalid xml:lang value '{}': {}", lang, e))
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
                            Some(Type::Triple) => Ok(Triple::new(
                                match subject.ok_or_else(|| {
                                    SyntaxError::msg(
                                        "triple serialization should have a 'subject' key",
                                    )
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
                                    SyntaxError::msg(
                                        "triple serialization should have a 'object' key",
                                    )
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
}
