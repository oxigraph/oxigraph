//! Implementation of [SPARQL 1.1 Query Results CSV and TSV Formats](https://www.w3.org/TR/sparql11-results-csv-tsv/)

use crate::error::{ParseError, SyntaxError, SyntaxErrorKind};
use oxrdf::Variable;
use oxrdf::{vocab::xsd, *};
use std::io::{self, BufRead, Write};
use std::str::FromStr;

pub fn write_boolean_csv_result<W: Write>(mut sink: W, value: bool) -> io::Result<W> {
    sink.write_all(if value { b"true" } else { b"false" })?;
    Ok(sink)
}

pub struct CsvSolutionsWriter<W: Write> {
    sink: W,
    variables: Vec<Variable>,
}

impl<W: Write> CsvSolutionsWriter<W> {
    pub fn start(mut sink: W, variables: Vec<Variable>) -> io::Result<Self> {
        let mut start_vars = true;
        for variable in &variables {
            if start_vars {
                start_vars = false;
            } else {
                sink.write_all(b",")?;
            }
            sink.write_all(variable.as_str().as_bytes())?;
        }
        Ok(Self { sink, variables })
    }

    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> io::Result<()> {
        let mut values = vec![None; self.variables.len()];
        for (variable, value) in solution {
            if let Some(position) = self.variables.iter().position(|v| *v == variable) {
                values[position] = Some(value);
            }
        }
        self.sink.write_all(b"\r\n")?;
        let mut start_binding = true;
        for value in values {
            if start_binding {
                start_binding = false;
            } else {
                self.sink.write_all(b",")?;
            }
            if let Some(value) = value {
                write_csv_term(value, &mut self.sink)?;
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.sink.flush()?;
        Ok(self.sink)
    }
}

fn write_csv_term<'a>(term: impl Into<TermRef<'a>>, sink: &mut impl Write) -> io::Result<()> {
    match term.into() {
        TermRef::NamedNode(uri) => sink.write_all(uri.as_str().as_bytes()),
        TermRef::BlankNode(bnode) => {
            sink.write_all(b"_:")?;
            sink.write_all(bnode.as_str().as_bytes())
        }
        TermRef::Literal(literal) => write_escaped_csv_string(literal.value(), sink),
        #[cfg(feature = "rdf-star")]
        TermRef::Triple(triple) => {
            write_csv_term(&triple.subject, sink)?;
            sink.write_all(b" ")?;
            write_csv_term(&triple.predicate, sink)?;
            sink.write_all(b" ")?;
            write_csv_term(&triple.object, sink)
        }
    }
}

fn write_escaped_csv_string(s: &str, sink: &mut impl Write) -> io::Result<()> {
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

pub fn write_boolean_tsv_result<W: Write>(mut sink: W, value: bool) -> io::Result<W> {
    sink.write_all(if value { b"true" } else { b"false" })?;
    Ok(sink)
}

pub struct TsvSolutionsWriter<W: Write> {
    sink: W,
    variables: Vec<Variable>,
}

impl<W: Write> TsvSolutionsWriter<W> {
    pub fn start(mut sink: W, variables: Vec<Variable>) -> io::Result<Self> {
        let mut start_vars = true;
        for variable in &variables {
            if start_vars {
                start_vars = false;
            } else {
                sink.write_all(b"\t")?;
            }
            sink.write_all(b"?")?;
            sink.write_all(variable.as_str().as_bytes())?;
        }
        Ok(Self { sink, variables })
    }

    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (VariableRef<'a>, TermRef<'a>)>,
    ) -> io::Result<()> {
        let mut values = vec![None; self.variables.len()];
        for (variable, value) in solution {
            if let Some(position) = self.variables.iter().position(|v| *v == variable) {
                values[position] = Some(value);
            }
        }
        self.sink.write_all(b"\n")?;
        let mut start_binding = true;
        for value in values {
            if start_binding {
                start_binding = false;
            } else {
                self.sink.write_all(b"\t")?;
            }
            if let Some(value) = value {
                write_tsv_term(value, &mut self.sink)?;
            }
        }
        Ok(())
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.sink.flush()?;
        Ok(self.sink)
    }
}

fn write_tsv_term<'a>(term: impl Into<TermRef<'a>>, sink: &mut impl Write) -> io::Result<()> {
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
        #[cfg(feature = "rdf-star")]
        TermRef::Triple(triple) => {
            sink.write_all(b"<<")?;
            write_tsv_term(&triple.subject, sink)?;
            sink.write_all(b" ")?;
            write_tsv_term(&triple.predicate, sink)?;
            sink.write_all(b" ")?;
            write_tsv_term(&triple.object, sink)?;
            sink.write_all(b">>")?;
            Ok(())
        }
    }
}

pub enum TsvQueryResultsReader<R: BufRead> {
    Solutions {
        variables: Vec<Variable>,
        solutions: TsvSolutionsReader<R>,
    },
    Boolean(bool),
}

