//! Implementation of [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/)

use crate::error::invalid_data_error;
use crate::model::{vocab::xsd, *};
use crate::sparql::error::EvaluationError;
use crate::sparql::model::*;
use std::io::{self, BufRead, Write};
use std::rc::Rc;
use std::str::FromStr;

pub fn write_csv_results(
    results: QueryResults,
    mut sink: impl Write,
) -> Result<(), EvaluationError> {
    match results {
        QueryResults::Boolean(value) => {
            sink.write_all(if value { b"true" } else { b"false" })?;
        }
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
                        write_csv_term(value, &mut sink)?;
                    }
                }
            }
        }
        QueryResults::Graph(g) => {
            sink.write_all(b"subject,predicate,object")?;
            for t in g {
                let t = t?;
                sink.write_all(b"\r\n")?;
                write_csv_term(&t.subject, &mut sink)?;
                sink.write_all(b",")?;
                write_csv_term(&t.predicate, &mut sink)?;
                sink.write_all(b",")?;
                write_csv_term(&t.object, &mut sink)?;
            }
        }
    }
    Ok(())
}

fn write_csv_term<'a>(term: impl Into<TermRef<'a>>, mut sink: impl Write) -> io::Result<()> {
    match term.into() {
        TermRef::NamedNode(uri) => sink.write_all(uri.as_str().as_bytes()),
        TermRef::BlankNode(bnode) => {
            sink.write_all(b"_:")?;
            sink.write_all(bnode.as_str().as_bytes())
        }
        TermRef::Literal(literal) => write_escaped_csv_string(literal.value(), &mut sink),
    }
}

fn write_escaped_csv_string(s: &str, mut sink: impl Write) -> io::Result<()> {
    if s.bytes().any(|c| matches!(c, b'"' | b',' | b'\n' | b'\r')) {
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
        QueryResults::Boolean(value) => {
            sink.write_all(if value { b"true" } else { b"false" })?;
        }
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
                        write_tsv_term(value, &mut sink)?;
                    }
                }
            }
        }
        QueryResults::Graph(g) => {
            sink.write_all(b"subject\tpredicate\tobject")?;
            for t in g {
                let t = t?;
                sink.write_all(b"\n")?;
                write_tsv_term(&t.subject, &mut sink)?;
                sink.write_all(b"\t")?;
                write_tsv_term(&t.predicate, &mut sink)?;
                sink.write_all(b"\t")?;
                write_tsv_term(&t.object, &mut sink)?;
            }
        }
    }
    Ok(())
}

fn write_tsv_term<'a>(term: impl Into<TermRef<'a>>, mut sink: impl Write) -> io::Result<()> {
    //TODO: full Turtle serialization
    match term.into() {
        TermRef::NamedNode(node) => write!(sink, "<{}>", node.as_str()),
        TermRef::BlankNode(node) => write!(sink, "_:{}", node.as_str()),
        TermRef::Literal(literal) => match literal.datatype() {
            xsd::BOOLEAN => match literal.value() {
                "true" | "1" => sink.write_all(b"true"),
                "false" | "0" => sink.write_all(b"false"),
                _ => sink.write_all(literal.to_string().as_bytes()),
            },
            xsd::INTEGER => {
                if literal.value().bytes().all(|c| matches!(c, b'0'..=b'9')) {
                    sink.write_all(literal.value().as_bytes())
                } else {
                    sink.write_all(literal.to_string().as_bytes())
                }
            }
            _ => sink.write_all(literal.to_string().as_bytes()),
        },
    }
}

pub fn read_tsv_results(mut source: impl BufRead + 'static) -> Result<QueryResults, io::Error> {
    let mut buffer = String::new();

    // We read the header
    source.read_line(&mut buffer)?;
    if buffer.trim().eq_ignore_ascii_case("true") {
        return Ok(QueryResults::Boolean(true));
    }
    if buffer.trim().eq_ignore_ascii_case("false") {
        return Ok(QueryResults::Boolean(false));
    }
    let variables = buffer
        .split('\t')
        .map(|v| Variable::from_str(v.trim()).map_err(invalid_data_error))
        .collect::<Result<Vec<_>, io::Error>>()?;

    Ok(QueryResults::Solutions(QuerySolutionIter::new(
        Rc::new(variables),
        Box::new(TsvResultsIterator { source, buffer }),
    )))
}

struct TsvResultsIterator<R: BufRead> {
    source: R,
    buffer: String,
}

impl<R: BufRead> Iterator for TsvResultsIterator<R> {
    type Item = Result<Vec<Option<Term>>, EvaluationError>;

    fn next(&mut self) -> Option<Result<Vec<Option<Term>>, EvaluationError>> {
        self.read_next().transpose()
    }
}

impl<R: BufRead> TsvResultsIterator<R> {
    fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, EvaluationError> {
        self.buffer.clear();
        if self.source.read_line(&mut self.buffer)? == 0 {
            return Ok(None);
        }
        Ok(Some(
            self.buffer
                .split('\t')
                .map(|v| {
                    let v = v.trim();
                    if v.is_empty() {
                        Ok(None)
                    } else {
                        Ok(Some(Term::from_str(v).map_err(invalid_data_error)?))
                    }
                })
                .collect::<Result<Vec<_>, EvaluationError>>()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::str;

    fn build_example() -> QueryResults {
        QuerySolutionIter::new(
            Rc::new(vec![
                Variable::new_unchecked("x"),
                Variable::new_unchecked("literal"),
            ]),
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
