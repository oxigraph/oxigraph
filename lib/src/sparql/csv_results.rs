//! Implementation of [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/)

use crate::error::invalid_input_error;
use crate::model::{vocab::xsd, *};
use crate::sparql::error::EvaluationError;
use crate::sparql::model::*;
use std::io::{self, Write};

pub fn write_csv_results(
    results: QueryResults,
    mut sink: impl Write,
) -> Result<(), EvaluationError> {
    match results {
        QueryResults::Boolean(_) => Err(invalid_input_error(
            "boolean could not be formatted to SPARQL query results CSV format",
        )
        .into()),
        QueryResults::Solutions(solutions) => {
            let mut start_vars = true;
            for variable in solutions.variables() {
                if start_vars {
                    start_vars = false;
                } else {
                    sink.write_all(b",")?;
                }
                sink.write_all(variable.as_str().as_bytes())?;
            }

            for solution in solutions {
                let solution = solution?;
                sink.write_all(b"\r\n")?;
                let mut start_binding = true;
                for value in solution.values() {
                    if start_binding {
                        start_binding = false;
                    } else {
                        sink.write_all(b",")?;
                    }
                    if let Some(value) = value {
                        match value {
                            Term::NamedNode(uri) => {
                                sink.write_all(uri.as_str().as_bytes())?;
                            }
                            Term::BlankNode(bnode) => {
                                sink.write_all(b"_:")?;
                                sink.write_all(bnode.as_str().as_bytes())?;
                            }
                            Term::Literal(literal) => {
                                write_escaped_csv_string(literal.value(), &mut sink)?;
                            }
                        }
                    }
                }
            }
            Ok(())
        }
        QueryResults::Graph(_) => Err(invalid_input_error(
            "Graphs could not be formatted to SPARQL query results CSV format",
        )
        .into()),
    }
}

fn write_escaped_csv_string(s: &str, mut sink: impl Write) -> Result<(), io::Error> {
    if s.bytes().any(|c| match c {
        b'"' | b',' | b'\n' | b'\r' => true,
        _ => false,
    }) {
        sink.write_all(b"\"")?;
        for c in s.bytes() {
            if c == b'\"' {
                sink.write_all(b"\"\"")
            } else {
                sink.write_all(&[c])
            }?;
        }
        sink.write_all(b"\"")
    } else {
        sink.write_all(s.as_bytes())
    }
}

pub fn write_tsv_results(
    results: QueryResults,
    mut sink: impl Write,
) -> Result<(), EvaluationError> {
    match results {
        QueryResults::Boolean(_) => Err(invalid_input_error(
            "boolean could not be formatted to SPARQL query results TSV format",
        )
        .into()),
        QueryResults::Solutions(solutions) => {
            let mut start_vars = true;
            for variable in solutions.variables() {
                if start_vars {
                    start_vars = false;
                } else {
                    sink.write_all(b"\t")?;
                }
                sink.write_all(b"?")?;
                sink.write_all(variable.as_str().as_bytes())?;
            }

            for solution in solutions {
                let solution = solution?;
                sink.write_all(b"\n")?;
                let mut start_binding = true;
                for value in solution.values() {
                    if start_binding {
                        start_binding = false;
                    } else {
                        sink.write_all(b"\t")?;
                    }
                    if let Some(value) = value {
                        //TODO: full Turtle serialization
                        sink.write_all(
                            match value {
                                Term::NamedNode(node) => node.to_string(),
                                Term::BlankNode(node) => node.to_string(),
                                Term::Literal(literal) => match literal.datatype() {
                                    xsd::BOOLEAN => match literal.value() {
                                        "true" | "1" => "true".to_owned(),
                                        "false" | "0" => "false".to_owned(),
                                        _ => literal.to_string(),
                                    },
                                    xsd::INTEGER => {
                                        if literal.value().bytes().all(|c| match c {
                                            b'0'..=b'9' => true,
                                            _ => false,
                                        }) {
                                            literal.value().to_owned()
                                        } else {
                                            literal.to_string()
                                        }
                                    }
                                    _ => literal.to_string(),
                                },
                            }
                            .as_bytes(),
                        )?;
                    }
                }
            }
            Ok(())
        }
        QueryResults::Graph(_) => Err(invalid_input_error(
            "Graphs could not be formatted to SPARQL query results TSV format",
        )
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::str;

    fn build_example() -> QueryResults {
        QuerySolutionIter::new(
            Rc::new(vec![Variable::new("x"), Variable::new("literal")]),
            Box::new(
                vec![
                    Ok(vec![
                        Some(NamedNode::new_unchecked("http://example/x").into()),
                        Some(Literal::new_simple_literal("String").into()),
                    ]),
                    Ok(vec![
                        Some(NamedNode::new_unchecked("http://example/x").into()),
                        Some(Literal::new_simple_literal("String-with-dquote\"").into()),
                    ]),
                    Ok(vec![
                        Some(BlankNode::new_unchecked("b0").into()),
                        Some(Literal::new_simple_literal("Blank node").into()),
                    ]),
                    Ok(vec![
                        None,
                        Some(Literal::new_simple_literal("Missing 'x'").into()),
                    ]),
                    Ok(vec![None, None]),
                    Ok(vec![
                        Some(NamedNode::new_unchecked("http://example/x").into()),
                        None,
                    ]),
                    Ok(vec![
                        Some(BlankNode::new_unchecked("b1").into()),
                        Some(
                            Literal::new_language_tagged_literal_unchecked(
                                "String-with-lang",
                                "en",
                            )
                            .into(),
                        ),
                    ]),
                    Ok(vec![
                        Some(BlankNode::new_unchecked("b1").into()),
                        Some(Literal::new_typed_literal("123", xsd::INTEGER).into()),
                    ]),
                ]
                .into_iter(),
            ),
        )
        .into()
    }

    #[test]
    fn test_csv_serialization() {
        let mut sink = Vec::new();
        write_csv_results(build_example(), &mut sink).unwrap();
        assert_eq!(str::from_utf8(&sink).unwrap(), "x,literal\r\nhttp://example/x,String\r\nhttp://example/x,\"String-with-dquote\"\"\"\r\n_:b0,Blank node\r\n,Missing 'x'\r\n,\r\nhttp://example/x,\r\n_:b1,String-with-lang\r\n_:b1,123");
    }

    #[test]
    fn test_tsv_serialization() {
        let mut sink = Vec::new();
        write_tsv_results(build_example(), &mut sink).unwrap();
        assert_eq!(str::from_utf8(&sink).unwrap(), "?x\t?literal\n<http://example/x>\t\"String\"\n<http://example/x>\t\"String-with-dquote\\\"\"\n_:b0\t\"Blank node\"\n\t\"Missing 'x'\"\n\t\n<http://example/x>\t\n_:b1\t\"String-with-lang\"@en\n_:b1\t123");
    }
}
