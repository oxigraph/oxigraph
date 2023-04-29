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
        sink.write_all(b"\r\n")?;
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
        self.sink.write_all(b"\r\n")
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
        sink.write_all(b"\n")?;
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
        self.sink.write_all(b"\n")
    }

    pub fn finish(mut self) -> io::Result<W> {
        self.sink.flush()?;
        Ok(self.sink)
    }
}

fn write_tsv_term<'a>(term: impl Into<TermRef<'a>>, sink: &mut impl Write) -> io::Result<()> {
    match term.into() {
        TermRef::NamedNode(node) => write!(sink, "<{}>", node.as_str()),
        TermRef::BlankNode(node) => write!(sink, "_:{}", node.as_str()),
        TermRef::Literal(literal) => {
            let value = literal.value();
            if let Some(language) = literal.language() {
                write_tsv_quoted_str(value, sink)?;
                write!(sink, "@{language}")
            } else {
                match literal.datatype() {
                    xsd::BOOLEAN if is_turtle_boolean(value) => sink.write_all(value.as_bytes()),
                    xsd::INTEGER if is_turtle_integer(value) => sink.write_all(value.as_bytes()),
                    xsd::DECIMAL if is_turtle_decimal(value) => sink.write_all(value.as_bytes()),
                    xsd::DOUBLE if is_turtle_double(value) => sink.write_all(value.as_bytes()),
                    xsd::STRING => write_tsv_quoted_str(value, sink),
                    datatype => {
                        write_tsv_quoted_str(value, sink)?;
                        write!(sink, "^^<{}>", datatype.as_str())
                    }
                }
            }
        }
        #[cfg(feature = "rdf-star")]
        TermRef::Triple(triple) => {
            sink.write_all(b"<< ")?;
            write_tsv_term(&triple.subject, sink)?;
            sink.write_all(b" ")?;
            write_tsv_term(&triple.predicate, sink)?;
            sink.write_all(b" ")?;
            write_tsv_term(&triple.object, sink)?;
            sink.write_all(b" >>")?;
            Ok(())
        }
    }
}

fn write_tsv_quoted_str(string: &str, f: &mut impl Write) -> io::Result<()> {
    f.write_all(b"\"")?;
    for c in string.bytes() {
        match c {
            b'\t' => f.write_all(b"\\t"),
            b'\n' => f.write_all(b"\\n"),
            b'\r' => f.write_all(b"\\r"),
            b'"' => f.write_all(b"\\\""),
            b'\\' => f.write_all(b"\\\\"),
            c => f.write_all(&[c]),
        }?;
    }
    f.write_all(b"\"")
}

fn is_turtle_boolean(value: &str) -> bool {
    matches!(value, "true" | "false")
}

fn is_turtle_integer(value: &str) -> bool {
    // [19] 	INTEGER 	::= 	[+-]? [0-9]+
    let mut value = value.as_bytes();
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    !value.is_empty() && value.iter().all(u8::is_ascii_digit)
}

fn is_turtle_decimal(value: &str) -> bool {
    // [20] 	DECIMAL 	::= 	[+-]? [0-9]* '.' [0-9]+
    let mut value = value.as_bytes();
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    while value.first().map_or(false, u8::is_ascii_digit) {
        value = &value[1..];
    }
    if let Some(v) = value.strip_prefix(b".") {
        value = v;
    } else {
        return false;
    }
    !value.is_empty() && value.iter().all(u8::is_ascii_digit)
}

fn is_turtle_double(value: &str) -> bool {
    // [21] 	DOUBLE 	::= 	[+-]? ([0-9]+ '.' [0-9]* EXPONENT | '.' [0-9]+ EXPONENT | [0-9]+ EXPONENT)
    // [154s] 	EXPONENT 	::= 	[eE] [+-]? [0-9]+
    let mut value = value.as_bytes();
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    let mut with_before = false;
    while value.first().map_or(false, u8::is_ascii_digit) {
        value = &value[1..];
        with_before = true;
    }
    let mut with_after = false;
    if let Some(v) = value.strip_prefix(b".") {
        value = v;
        while value.first().map_or(false, u8::is_ascii_digit) {
            value = &value[1..];
            with_after = true;
        }
    }
    if let Some(v) = value.strip_prefix(b"e") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"E") {
        value = v;
    } else {
        return false;
    }
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    (with_before || with_after) && !value.is_empty() && value.iter().all(u8::is_ascii_digit)
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
        let line = buffer
            .as_str()
            .trim_matches(|c| matches!(c, ' ' | '\r' | '\n'));
        if line.eq_ignore_ascii_case("true") {
            return Ok(Self::Boolean(true));
        }
        if line.eq_ignore_ascii_case("false") {
            return Ok(Self::Boolean(false));
        }
        let mut variables = Vec::new();
        if !line.is_empty() {
            for v in line.split('\t') {
                let v = v.trim();
                if v.is_empty() {
                    return Err(SyntaxError::msg("Empty column on the first row. The first row should be a list of variables like ?foo or $bar").into());
                }
                let variable = Variable::from_str(v).map_err(|e| {
                    SyntaxError::msg(format!("Invalid variable declaration '{v}': {e}"))
                })?;
                if variables.contains(&variable) {
                    return Err(SyntaxError::msg(format!(
                        "The variable {variable} is declared twice"
                    ))
                    .into());
                }
                variables.push(variable);
            }
        }
        let column_len = variables.len();
        Ok(Self::Solutions {
            variables,
            solutions: TsvSolutionsReader {
                source,
                buffer,
                column_len,
            },
        })
    }
}

