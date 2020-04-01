//! Implementation of [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)

use crate::model::*;
use crate::sparql::model::*;
use crate::Error;
use crate::Result;
use std::io::Write;

pub fn write_json_results<W: Write>(results: QueryResult<'_>, mut sink: W) -> Result<W> {
    match results {
        QueryResult::Boolean(value) => {
            sink.write_all(b"{\"head\":{},\"boolean\":")?;
            sink.write_all(if value { b"true" } else { b"false" })?;
            sink.write_all(b"}")?;
        }
        QueryResult::Solutions(solutions) => {
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
                    match value {
                        Term::NamedNode(uri) => {
                            sink.write_all(b":{\"type\":\"uri\",\"value\":")?;
                            write_escaped_json_string(uri.as_str(), &mut sink)?;
                            sink.write_all(b"}")?;
                        }
                        Term::BlankNode(bnode) => {
                            sink.write_all(b":{\"type\":\"bnode\",\"value\":")?;
                            write_escaped_json_string(bnode.as_str(), &mut sink)?;
                            sink.write_all(b"}")?;
                        }
                        Term::Literal(literal) => {
                            sink.write_all(b":{\"type\":\"literal\",\"value\":")?;
                            write_escaped_json_string(literal.value(), &mut sink)?;
                            if let Some(language) = literal.language() {
                                sink.write_all(b",\"xml:lang\":")?;
                                write_escaped_json_string(language, &mut sink)?;
                            } else if !literal.is_plain() {
                                sink.write_all(b",\"datatype\":")?;
                                write_escaped_json_string(literal.datatype().as_str(), &mut sink)?;
                            }
                            sink.write_all(b"}")?;
                        }
                    }
                }
                sink.write_all(b"}")?;
            }
            sink.write_all(b"]}}")?;
        }
        QueryResult::Graph(_) => {
            return Err(Error::msg(
                "Graphs could not be formatted to SPARQL query results XML format",
            ));
        }
    }
    Ok(sink)
}

fn write_escaped_json_string(s: &str, sink: &mut impl Write) -> Result<()> {
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
