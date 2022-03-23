#![doc = include_str!("../README.md")]
#![deny(unsafe_code)]
#![doc(test(attr(deny(warnings))))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

mod csv;
mod error;
mod json;
pub mod solution;
mod xml;

use crate::csv::*;
pub use crate::error::{ParseError, SyntaxError};
use crate::json::*;
pub use crate::solution::QuerySolution;
use crate::xml::*;
use oxrdf::{TermRef, Variable, VariableRef};
use std::io::{self, BufRead, Write};
use std::rc::Rc;

/// [SPARQL query](https://www.w3.org/TR/sparql11-query/) results serialization formats.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[non_exhaustive]
pub enum QueryResultsFormat {
    /// [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/)
    Xml,
    /// [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)
    Json,
    /// [SPARQL Query Results CSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/)
    Csv,
    /// [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/)
    Tsv,
}

impl QueryResultsFormat {
    /// The format canonical IRI according to the [Unique URIs for file formats registry](https://www.w3.org/ns/formats/).
    ///
    /// ```
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::Json.iri(), "http://www.w3.org/ns/formats/SPARQL_Results_JSON")
    /// ```
    #[inline]
    pub fn iri(self) -> &'static str {
        match self {
            QueryResultsFormat::Xml => "http://www.w3.org/ns/formats/SPARQL_Results_XML",
            QueryResultsFormat::Json => "http://www.w3.org/ns/formats/SPARQL_Results_JSON",
            QueryResultsFormat::Csv => "http://www.w3.org/ns/formats/SPARQL_Results_CSV",
            QueryResultsFormat::Tsv => "http://www.w3.org/ns/formats/SPARQL_Results_TSV",
        }
    }
    /// The format [IANA media type](https://tools.ietf.org/html/rfc2046).
    ///
    /// ```
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::Json.media_type(), "application/sparql-results+json")
    /// ```
    #[inline]
    pub fn media_type(self) -> &'static str {
        match self {
            QueryResultsFormat::Xml => "application/sparql-results+xml",
            QueryResultsFormat::Json => "application/sparql-results+json",
            QueryResultsFormat::Csv => "text/csv; charset=utf-8",
            QueryResultsFormat::Tsv => "text/tab-separated-values; charset=utf-8",
        }
    }

    /// The format [IANA-registered](https://tools.ietf.org/html/rfc2046) file extension.
    ///
    /// ```
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::Json.file_extension(), "srj")
    /// ```
    #[inline]
    pub fn file_extension(self) -> &'static str {
        match self {
            QueryResultsFormat::Xml => "srx",
            QueryResultsFormat::Json => "srj",
            QueryResultsFormat::Csv => "csv",
            QueryResultsFormat::Tsv => "tsv",
        }
    }

    /// Looks for a known format from a media type.
    ///
    /// It supports some media type aliases.
    /// For example, "application/xml" is going to return `Xml` even if it is not its canonical media type.
    ///
    /// Example:
    /// ```
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::from_media_type("application/sparql-results+json; charset=utf-8"), Some(QueryResultsFormat::Json))
    /// ```
    #[inline]
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        match media_type.split(';').next()?.trim() {
            "application/sparql-results+xml" | "application/xml" | "text/xml" => Some(Self::Xml),
            "application/sparql-results+json" | "application/json" | "text/json" => {
                Some(Self::Json)
            }
            "text/csv" => Some(Self::Csv),
            "text/tab-separated-values" | "text/tsv" => Some(Self::Tsv),
            _ => None,
        }
    }

    /// Looks for a known format from an extension.
    ///
    /// It supports some aliases.
    ///
    /// Example:
    /// ```
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::from_extension("json"), Some(QueryResultsFormat::Json))
    /// ```
    #[inline]
    pub fn from_extension(extension: &str) -> Option<Self> {
        match extension {
            "srx" | "xml" => Some(Self::Xml),
            "srj" | "json" => Some(Self::Json),
            "csv" | "txt" => Some(Self::Csv),
            "tsv" => Some(Self::Tsv),
            _ => None,
        }
    }
}

