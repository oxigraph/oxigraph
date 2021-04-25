//! Implementation of [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)

use crate::error::invalid_input_error;
use crate::model::*;
use crate::sparql::error::EvaluationError;
use crate::sparql::model::*;
use std::io::Write;

pub fn write_json_results(
    results: QueryResults,
    mut sink: impl Write,
) -> Result<(), EvaluationError> {
    match results {
        QueryResults::Boolean(value) => {
            sink.write_all(b"{\"head\":{},\"boolean\":")?;
            sink.write_all(if value { b"true" } else { b"false" })?;
            sink.write_all(b"}")?;
            Ok(())
        }
        QueryResults::Solutions(solutions) => {
            sink.write_all(b"{\"head\":{\"vars\":[")?;
            let mut start_vars = true;
            for variable in solutions.variables() {
                if start_vars {
                    start_vars = false;
                } else {
                    sink.write_all(b",")?;
                }
                write_escaped_json_string(variable.as_str(), &mut sink)?;
            }
            sink.write_all(b"]},\"results\":{\"bindings\":[")?;
            let mut start_bindings = true;
            for solution in solutions {
                if start_bindings {
                    start_bindings = false;
                } else {
                    sink.write_all(b",")?;
                }
                sink.write_all(b"{")?;

                let solution = solution?;
                let mut start_binding = true;
                for (variable, value) in solution.iter() {
                    if start_binding {
                        start_binding = false;
                    } else {
                        sink.write_all(b",")?;
                    }
                    write_escaped_json_string(variable.as_str(), &mut sink)?;
                    sink.write_all(b":")?;
                    write_json_term(value.as_ref(), &mut sink)?;
                }
                sink.write_all(b"}")?;
            }
            sink.write_all(b"]}}")?;
            Ok(())
        }
        QueryResults::Graph(_) => Err(invalid_input_error(
            "Graphs could not be formatted to SPARQL query results XML format",
        )
        .into()),
    }
}

fn write_json_term(term: TermRef<'_>, sink: &mut impl Write) -> Result<(), EvaluationError> {
    match term {
        TermRef::NamedNode(uri) => {
            sink.write_all(b"{\"type\":\"uri\",\"value\":")?;
            write_escaped_json_string(uri.as_str(), sink)?;
            sink.write_all(b"}")?;
        }
        TermRef::BlankNode(bnode) => {
            sink.write_all(b"{\"type\":\"bnode\",\"value\":")?;
            write_escaped_json_string(bnode.as_str(), sink)?;
            sink.write_all(b"}")?;
        }
        TermRef::Literal(literal) => {
            sink.write_all(b"{\"type\":\"literal\",\"value\":")?;
            write_escaped_json_string(literal.value(), sink)?;
            if let Some(language) = literal.language() {
                sink.write_all(b",\"xml:lang\":")?;
                write_escaped_json_string(language, sink)?;
            } else if !literal.is_plain() {
                sink.write_all(b",\"datatype\":")?;
                write_escaped_json_string(literal.datatype().as_str(), sink)?;
            }
            sink.write_all(b"}")?;
        }
        TermRef::Triple(triple) => {
            sink.write_all(b":{\"type\":\"triple\",\"value\":{\"subject\":")?;
            write_json_term(triple.subject.as_ref().into(), sink)?;
            sink.write_all(b":,\"predicate\":")?;
            write_json_term(triple.predicate.as_ref().into(), sink)?;
            sink.write_all(b":,\"object\":")?;
            write_json_term(triple.object.as_ref(), sink)?;
            sink.write_all(b"}}")?;
        }
    }
    Ok(())
}

fn write_escaped_json_string(s: &str, sink: &mut impl Write) -> Result<(), EvaluationError> {
    sink.write_all(b"\"")?;
    for c in s.chars() {
        match c {
            '\\' => sink.write_all(b"\\\\"),
            '"' => sink.write_all(b"\\\""),
            c => {
                if c < char::from(32) {
                    match c {
                        '\u{08}' => sink.write_all(b"\\b"),
                        '\u{0C}' => sink.write_all(b"\\f"),
                        '\n' => sink.write_all(b"\\n"),
                        '\r' => sink.write_all(b"\\r"),
                        '\t' => sink.write_all(b"\\t"),
                        c => {
                            let mut c = c as u8;
                            let mut result = [b'\\', b'u', 0, 0, 0, 0];
                            for i in (2..6).rev() {
                                let ch = c % 16;
                                result[i] = ch + if ch < 10 { b'0' } else { b'A' };
                                c /= 16;
                            }
                            sink.write_all(&result)
                        }
                    }
                } else {
                    write!(sink, "{}", c)
                }
            }
        }?;
    }
    sink.write_all(b"\"")?;
    Ok(())
}
