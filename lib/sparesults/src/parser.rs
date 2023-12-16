use crate::csv::{TsvQueryResultsReader, TsvSolutionsReader};
use crate::error::{ParseError, SyntaxError};
use crate::format::QueryResultsFormat;
use crate::json::{JsonQueryResultsReader, JsonSolutionsReader};
use crate::solution::QuerySolution;
use crate::xml::{XmlQueryResultsReader, XmlSolutionsReader};
use oxrdf::Variable;
use std::io::Read;
use std::sync::Arc;

/// Parsers for [SPARQL query](https://www.w3.org/TR/sparql11-query/) results serialization formats.
///
/// It currently supports the following formats:
/// * [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/) ([`QueryResultsFormat::Xml`](QueryResultsFormat::Xml)).
/// * [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) ([`QueryResultsFormat::Json`](QueryResultsFormat::Json)).
/// * [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/) ([`QueryResultsFormat::Tsv`](QueryResultsFormat::Tsv)).
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, FromReadQueryResultsReader};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// // boolean
/// if let FromReadQueryResultsReader::Boolean(v) = json_parser.parse_read(b"{\"boolean\":true}".as_slice())? {
///     assert_eq!(v, true);
/// }
/// // solutions
/// if let FromReadQueryResultsReader::Solutions(solutions) = json_parser.parse_read(b"{\"head\":{\"vars\":[\"foo\",\"bar\"]},\"results\":{\"bindings\":[{\"foo\":{\"type\":\"literal\",\"value\":\"test\"}}]}}".as_slice())? {
///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
///     }
/// }
/// # Result::<(),sparesults::ParseError>::Ok(())
/// ```
pub struct QueryResultsParser {
    format: QueryResultsFormat,
}

impl QueryResultsParser {
    /// Builds a parser for the given format.
    #[inline]
    pub fn from_format(format: QueryResultsFormat) -> Self {
        Self { format }
    }

    /// Reads a result file.
    ///
    /// Reads are buffered.
    ///
    /// Example in XML (the API is the same for JSON and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, FromReadQueryResultsReader};
    /// use oxrdf::{Literal, Variable};
    ///
    /// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Xml);
    ///
    /// // boolean
    /// if let FromReadQueryResultsReader::Boolean(v) = json_parser.parse_read(b"<sparql xmlns=\"http://www.w3.org/2005/sparql-results#\"><head/><boolean>true</boolean></sparql>".as_slice())? {
    ///     assert_eq!(v, true);
    /// }
    ///
    /// // solutions
    /// if let FromReadQueryResultsReader::Solutions(solutions) = json_parser.parse_read(b"<sparql xmlns=\"http://www.w3.org/2005/sparql-results#\"><head><variable name=\"foo\"/><variable name=\"bar\"/></head><results><result><binding name=\"foo\"><literal>test</literal></binding></result></results></sparql>".as_slice())? {
    ///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
    ///     for solution in solutions {
    ///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
    ///     }
    /// }
    /// # Result::<(),sparesults::ParseError>::Ok(())
    /// ```
    pub fn parse_read<R: Read>(
        &self,
        reader: R,
    ) -> Result<FromReadQueryResultsReader<R>, ParseError> {
        Ok(match self.format {
            QueryResultsFormat::Xml => match XmlQueryResultsReader::read(reader)? {
                XmlQueryResultsReader::Boolean(r) => FromReadQueryResultsReader::Boolean(r),
                XmlQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromReadQueryResultsReader::Solutions(FromReadSolutionsReader {
                    variables: variables.into(),
                    solutions: SolutionsReaderKind::Xml(solutions),
                }),
            },
            QueryResultsFormat::Json => match JsonQueryResultsReader::read(reader)? {
                JsonQueryResultsReader::Boolean(r) => FromReadQueryResultsReader::Boolean(r),
                JsonQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromReadQueryResultsReader::Solutions(FromReadSolutionsReader {
                    variables: variables.into(),
                    solutions: SolutionsReaderKind::Json(solutions),
                }),
            },
            QueryResultsFormat::Csv => return Err(SyntaxError::msg("CSV SPARQL results syntax is lossy and can't be parsed to a proper RDF representation").into()),
            QueryResultsFormat::Tsv => match TsvQueryResultsReader::read(reader)? {
                TsvQueryResultsReader::Boolean(r) => FromReadQueryResultsReader::Boolean(r),
                TsvQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromReadQueryResultsReader::Solutions(FromReadSolutionsReader {
                    variables: variables.into(),
                    solutions: SolutionsReaderKind::Tsv(solutions),
                }),
            },
        })
    }

