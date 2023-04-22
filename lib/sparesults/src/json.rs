//! Implementation of [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)

use crate::error::{ParseError, SyntaxError};
use json_event_parser::{JsonEvent, JsonReader, JsonWriter};
use oxrdf::vocab::rdf;
use oxrdf::Variable;
use oxrdf::*;
use std::collections::BTreeMap;
use std::io::{self, BufRead, Write};
use std::mem::take;

/// This limit is set in order to avoid stack overflow error when parsing nested triples due to too many recursive calls.
/// The actual limit value is a wet finger compromise between not failing to parse valid files and avoiding to trigger stack overflow errors.
const MAX_NUMBER_OF_NESTED_TRIPLES: usize = 128;

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
        Ok(Self { writer })
    }

    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> io::Result<()> {
        self.writer.write_event(JsonEvent::StartObject)?;
        for (variable, value) in solution {
            self.writer
                .write_event(JsonEvent::ObjectKey(variable.as_str()))?;
            write_json_term(value, &mut self.writer)?;
        }
        self.writer.write_event(JsonEvent::EndObject)?;
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.writer.write_event(JsonEvent::EndArray)?;
        self.writer.write_event(JsonEvent::EndObject)?;
        self.writer.write_event(JsonEvent::EndObject)?;
        let mut inner = self.writer.into_inner();
        inner.flush()?;
        Ok(inner)
    }
}

