mod csv;
mod json;
mod xml;

use crate::io::read::{ParserError, SyntaxError};
use crate::model::{Term, TermRef};
use crate::sparql::io::csv::*;
use crate::sparql::io::json::*;
use crate::sparql::io::xml::*;
use crate::sparql::{EvaluationError, QueryResults, QuerySolution, QuerySolutionIter, Variable};
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
    /// use oxigraph::sparql::QueryResultsFormat;
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
    /// use oxigraph::sparql::QueryResultsFormat;
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
    /// use oxigraph::sparql::QueryResultsFormat;
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
    /// For example "application/xml" is going to return `Xml` even if it is not its canonical media type.
    ///
    /// Example:
    /// ```
    /// use oxigraph::sparql::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::from_media_type("application/sparql-results+json; charset=utf-8"), Some(QueryResultsFormat::Json))
    /// ```
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
    /// use oxigraph::sparql::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::from_extension("json"), Some(QueryResultsFormat::Json))
    /// ```
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
/// * [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/) ([`QueryResultsFormat::Xml`](QueryResultsFormat::Xml))
/// * [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) ([`QueryResultsFormat::Json`](QueryResultsFormat::Json))
/// * [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/) ([`QueryResultsFormat::Tsv`](QueryResultsFormat::Tsv))
#[allow(missing_copy_implementations)]
pub struct QueryResultsParser {
    format: QueryResultsFormat,
}

impl QueryResultsParser {
    /// Builds a parser for the given format.
    pub fn from_format(format: QueryResultsFormat) -> Self {
        Self { format }
    }

    pub fn read_results<R: BufRead>(
        &self,
        reader: R,
    ) -> Result<QueryResultsReader<R>, ParserError> {
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

pub enum QueryResultsReader<R: BufRead> {
    Solutions(SolutionsReader<R>),
    Boolean(bool),
}

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
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl<R: BufRead> Iterator for SolutionsReaderKind<R> {
    type Item = Result<Vec<Option<Term>>, ParserError>;

    fn next(&mut self) -> Option<Result<Vec<Option<Term>>, ParserError>> {
        match self {
            Self::Xml(reader) => reader.read_next(),
            Self::Json(reader) => reader.read_next(),
            Self::Tsv(reader) => reader.read_next(),
        }
        .transpose()
    }
}

impl<R: BufRead> Iterator for SolutionsReader<R> {
    type Item = Result<QuerySolution, ParserError>;

    fn next(&mut self) -> Option<Result<QuerySolution, ParserError>> {
        Some(self.solutions.next()?.map(|values| QuerySolution {
            values,
            variables: self.variables.clone(),
        }))
    }
}

impl<R: BufRead + 'static> From<SolutionsReader<R>> for QuerySolutionIter {
    fn from(reader: SolutionsReader<R>) -> Self {
        Self::new(
            reader.variables.clone(),
            Box::new(reader.solutions.map(|r| r.map_err(EvaluationError::from))),
        )
    }
}

impl<R: BufRead + 'static> From<QueryResultsReader<R>> for QueryResults {
    fn from(reader: QueryResultsReader<R>) -> Self {
        match reader {
            QueryResultsReader::Solutions(s) => Self::Solutions(s.into()),
            QueryResultsReader::Boolean(v) => Self::Boolean(v),
        }
    }
}

/// A serializer for [SPARQL query](https://www.w3.org/TR/sparql11-query/) results serialization formats.
///
/// It currently supports the following formats:
/// * [SPARQL Query Results XML Format](http://www.w3.org/TR/rdf-sparql-XMLres/) ([`QueryResultsFormat::Xml`](QueryResultsFormat::Xml))
/// * [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) ([`QueryResultsFormat::Json`](QueryResultsFormat::Json))
/// * [SPARQL Query Results CSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/) ([`QueryResultsFormat::Csv`](QueryResultsFormat::Csv))
/// * [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/) ([`QueryResultsFormat::Tsv`](QueryResultsFormat::Tsv))
#[allow(missing_copy_implementations)]
pub struct QueryResultsSerializer {
    format: QueryResultsFormat,
}

impl QueryResultsSerializer {
    /// Builds a serializer for the given format
    pub fn from_format(format: QueryResultsFormat) -> Self {
        Self { format }
    }

    pub fn write_boolean_result<W: Write>(&self, writer: W, value: bool) -> io::Result<W> {
        match self.format {
            QueryResultsFormat::Xml => write_boolean_xml_result(writer, value),
            QueryResultsFormat::Json => write_boolean_json_result(writer, value),
            QueryResultsFormat::Csv => write_boolean_csv_result(writer, value),
            QueryResultsFormat::Tsv => write_boolean_tsv_result(writer, value),
        }
    }

    /// Returns a `SolutionsWriter` allowing writing query solutions into the given [`Write`](std::io::Write) implementation
    pub fn solutions_writer<W: Write>(
        &self,
        writer: W,
        variables: &[Variable],
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
    /// Writes a solution
    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = Option<TermRef<'a>>>,
    ) -> io::Result<()> {
        match &mut self.formatter {
            SolutionsWriterKind::Xml(writer) => writer.write(solution),
            SolutionsWriterKind::Json(writer) => writer.write(solution),
            SolutionsWriterKind::Csv(writer) => writer.write(solution),
            SolutionsWriterKind::Tsv(writer) => writer.write(solution),
        }
    }

    /// Writes the last bytes of the file
    pub fn finish(self) -> io::Result<()> {
        match self.formatter {
            SolutionsWriterKind::Xml(write) => write.finish()?,
            SolutionsWriterKind::Json(write) => write.finish()?,
            SolutionsWriterKind::Csv(write) => write.finish(),
            SolutionsWriterKind::Tsv(write) => write.finish(),
        };
        Ok(())
    }
}