    #[deprecated(note = "use parse_read", since = "0.4.0")]
    pub fn read_results<R: Read>(
        &self,
        reader: R,
    ) -> Result<FromReadQueryResultsReader<R>, ParseError> {
        self.parse_read(reader)
    }
}

impl From<QueryResultsFormat> for QueryResultsParser {
    fn from(format: QueryResultsFormat) -> Self {
        Self::from_format(format)
    }
}

/// The reader for a given read of a results file.
///
/// It is either a read boolean ([`bool`]) or a streaming reader of a set of solutions ([`FromReadSolutionsReader`]).
///
/// Example in TSV (the API is the same for JSON and XML):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, FromReadQueryResultsReader};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
///
/// // boolean
/// if let FromReadQueryResultsReader::Boolean(v) = json_parser.parse_read(b"true".as_slice())? {
///     assert_eq!(v, true);
/// }
///
/// // solutions
/// if let FromReadQueryResultsReader::Solutions(solutions) = json_parser.parse_read(b"?foo\t?bar\n\"test\"\t".as_slice())? {
///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
///     }
/// }
/// # Result::<(),sparesults::ParseError>::Ok(())
/// ```
pub enum FromReadQueryResultsReader<R: Read> {
    Solutions(FromReadSolutionsReader<R>),
    Boolean(bool),
}

/// A streaming reader of a set of [`QuerySolution`] solutions.
///
/// It implements the [`Iterator`] API to iterate over the solutions.
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, FromReadQueryResultsReader};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// if let FromReadQueryResultsReader::Solutions(solutions) = json_parser.parse_read(b"{\"head\":{\"vars\":[\"foo\",\"bar\"]},\"results\":{\"bindings\":[{\"foo\":{\"type\":\"literal\",\"value\":\"test\"}}]}}".as_slice())? {
///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
///     }
/// }
/// # Result::<(),sparesults::ParseError>::Ok(())
/// ```
pub struct FromReadSolutionsReader<R: Read> {
    variables: Arc<[Variable]>,
    solutions: SolutionsReaderKind<R>,
}

enum SolutionsReaderKind<R: Read> {
    Xml(XmlSolutionsReader<R>),
    Json(JsonSolutionsReader<R>),
    Tsv(TsvSolutionsReader<R>),
}

impl<R: Read> FromReadSolutionsReader<R> {
    /// Ordered list of the declared variables at the beginning of the results.
    ///
    /// Example in TSV (the API is the same for JSON and XML):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, FromReadQueryResultsReader};
    /// use oxrdf::Variable;
    ///
    /// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
    /// if let FromReadQueryResultsReader::Solutions(solutions) = json_parser.parse_read(b"?foo\t?bar\n\"ex1\"\t\"ex2\"".as_slice())? {
    ///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
    /// }
    /// # Result::<(),sparesults::ParseError>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl<R: Read> Iterator for FromReadSolutionsReader<R> {
    type Item = Result<QuerySolution, ParseError>;

    fn next(&mut self) -> Option<Result<QuerySolution, ParseError>> {
        Some(
            match &mut self.solutions {
                SolutionsReaderKind::Xml(reader) => reader.read_next(),
                SolutionsReaderKind::Json(reader) => reader.read_next(),
                SolutionsReaderKind::Tsv(reader) => reader.read_next(),
            }
            .transpose()?
            .map(|values| (Arc::clone(&self.variables), values).into()),
        )
    }
}