pub struct TsvSolutionsReader<R: BufRead> {
    source: R,
    buffer: String,
    column_len: usize,
}

impl<R: BufRead> TsvSolutionsReader<R> {
    pub fn read_next(&mut self) -> Result<Option<Vec<Option<Term>>>, ParseError> {
        self.buffer.clear();
        if self.source.read_line(&mut self.buffer)? == 0 {
            return Ok(None);
        }
        let elements = self
            .buffer
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
            .collect::<Result<Vec<_>, ParseError>>()?;
        if elements.len() == self.column_len {
            Ok(Some(elements))
        } else if self.column_len == 0 && elements == [None] {
            Ok(Some(Vec::new())) // Zero columns case
        } else {
            Err(SyntaxError::msg(format!(
                "This TSV files has {} columns but we found a row with {} columns: {:?}",
                self.column_len,
                elements.len(),
                self.buffer
            ))
            .into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
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
                vec![
                    None,
                    Some(Literal::new_simple_literal("escape,\t\r\n").into()),
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
        assert_eq!(str::from_utf8(&result).unwrap(), "x,literal\r\nhttp://example/x,String\r\nhttp://example/x,\"String-with-dquote\"\"\"\r\n_:b0,Blank node\r\n,Missing 'x'\r\n,\r\nhttp://example/x,\r\n_:b1,String-with-lang\r\n_:b1,123\r\n,\"escape,\t\r\n\"\r\n");
        Ok(())
    }

    #[test]
    fn test_tsv_roundtrip() -> Result<(), Box<dyn Error>> {
        let (variables, solutions) = build_example();

        // Write
        let mut writer = TsvSolutionsWriter::start(Vec::new(), variables.clone())?;
        let variables = Rc::new(variables);
        for solution in &solutions {
            writer.write(
                variables
                    .iter()
                    .zip(solution)
                    .filter_map(|(v, s)| s.as_ref().map(|s| (v.as_ref(), s.as_ref()))),
            )?;
        }
        let result = writer.finish()?;
        assert_eq!(str::from_utf8(&result).unwrap(), "?x\t?literal\n<http://example/x>\t\"String\"\n<http://example/x>\t\"String-with-dquote\\\"\"\n_:b0\t\"Blank node\"\n\t\"Missing 'x'\"\n\t\n<http://example/x>\t\n_:b1\t\"String-with-lang\"@en\n_:b1\t123\n\t\"escape,\\t\\r\\n\"\n");

        // Read
        if let TsvQueryResultsReader::Solutions {
            solutions: mut solutions_iter,
            variables: actual_variables,
        } = TsvQueryResultsReader::read(Cursor::new(result))?
        {
            assert_eq!(actual_variables.as_slice(), variables.as_slice());
            let mut rows = Vec::new();
            while let Some(row) = solutions_iter.read_next()? {
                rows.push(row);
            }
            assert_eq!(rows, solutions);
        } else {
            unreachable!()
        }

        Ok(())
    }

    #[test]
    fn test_bad_tsv() {
        let mut bad_tsvs = vec![
            "?",
            "?p",
            "?p?o",
            "?p\n<",
            "?p\n_",
            "?p\n_:",
            "?p\n\"",
            "?p\n<<",
            "?p\n1\t2\n",
            "?p\n\n",
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

    #[test]
    fn test_no_columns_csv_serialization() -> io::Result<()> {
        let mut writer = CsvSolutionsWriter::start(Vec::new(), Vec::new())?;
        writer.write([])?;
        let result = writer.finish()?;
        assert_eq!(str::from_utf8(&result).unwrap(), "\r\n\r\n");
        Ok(())
    }

    #[test]
    fn test_no_columns_tsv_serialization() -> io::Result<()> {
        let mut writer = TsvSolutionsWriter::start(Vec::new(), Vec::new())?;
        writer.write([])?;
        let result = writer.finish()?;
        assert_eq!(str::from_utf8(&result).unwrap(), "\n\n");
        Ok(())
    }

    #[test]
    fn test_no_columns_tsv_parsing() -> io::Result<()> {
        if let TsvQueryResultsReader::Solutions {
            mut solutions,
            variables,
        } = TsvQueryResultsReader::read(b"\n\n".as_slice())?
        {
            assert_eq!(variables, Vec::<Variable>::new());
            assert_eq!(solutions.read_next()?, Some(Vec::new()));
            assert_eq!(solutions.read_next()?, None);
        } else {
            unreachable!()
        }
        Ok(())
    }

    #[test]
    fn test_no_results_csv_serialization() -> io::Result<()> {
        let result =
            CsvSolutionsWriter::start(Vec::new(), vec![Variable::new_unchecked("a")])?.finish()?;
        assert_eq!(str::from_utf8(&result).unwrap(), "a\r\n");
        Ok(())
    }

    #[test]
    fn test_no_results_tsv_serialization() -> io::Result<()> {
        let result =
            TsvSolutionsWriter::start(Vec::new(), vec![Variable::new_unchecked("a")])?.finish()?;
        assert_eq!(str::from_utf8(&result).unwrap(), "?a\n");
        Ok(())
    }

    #[test]
    fn test_no_results_tsv_parsing() -> io::Result<()> {
        if let TsvQueryResultsReader::Solutions {
            mut solutions,
            variables,
        } = TsvQueryResultsReader::read(b"?a\n".as_slice())?
        {
            assert_eq!(variables, vec![Variable::new_unchecked("a")]);
            assert_eq!(solutions.read_next()?, None);
        } else {
            unreachable!()
        }
        Ok(())
    }
}