fn write_json_term(term: TermRef<'_>, writer: &mut JsonWriter<impl Write>) -> io::Result<()> {
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
        #[cfg(feature = "rdf-star")]
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
    pub fn read(source: R) -> Result<Self, ParseError> {
        let mut reader = JsonReader::from_reader(source);
        let mut buffer = Vec::default();
        let mut variables = None;
        let mut buffered_bindings: Option<Vec<_>> = None;
        let mut output_iter = None;

        if reader.read_event(&mut buffer)? != JsonEvent::StartObject {
            return Err(SyntaxError::msg("SPARQL JSON results should be an object").into());
        }

        loop {
            let event = reader.read_event(&mut buffer)?;
            match event {
                JsonEvent::ObjectKey(key) => match key {
                    "head" => {
                        let extracted_variables = read_head(&mut reader, &mut buffer)?;
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
                        if reader.read_event(&mut buffer)? != JsonEvent::StartObject {
                            return Err(SyntaxError::msg("'results' should be an object").into());
                        }
                        loop {
                            match reader.read_event(&mut buffer)? {
                                JsonEvent::ObjectKey("bindings") => break, // Found
                                JsonEvent::ObjectKey(_) => ignore_value(&mut reader, &mut buffer)?,
                                _ => {
                                    return Err(SyntaxError::msg(
                                        "'results' should contain a 'bindings' key",
                                    )
                                    .into())
                                }
                            }
                        }
                        if reader.read_event(&mut buffer)? != JsonEvent::StartArray {
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
                                    kind: JsonSolutionsReaderKind::Streaming { reader, buffer },
                                    mapping,
                                },
                            });
                        }
                        // We buffer all results before being able to read the header
                        let mut bindings = Vec::new();
                        let mut variables = Vec::new();
                        let mut values = Vec::new();
                        loop {
                            match reader.read_event(&mut buffer)? {
                                JsonEvent::StartObject => (),
                                JsonEvent::EndObject => {
                                    bindings.push((take(&mut variables), take(&mut values)));
                                }
                                JsonEvent::EndArray | JsonEvent::Eof => {
                                    buffered_bindings = Some(bindings);
                                    break;
                                }
                                JsonEvent::ObjectKey(key) => {
                                    variables.push(key.to_owned());
                                    values.push(read_value(&mut reader, &mut buffer, 0)?);
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
                        return if let JsonEvent::Boolean(v) = reader.read_event(&mut buffer)? {
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

pub struct JsonSolutionsReader<R: BufRead> {
    mapping: BTreeMap<String, usize>,
    kind: JsonSolutionsReaderKind<R>,
}

enum JsonSolutionsReaderKind<R: BufRead> {
    Streaming {
        reader: JsonReader<R>,
        buffer: Vec<u8>,
    },
    Buffered {
        bindings: std::vec::IntoIter<(Vec<String>, Vec<Term>)>,
    },
}

impl<R: BufRead> JsonSolutionsReader<R> {
    pub fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, ParseError> {
        match &mut self.kind {
            JsonSolutionsReaderKind::Streaming { reader, buffer } => {
                let mut new_bindings = vec![None; self.mapping.len()];
                loop {
                    match reader.read_event(buffer)? {
                        JsonEvent::StartObject => (),
                        JsonEvent::EndObject => return Ok(Some(new_bindings)),
                        JsonEvent::EndArray | JsonEvent::Eof => return Ok(None),
                        JsonEvent::ObjectKey(key) => {
                            let k = *self.mapping.get(key).ok_or_else(|| {
                                SyntaxError::msg(format!(
                                    "The variable {key} has not been defined in the header"
                                ))
                            })?;
                            new_bindings[k] = Some(read_value(reader, buffer, 0)?)
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

fn read_value<R: BufRead>(
    reader: &mut JsonReader<R>,
    buffer: &mut Vec<u8>,
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
    if reader.read_event(buffer)? != JsonEvent::StartObject {
        return Err(SyntaxError::msg("Term serializations should be an object").into());
    }
    loop {
        match reader.read_event(buffer)? {
            JsonEvent::ObjectKey(key) => match key {
                "type" => state = Some(State::Type),
                "value" => state = Some(State::Value),
                "xml:lang" => state = Some(State::Lang),
                "datatype" => state = Some(State::Datatype),
                #[cfg(feature = "rdf-star")]
                "subject" => {
                    subject = Some(read_value(reader, buffer, number_of_recursive_calls + 1)?)
                }
                #[cfg(feature = "rdf-star")]
                "predicate" => {
                    predicate = Some(read_value(reader, buffer, number_of_recursive_calls + 1)?)
                }
                #[cfg(feature = "rdf-star")]
                "object" => {
                    object = Some(read_value(reader, buffer, number_of_recursive_calls + 1)?)
                }
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
                    match s {
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
                    value = Some(s.to_owned());
                    state = None;
                }
                Some(State::Lang) => {
                    lang = Some(s.to_owned());
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
                                    Literal::new_language_tagged_literal(value, &lang).map_err(|e| {
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

fn read_head<R: BufRead>(
    reader: &mut JsonReader<R>,
    buffer: &mut Vec<u8>,
) -> Result<Vec<Variable>, ParseError> {
    if reader.read_event(buffer)? != JsonEvent::StartObject {
        return Err(SyntaxError::msg("head should be an object").into());
    }
    let mut variables = Vec::new();
    loop {
        match reader.read_event(buffer)? {
            JsonEvent::ObjectKey(key) => match key {
                "vars" => {
                    if reader.read_event(buffer)? != JsonEvent::StartArray {
                        return Err(SyntaxError::msg("Variable list should be an array").into());
                    }
                    loop {
                        match reader.read_event(buffer)? {
                            JsonEvent::String(s) => {
                                let new_var = Variable::new(s).map_err(|e| {
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
                    if reader.read_event(buffer)? != JsonEvent::StartArray {
                        return Err(SyntaxError::msg("Variable list should be an array").into());
                    }
                    loop {
                        match reader.read_event(buffer)? {
                            JsonEvent::String(_) => (),
                            JsonEvent::EndArray => break,
                            _ => {
                                return Err(SyntaxError::msg("Link names should be strings").into())
                            }
                        }
                    }
                }
                _ => ignore_value(reader, buffer)?,
            },
            JsonEvent::EndObject => return Ok(variables),
            _ => return Err(SyntaxError::msg("Invalid head serialization").into()),
        }
    }
}

fn ignore_value<R: BufRead>(
    reader: &mut JsonReader<R>,
    buffer: &mut Vec<u8>,
) -> Result<(), ParseError> {
    let mut nesting = 0;
    loop {
        match reader.read_event(buffer)? {
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