/// Parsers for [SPARQL query](https://www.w3.org/TR/sparql11-query/) results serialization formats.
///
/// It currently supports the following formats:
/// * [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/) ([`QueryResultsFormat::Xml`](QueryResultsFormat::Xml)).
/// * [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) ([`QueryResultsFormat::Json`](QueryResultsFormat::Json)).
/// * [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/) ([`QueryResultsFormat::Tsv`](QueryResultsFormat::Tsv)).
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, QueryResultsReader};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// // boolean
/// if let QueryResultsReader::Boolean(v) = json_parser.read_results(b"{\"boolean\":true}".as_slice())? {
///     assert_eq!(v, true);
/// }
/// // solutions
/// if let QueryResultsReader::Solutions(solutions) = json_parser.read_results(b"{\"head\":{\"vars\":[\"foo\",\"bar\"]},\"results\":{\"bindings\":[{\"foo\":{\"type\":\"literal\",\"value\":\"test\"}}]}}".as_slice())? {
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
    /// Example in XML (the API is the same for JSON and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, QueryResultsReader};
    /// use oxrdf::{Literal, Variable};
    ///
    /// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Xml);
    ///
    /// // boolean
    /// if let QueryResultsReader::Boolean(v) = json_parser.read_results(b"<sparql xmlns=\"http://www.w3.org/2005/sparql-results#\"><head/><boolean>true</boolean></sparql>".as_slice())? {
    ///     assert_eq!(v, true);
    /// }
    ///
    /// // solutions
    /// if let QueryResultsReader::Solutions(solutions) = json_parser.read_results(b"<sparql xmlns=\"http://www.w3.org/2005/sparql-results#\"><head><variable name=\"foo\"/><variable name=\"bar\"/></head><results><result><binding name=\"foo\"><literal>test</literal></binding></result></results></sparql>".as_slice())? {
    ///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
    ///     for solution in solutions {
    ///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
    ///     }
    /// }
    /// # Result::<(),sparesults::ParseError>::Ok(())
    /// ```
    pub fn read_results<R: BufRead>(&self, reader: R) -> Result<QueryResultsReader<R>, ParseError> {
        Ok(match self.format {
            QueryResultsFormat::Xml => match XmlQueryResultsReader::read(reader)? {
                XmlQueryResultsReader::Boolean(r) => QueryResultsReader::Boolean(r),
                XmlQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => QueryResultsReader::Solutions(SolutionsReader {
                    variables: Rc::new(variables),
                    solutions: SolutionsReaderKind::Xml(solutions),
                }),
            },
            QueryResultsFormat::Json => match JsonQueryResultsReader::read(reader)? {
                JsonQueryResultsReader::Boolean(r) => QueryResultsReader::Boolean(r),
                JsonQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => QueryResultsReader::Solutions(SolutionsReader {
                    variables: Rc::new(variables),
                    solutions: SolutionsReaderKind::Json(solutions),
                }),
            },
            QueryResultsFormat::Csv => return Err(SyntaxError::msg("CSV SPARQL results syntax is lossy and can't be parsed to a proper RDF representation").into()),
            QueryResultsFormat::Tsv => match TsvQueryResultsReader::read(reader)? {
                TsvQueryResultsReader::Boolean(r) => QueryResultsReader::Boolean(r),
                TsvQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => QueryResultsReader::Solutions(SolutionsReader {
                    variables: Rc::new(variables),
                    solutions: SolutionsReaderKind::Tsv(solutions),
                }),
            },
        })
    }
}

/// The reader for a given read of a results file.
///
/// It is either a read boolean ([`bool`]) or a streaming reader of a set of solutions ([`SolutionsReader`]).
///
/// Example in TSV (the API is the same for JSON and XML):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, QueryResultsReader};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
///
/// // boolean
/// if let QueryResultsReader::Boolean(v) = json_parser.read_results(b"true".as_slice())? {
///     assert_eq!(v, true);
/// }
///
/// // solutions
/// if let QueryResultsReader::Solutions(solutions) = json_parser.read_results(b"?foo\t?bar\n\"test\"\t".as_slice())? {
///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
///     }
/// }
/// # Result::<(),sparesults::ParseError>::Ok(())
/// ```
pub enum QueryResultsReader<R: BufRead> {
    Solutions(SolutionsReader<R>),
    Boolean(bool),
}

/// A streaming reader of a set of [`QuerySolution`] solutions.
///
/// It implements the [`Iterator`] API to iterate over the solutions.
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, QueryResultsReader};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// if let QueryResultsReader::Solutions(solutions) = json_parser.read_results(b"{\"head\":{\"vars\":[\"foo\",\"bar\"]},\"results\":{\"bindings\":[{\"foo\":{\"type\":\"literal\",\"value\":\"test\"}}]}}".as_slice())? {
///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
///     }
/// }
/// # Result::<(),sparesults::ParseError>::Ok(())
/// ```
pub struct SolutionsReader<R: BufRead> {
    variables: Rc<Vec<Variable>>,
    solutions: SolutionsReaderKind<R>,
}