impl<R: BufRead> TsvQueryResultsReader<R> {
    pub fn read(mut source: R) -> Result<Self, ParseError> {
        let mut buffer = String::new();

        // We read the header
        source.read_line(&mut buffer)?;
        if buffer.trim().eq_ignore_ascii_case("true") {
            return Ok(Self::Boolean(true));
        }
        if buffer.trim().eq_ignore_ascii_case("false") {
            return Ok(Self::Boolean(false));
        }
        let mut variables = Vec::new();
        for v in buffer.split('\t') {
            let v = v.trim();
            let variable = Variable::from_str(v).map_err(|e| {
                SyntaxError::msg(format!("Invalid variable declaration '{}': {}", v, e))
            })?;
            if variables.contains(&variable) {
                return Err(SyntaxError::msg(format!(
                    "The variable {} is declared twice",
                    variable
                ))
                .into());
            }
            variables.push(variable);
        }

        Ok(Self::Solutions {
            variables,
            solutions: TsvSolutionsReader { source, buffer },
        })
    }
}

pub struct TsvSolutionsReader<R: BufRead> {
    source: R,
    buffer: String,
}

impl<R: BufRead> TsvSolutionsReader<R> {
    pub fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, ParseError> {
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
                        Ok(Some(Term::from_str(v).map_err(|e| SyntaxError {
                            inner: SyntaxErrorKind::Term(e),
                        })?))
                    }
                })
                .collect::<Result<_, ParseError>>()?,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::rc::Rc;
    use std::str;

    fn build_example() -> (Vec<Variable>, Vec<Vec<Option<Term>>>) {
        (
            vec![
                Variable::new_unchecked("x"),
                Variable::new_unchecked("literal"),
            ],
            vec![
                vec![
                    Some(NamedNode::new_unchecked("http://example/x").into()),
                    Some(Literal::new_simple_literal("String").into()),
                ],
                vec![
                    Some(NamedNode::new_unchecked("http://example/x").into()),
                    Some(Literal::new_simple_literal("String-with-dquote\"").into()),
                ],
                vec![
                    Some(BlankNode::new_unchecked("b0").into()),
                    Some(Literal::new_simple_literal("Blank node").into()),
                ],
                vec![
                    None,
                    Some(Literal::new_simple_literal("Missing 'x'").into()),
                ],
                vec![None, None],
                vec![
                    Some(NamedNode::new_unchecked("http://example/x").into()),
                    None,
                ],
                vec![
                    Some(BlankNode::new_unchecked("b1").into()),
                    Some(
                        Literal::new_language_tagged_literal_unchecked("String-with-lang", "en")
                            .into(),
                    ),
                ],
                vec![
                    Some(BlankNode::new_unchecked("b1").into()),
                    Some(Literal::new_typed_literal("123", xsd::INTEGER).into()),
                ],
            ],
        )
    }

    #[test]
    fn test_csv_serialization() -> io::Result<()> {
        let (variables, solutions) = build_example();
        let mut writer = CsvSolutionsWriter::start(Vec::new(), variables.clone())?;
        let variables = Rc::new(variables);
        for solution in solutions {
            writer.write(
                variables
                    .iter()
                    .zip(&solution)
                    .filter_map(|(v, s)| s.as_ref().map(|s| (v.as_ref(), s.as_ref()))),
            )?;
        }
        let result = writer.finish()?;
        assert_eq!(str::from_utf8(&result).unwrap(), "x,literal\r\nhttp://example/x,String\r\nhttp://example/x,\"String-with-dquote\"\"\"\r\n_:b0,Blank node\r\n,Missing 'x'\r\n,\r\nhttp://example/x,\r\n_:b1,String-with-lang\r\n_:b1,123");
        Ok(())
    }

    #[test]
    fn test_tsv_serialization() -> io::Result<()> {
        let (variables, solutions) = build_example();
        let mut writer = TsvSolutionsWriter::start(Vec::new(), variables.clone())?;
        let variables = Rc::new(variables);
        for solution in solutions {
            writer.write(
                variables
                    .iter()
                    .zip(&solution)
                    .filter_map(|(v, s)| s.as_ref().map(|s| (v.as_ref(), s.as_ref()))),
            )?;
        }
        let result = writer.finish()?;
        assert_eq!(str::from_utf8(&result).unwrap(), "?x\t?literal\n<http://example/x>\t\"String\"\n<http://example/x>\t\"String-with-dquote\\\"\"\n_:b0\t\"Blank node\"\n\t\"Missing 'x'\"\n\t\n<http://example/x>\t\n_:b1\t\"String-with-lang\"@en\n_:b1\t123");
        Ok(())
    }

    #[test]
    fn test_bad_tsv() {
        let mut bad_tsvs = vec![
            "?", "?p", "?p?o", "?p\n<", "?p\n_", "?p\n_:", "?p\n\"", "?p\n<<",
        ];
        let a_lot_of_strings = format!("?p\n{}\n", "<".repeat(100_000));
        bad_tsvs.push(&a_lot_of_strings);
        for bad_tsv in bad_tsvs {
            if let Ok(TsvQueryResultsReader::Solutions { mut solutions, .. }) =
                TsvQueryResultsReader::read(Cursor::new(bad_tsv))
            {
                while let Ok(Some(_)) = solutions.read_next() {}
            }
        }
    }
}