enum SolutionsReaderKind<R: BufRead> {
    Xml(XmlSolutionsReader<R>),
    Json(JsonSolutionsReader<R>),
    Tsv(TsvSolutionsReader<R>),
}

impl<R: BufRead> SolutionsReader<R> {
    /// Ordered list of the declared variables at the beginning of the results.
    ///
    /// Example in TSV (the API is the same for JSON and XML):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, QueryResultsReader};
    /// use oxrdf::Variable;
    ///
    /// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
    /// if let QueryResultsReader::Solutions(solutions) = json_parser.read_results(b"?foo\t?bar\n\"ex1\"\t\"ex2\"".as_slice())? {
    ///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
    /// }
    /// # Result::<(),sparesults::ParseError>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl<R: BufRead> Iterator for SolutionsReader<R> {
    type Item = Result<QuerySolution, ParseError>;

    fn next(&mut self) -> Option<Result<QuerySolution, ParseError>> {
        Some(
            match &mut self.solutions {
                SolutionsReaderKind::Xml(reader) => reader.read_next(),
                SolutionsReaderKind::Json(reader) => reader.read_next(),
                SolutionsReaderKind::Tsv(reader) => reader.read_next(),
            }
            .transpose()?
            .map(|values| (self.variables.clone(), values).into()),
        )
    }
}

/// A serializer for [SPARQL query](https://www.w3.org/TR/sparql11-query/) results serialization formats.
///
/// It currently supports the following formats:
/// * [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/) ([`QueryResultsFormat::Xml`](QueryResultsFormat::Xml))
/// * [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) ([`QueryResultsFormat::Json`](QueryResultsFormat::Json))
/// * [SPARQL Query Results CSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/) ([`QueryResultsFormat::Csv`](QueryResultsFormat::Csv))
/// * [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/) ([`QueryResultsFormat::Tsv`](QueryResultsFormat::Tsv))
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
/// use oxrdf::{LiteralRef, Variable, VariableRef};
/// use std::iter::once;
///
/// let json_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Json);
///
/// // boolean
/// let mut buffer = Vec::new();
/// json_serializer.write_boolean_result(&mut buffer, true)?;
/// assert_eq!(buffer, b"{\"head\":{},\"boolean\":true}");
///
/// // solutions
/// let mut buffer = Vec::new();
/// let mut writer = json_serializer.solutions_writer(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")])?;
/// writer.write(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test"))))?;
/// writer.finish()?;
/// assert_eq!(buffer, b"{\"head\":{\"vars\":[\"foo\",\"bar\"]},\"results\":{\"bindings\":[{\"foo\":{\"type\":\"literal\",\"value\":\"test\"}}]}}");
/// # std::io::Result::Ok(())
/// ```
pub struct QueryResultsSerializer {
    format: QueryResultsFormat,
}

impl QueryResultsSerializer {
    /// Builds a serializer for the given format.
    #[inline]
    pub fn from_format(format: QueryResultsFormat) -> Self {
        Self { format }
    }

    /// Write a boolean query result (from an `ASK` query)  into the given [`Write`](std::io::Write) implementation.
    ///
    /// Example in XML (the API is the same for JSON and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
    ///
    /// let json_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Xml);
    /// let mut buffer = Vec::new();
    /// json_serializer.write_boolean_result(&mut buffer, true)?;
    /// assert_eq!(buffer, b"<?xml version=\"1.0\"?><sparql xmlns=\"http://www.w3.org/2005/sparql-results#\"><head></head><boolean>true</boolean></sparql>");
    /// # std::io::Result::Ok(())
    /// ```
    pub fn write_boolean_result<W: Write>(&self, writer: W, value: bool) -> io::Result<W> {
        match self.format {
            QueryResultsFormat::Xml => write_boolean_xml_result(writer, value),
            QueryResultsFormat::Json => write_boolean_json_result(writer, value),
            QueryResultsFormat::Csv => write_boolean_csv_result(writer, value),
            QueryResultsFormat::Tsv => write_boolean_tsv_result(writer, value),
        }
    }

    /// Returns a `SolutionsWriter` allowing writing query solutions into the given [`Write`](std::io::Write) implementation.
    ///
    /// Example in XML (the API is the same for JSON and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
    /// use oxrdf::{LiteralRef, Variable, VariableRef};
    /// use std::iter::once;
    ///
    /// let json_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Xml);
    /// let mut buffer = Vec::new();
    /// let mut writer = json_serializer.solutions_writer(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")])?;
    /// writer.write(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test"))))?;
    /// writer.finish()?;
    /// assert_eq!(buffer, b"<?xml version=\"1.0\"?><sparql xmlns=\"http://www.w3.org/2005/sparql-results#\"><head><variable name=\"foo\"/><variable name=\"bar\"/></head><results><result><binding name=\"foo\"><literal>test</literal></binding></result></results></sparql>");
    /// # std::io::Result::Ok(())
    /// ```
    pub fn solutions_writer<W: Write>(
        &self,
        writer: W,
        variables: Vec<Variable>,
    ) -> io::Result<SolutionsWriter<W>> {
        Ok(SolutionsWriter {
            formatter: match self.format {
                QueryResultsFormat::Xml => {
                    SolutionsWriterKind::Xml(XmlSolutionsWriter::start(writer, variables)?)
                }
                QueryResultsFormat::Json => {
                    SolutionsWriterKind::Json(JsonSolutionsWriter::start(writer, variables)?)
                }
                QueryResultsFormat::Csv => {
                    SolutionsWriterKind::Csv(CsvSolutionsWriter::start(writer, variables)?)
                }
                QueryResultsFormat::Tsv => {
                    SolutionsWriterKind::Tsv(TsvSolutionsWriter::start(writer, variables)?)
                }
            },
        })
    }
}

/// Allows writing query results.
/// Could be built using a [`QueryResultsSerializer`].
///
/// Warning: Do not forget to run the [`finish`](SolutionsWriter::finish()) method to properly write the last bytes of the file.
///
/// Example in TSV (the API is the same for JSON and XML):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
/// use oxrdf::{LiteralRef, Variable, VariableRef};
/// use std::iter::once;
///
/// let json_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Tsv);
/// let mut buffer = Vec::new();
/// let mut writer = json_serializer.solutions_writer(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")])?;
/// writer.write(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test"))))?;
/// writer.finish()?;
/// assert_eq!(buffer, b"?foo\t?bar\n\"test\"\t");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct SolutionsWriter<W: Write> {
    formatter: SolutionsWriterKind<W>,
}

enum SolutionsWriterKind<W: Write> {
    Xml(XmlSolutionsWriter<W>),
    Json(JsonSolutionsWriter<W>),
    Csv(CsvSolutionsWriter<W>),
    Tsv(TsvSolutionsWriter<W>),
}

impl<W: Write> SolutionsWriter<W> {
    /// Writes a solution.
    ///
    /// Example in JSON (the API is the same for XML and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsSerializer, QuerySolution};
    /// use oxrdf::{Literal, LiteralRef, Variable, VariableRef};
    /// use std::iter::once;
    ///
    /// let json_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Json);
    /// let mut buffer = Vec::new();
    /// let mut writer = json_serializer.solutions_writer(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")])?;
    /// writer.write(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test"))))?;
    /// writer.write(&QuerySolution::from((vec![Variable::new_unchecked("bar")], vec![Some(Literal::from("test").into())])))?;
    /// writer.finish()?;
    /// assert_eq!(buffer, b"{\"head\":{\"vars\":[\"foo\",\"bar\"]},\"results\":{\"bindings\":[{\"foo\":{\"type\":\"literal\",\"value\":\"test\"}},{\"bar\":{\"type\":\"literal\",\"value\":\"test\"}}]}}");
    /// # std::io::Result::Ok(())
    /// ```
    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (impl Into<VariableRef<'a>>, impl Into<TermRef<'a>>)>,
    ) -> io::Result<()> {
        let solution = solution.into_iter().map(|(v, s)| (v.into(), s.into()));
        match &mut self.formatter {
            SolutionsWriterKind::Xml(writer) => writer.write(solution),
            SolutionsWriterKind::Json(writer) => writer.write(solution),
            SolutionsWriterKind::Csv(writer) => writer.write(solution),
            SolutionsWriterKind::Tsv(writer) => writer.write(solution),
        }
    }

    /// Writes the last bytes of the file.
    pub fn finish(self) -> io::Result<W> {
        match self.formatter {
            SolutionsWriterKind::Xml(write) => write.finish(),
            SolutionsWriterKind::Json(write) => write.finish(),
            SolutionsWriterKind::Csv(write) => write.finish(),
            SolutionsWriterKind::Tsv(write) => write.finish(),
        }
    }
}
