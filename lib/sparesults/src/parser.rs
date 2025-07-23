#![allow(clippy::large_enum_variant)]

use crate::csv::{
    ReaderTsvQueryResultsParserOutput, ReaderTsvSolutionsParser, SliceTsvQueryResultsParserOutput,
    SliceTsvSolutionsParser,
};
#[cfg(feature = "async-tokio")]
use crate::csv::{TokioAsyncReaderTsvQueryResultsParserOutput, TokioAsyncReaderTsvSolutionsParser};
use crate::error::{QueryResultsParseError, QueryResultsSyntaxError};
use crate::format::QueryResultsFormat;
use crate::json::{
    ReaderJsonQueryResultsParserOutput, ReaderJsonSolutionsParser,
    SliceJsonQueryResultsParserOutput, SliceJsonSolutionsParser,
};
#[cfg(feature = "async-tokio")]
use crate::json::{
    TokioAsyncReaderJsonQueryResultsParserOutput, TokioAsyncReaderJsonSolutionsParser,
};
use crate::solution::QuerySolution;
use crate::xml::{
    ReaderXmlQueryResultsParserOutput, ReaderXmlSolutionsParser, SliceXmlQueryResultsParserOutput,
    SliceXmlSolutionsParser,
};
#[cfg(feature = "async-tokio")]
use crate::xml::{TokioAsyncReaderXmlQueryResultsParserOutput, TokioAsyncReaderXmlSolutionsParser};
use oxrdf::Variable;
use std::io::Read;
use std::sync::Arc;
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncRead;

/// Parsers for [SPARQL query](https://www.w3.org/TR/sparql11-query/) results serialization formats.
///
/// It currently supports the following formats:
/// * [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/) ([`QueryResultsFormat::Xml`](QueryResultsFormat::Xml)).
/// * [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) ([`QueryResultsFormat::Json`](QueryResultsFormat::Json)).
/// * [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/) ([`QueryResultsFormat::Tsv`](QueryResultsFormat::Tsv)).
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, ReaderQueryResultsParserOutput};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// // boolean
/// if let ReaderQueryResultsParserOutput::Boolean(v) = json_parser.clone().for_reader(br#"{"boolean":true}"#.as_slice())? {
///     assert_eq!(v, true);
/// }
/// // solutions
/// if let ReaderQueryResultsParserOutput::Solutions(solutions) = json_parser.for_reader(br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#.as_slice())? {
///     assert_eq!(solutions.variables(), &[Variable::new("foo")?, Variable::new("bar")?]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new("foo")?, &Literal::from("test").into())]);
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
#[derive(Clone)]
pub struct QueryResultsParser {
    format: QueryResultsFormat,
}

impl QueryResultsParser {
    /// Builds a parser for the given format.
    #[inline]
    pub fn from_format(format: QueryResultsFormat) -> Self {
        Self { format }
    }

    /// Reads a result file from a [`Read`] implementation.
    ///
    /// Reads are automatically buffered.
    ///
    /// Example in XML (the API is the same for JSON and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, ReaderQueryResultsParserOutput};
    /// use oxrdf::{Literal, Variable};
    ///
    /// let xml_parser = QueryResultsParser::from_format(QueryResultsFormat::Xml);
    ///
    /// // boolean
    /// if let ReaderQueryResultsParserOutput::Boolean(v) = xml_parser.clone().for_reader(br#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head/><boolean>true</boolean></sparql>"#.as_slice())? {
    ///     assert_eq!(v, true);
    /// }
    ///
    /// // solutions
    /// if let ReaderQueryResultsParserOutput::Solutions(solutions) = xml_parser.for_reader(br#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head><variable name="foo"/><variable name="bar"/></head><results><result><binding name="foo"><literal>test</literal></binding></result></results></sparql>"#.as_slice())? {
    ///     assert_eq!(solutions.variables(), &[Variable::new("foo")?, Variable::new("bar")?]);
    ///     for solution in solutions {
    ///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new("foo")?, &Literal::from("test").into())]);
    ///     }
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_reader<R: Read>(
        self,
        reader: R,
    ) -> Result<ReaderQueryResultsParserOutput<R>, QueryResultsParseError> {
        Ok(match self.format {
            QueryResultsFormat::Xml => match ReaderXmlQueryResultsParserOutput::read(reader)? {
                ReaderXmlQueryResultsParserOutput::Boolean(r) => ReaderQueryResultsParserOutput::Boolean(r),
                ReaderXmlQueryResultsParserOutput::Solutions {
                    solutions,
                    variables,
                } => ReaderQueryResultsParserOutput::Solutions(ReaderSolutionsParser {
                    variables: variables.into(),
                    solutions: ReaderSolutionsParserKind::Xml(solutions),
                }),
            },
            QueryResultsFormat::Json => match ReaderJsonQueryResultsParserOutput::read(reader)? {
                ReaderJsonQueryResultsParserOutput::Boolean(r) => ReaderQueryResultsParserOutput::Boolean(r),
                ReaderJsonQueryResultsParserOutput::Solutions {
                    solutions,
                    variables,
                } => ReaderQueryResultsParserOutput::Solutions(ReaderSolutionsParser {
                    variables: variables.into(),
                    solutions: ReaderSolutionsParserKind::Json(solutions),
                }),
            },
            QueryResultsFormat::Csv => return Err(QueryResultsSyntaxError::msg("CSV SPARQL results syntax is lossy and can't be parsed to a proper RDF representation").into()),
            QueryResultsFormat::Tsv => match ReaderTsvQueryResultsParserOutput::read(reader)? {
                ReaderTsvQueryResultsParserOutput::Boolean(r) => ReaderQueryResultsParserOutput::Boolean(r),
                ReaderTsvQueryResultsParserOutput::Solutions {
                    solutions,
                    variables,
                } => ReaderQueryResultsParserOutput::Solutions(ReaderSolutionsParser {
                    variables: variables.into(),
                    solutions: ReaderSolutionsParserKind::Tsv(solutions),
                }),
            },
        })
    }

    /// Reads a result file from a Tokio [`AsyncRead`] implementation.
    ///
    /// Reads are automatically buffered.
    ///
    /// Example in XML (the API is the same for JSON and TSV):
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, TokioAsyncReaderQueryResultsParserOutput};
    /// use oxrdf::{Literal, Variable};
    ///
    /// let xml_parser = QueryResultsParser::from_format(QueryResultsFormat::Xml);
    ///
    /// // boolean
    /// if let TokioAsyncReaderQueryResultsParserOutput::Boolean(v) = xml_parser.clone().for_tokio_async_reader(br#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head/><boolean>true</boolean></sparql>"#.as_slice()).await? {
    ///     assert_eq!(v, true);
    /// }
    ///
    /// // solutions
    /// if let TokioAsyncReaderQueryResultsParserOutput::Solutions(mut solutions) = xml_parser.for_tokio_async_reader(br#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head><variable name="foo"/><variable name="bar"/></head><results><result><binding name="foo"><literal>test</literal></binding></result></results></sparql>"#.as_slice()).await? {
    ///     assert_eq!(solutions.variables(), &[Variable::new("foo")?, Variable::new("bar")?]);
    ///     while let Some(solution) = solutions.next().await {
    ///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new("foo")?, &Literal::from("test").into())]);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub async fn for_tokio_async_reader<R: AsyncRead + Unpin>(
        self,
        reader: R,
    ) -> Result<TokioAsyncReaderQueryResultsParserOutput<R>, QueryResultsParseError> {
        Ok(match self.format {
            QueryResultsFormat::Xml => match TokioAsyncReaderXmlQueryResultsParserOutput::read(reader).await? {
                TokioAsyncReaderXmlQueryResultsParserOutput::Boolean(r) => TokioAsyncReaderQueryResultsParserOutput::Boolean(r),
                TokioAsyncReaderXmlQueryResultsParserOutput::Solutions {
                    solutions,
                    variables,
                } => TokioAsyncReaderQueryResultsParserOutput::Solutions(TokioAsyncReaderSolutionsParser {
                    variables: variables.into(),
                    solutions: TokioAsyncReaderSolutionsParserKind::Xml(solutions),
                }),
            },
            QueryResultsFormat::Json => match TokioAsyncReaderJsonQueryResultsParserOutput::read(reader).await? {
                TokioAsyncReaderJsonQueryResultsParserOutput::Boolean(r) => TokioAsyncReaderQueryResultsParserOutput::Boolean(r),
                TokioAsyncReaderJsonQueryResultsParserOutput::Solutions {
                    solutions,
                    variables,
                } => TokioAsyncReaderQueryResultsParserOutput::Solutions(TokioAsyncReaderSolutionsParser {
                    variables: variables.into(),
                    solutions: TokioAsyncReaderSolutionsParserKind::Json(solutions),
                }),
            },
            QueryResultsFormat::Csv => return Err(QueryResultsSyntaxError::msg("CSV SPARQL results syntax is lossy and can't be parsed to a proper RDF representation").into()),
            QueryResultsFormat::Tsv => match TokioAsyncReaderTsvQueryResultsParserOutput::read(reader).await? {
                TokioAsyncReaderTsvQueryResultsParserOutput::Boolean(r) => TokioAsyncReaderQueryResultsParserOutput::Boolean(r),
                TokioAsyncReaderTsvQueryResultsParserOutput::Solutions {
                    solutions,
                    variables,
                } => TokioAsyncReaderQueryResultsParserOutput::Solutions(TokioAsyncReaderSolutionsParser {
                    variables: variables.into(),
                    solutions: TokioAsyncReaderSolutionsParserKind::Tsv(solutions),
                }),
            },
        })
    }

    /// Reads a result file from a [`Read`] implementation.
    ///
    /// Reads are automatically buffered.
    ///
    /// Example in XML (the API is the same for JSON and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, SliceQueryResultsParserOutput};
    /// use oxrdf::{Literal, Variable};
    ///
    /// let xml_parser = QueryResultsParser::from_format(QueryResultsFormat::Xml);
    ///
    /// // boolean
    /// if let SliceQueryResultsParserOutput::Boolean(v) = xml_parser.clone().for_slice(r#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head/><boolean>true</boolean></sparql>"#)? {
    ///     assert_eq!(v, true);
    /// }
    ///
    /// // solutions
    /// if let SliceQueryResultsParserOutput::Solutions(solutions) = xml_parser.for_slice(r#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head><variable name="foo"/><variable name="bar"/></head><results><result><binding name="foo"><literal>test</literal></binding></result></results></sparql>"#)? {
    ///     assert_eq!(solutions.variables(), &[Variable::new("foo")?, Variable::new("bar")?]);
    ///     for solution in solutions {
    ///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new("foo")?, &Literal::from("test").into())]);
    ///     }
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_slice(
        self,
        slice: &(impl AsRef<[u8]> + ?Sized),
    ) -> Result<SliceQueryResultsParserOutput<'_>, QueryResultsSyntaxError> {
        Ok(match self.format {
            QueryResultsFormat::Xml => {
                match SliceXmlQueryResultsParserOutput::read(slice.as_ref())? {
                    SliceXmlQueryResultsParserOutput::Boolean(r) => {
                        SliceQueryResultsParserOutput::Boolean(r)
                    }
                    SliceXmlQueryResultsParserOutput::Solutions {
                        solutions,
                        variables,
                    } => SliceQueryResultsParserOutput::Solutions(SliceSolutionsParser {
                        variables: variables.into(),
                        solutions: SliceSolutionsParserKind::Xml(solutions),
                    }),
                }
            }
            QueryResultsFormat::Json => {
                match SliceJsonQueryResultsParserOutput::read(slice.as_ref())? {
                    SliceJsonQueryResultsParserOutput::Boolean(r) => {
                        SliceQueryResultsParserOutput::Boolean(r)
                    }
                    SliceJsonQueryResultsParserOutput::Solutions {
                        solutions,
                        variables,
                    } => SliceQueryResultsParserOutput::Solutions(SliceSolutionsParser {
                        variables: variables.into(),
                        solutions: SliceSolutionsParserKind::Json(solutions),
                    }),
                }
            }
            QueryResultsFormat::Csv => {
                return Err(QueryResultsSyntaxError::msg(
                    "CSV SPARQL results syntax is lossy and can't be parsed to a proper RDF representation",
                ));
            }
            QueryResultsFormat::Tsv => {
                match SliceTsvQueryResultsParserOutput::read(slice.as_ref())? {
                    SliceTsvQueryResultsParserOutput::Boolean(r) => {
                        SliceQueryResultsParserOutput::Boolean(r)
                    }
                    SliceTsvQueryResultsParserOutput::Solutions {
                        solutions,
                        variables,
                    } => SliceQueryResultsParserOutput::Solutions(SliceSolutionsParser {
                        variables: variables.into(),
                        solutions: SliceSolutionsParserKind::Tsv(solutions),
                    }),
                }
            }
        })
    }
}

impl From<QueryResultsFormat> for QueryResultsParser {
    fn from(format: QueryResultsFormat) -> Self {
        Self::from_format(format)
    }
}

/// The reader for a given read of a results file.
///
/// It is either a read boolean ([`bool`]) or a streaming reader of a set of solutions ([`ReaderSolutionsParser`]).
///
/// Example in TSV (the API is the same for JSON and XML):
/// ```
/// use oxrdf::{Literal, Variable};
/// use sparesults::{QueryResultsFormat, QueryResultsParser, ReaderQueryResultsParserOutput};
///
/// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
///
/// // boolean
/// if let ReaderQueryResultsParserOutput::Boolean(v) =
///     tsv_parser.clone().for_reader("true".as_bytes())?
/// {
///     assert_eq!(v, true);
/// }
///
/// // solutions
/// if let ReaderQueryResultsParserOutput::Solutions(solutions) =
///     tsv_parser.for_reader("?foo\t?bar\n\"test\"\t".as_bytes())?
/// {
///     assert_eq!(
///         solutions.variables(),
///         &[Variable::new("foo")?, Variable::new("bar")?]
///     );
///     for solution in solutions {
///         assert_eq!(
///             solution?.iter().collect::<Vec<_>>(),
///             vec![(&Variable::new("foo")?, &Literal::from("test").into())]
///         );
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub enum ReaderQueryResultsParserOutput<R: Read> {
    Solutions(ReaderSolutionsParser<R>),
    Boolean(bool),
}

/// A streaming parser of a set of [`QuerySolution`] solutions.
///
/// It implements the [`Iterator`] API to iterate over the solutions.
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, ReaderQueryResultsParserOutput};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// if let ReaderQueryResultsParserOutput::Solutions(solutions) = json_parser.for_reader(br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#.as_slice())? {
///     assert_eq!(solutions.variables(), &[Variable::new("foo")?, Variable::new("bar")?]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new("foo")?, &Literal::from("test").into())]);
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct ReaderSolutionsParser<R: Read> {
    variables: Arc<[Variable]>,
    solutions: ReaderSolutionsParserKind<R>,
}

enum ReaderSolutionsParserKind<R: Read> {
    Xml(ReaderXmlSolutionsParser<R>),
    Json(ReaderJsonSolutionsParser<R>),
    Tsv(ReaderTsvSolutionsParser<R>),
}

impl<R: Read> ReaderSolutionsParser<R> {
    /// Ordered list of the declared variables at the beginning of the results.
    ///
    /// Example in TSV (the API is the same for JSON and XML):
    /// ```
    /// use oxrdf::Variable;
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, ReaderQueryResultsParserOutput};
    ///
    /// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
    /// if let ReaderQueryResultsParserOutput::Solutions(solutions) =
    ///     tsv_parser.for_reader(b"?foo\t?bar\n\"ex1\"\t\"ex2\"".as_slice())?
    /// {
    ///     assert_eq!(
    ///         solutions.variables(),
    ///         &[Variable::new("foo")?, Variable::new("bar")?]
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl<R: Read> Iterator for ReaderSolutionsParser<R> {
    type Item = Result<QuerySolution, QueryResultsParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(
            match &mut self.solutions {
                ReaderSolutionsParserKind::Xml(reader) => reader.parse_next(),
                ReaderSolutionsParserKind::Json(reader) => reader.parse_next(),
                ReaderSolutionsParserKind::Tsv(reader) => reader.parse_next(),
            }
            .transpose()?
            .map(|values| (Arc::clone(&self.variables), values).into()),
        )
    }
}

/// The reader for a given read of a results file.
///
/// It is either a read boolean ([`bool`]) or a streaming reader of a set of solutions ([`ReaderSolutionsParser`]).
///
/// Example in TSV (the API is the same for JSON and XML):
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdf::{Literal, Variable};
/// use sparesults::{
///     QueryResultsFormat, QueryResultsParser, TokioAsyncReaderQueryResultsParserOutput,
/// };
///
/// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
///
/// // boolean
/// if let TokioAsyncReaderQueryResultsParserOutput::Boolean(v) = tsv_parser
///     .clone()
///     .for_tokio_async_reader(b"true".as_slice())
///     .await?
/// {
///     assert_eq!(v, true);
/// }
///
/// // solutions
/// if let TokioAsyncReaderQueryResultsParserOutput::Solutions(mut solutions) = tsv_parser
///     .for_tokio_async_reader(b"?foo\t?bar\n\"test\"\t".as_slice())
///     .await?
/// {
///     assert_eq!(
///         solutions.variables(),
///         &[Variable::new("foo")?, Variable::new("bar")?]
///     );
///     while let Some(solution) = solutions.next().await {
///         assert_eq!(
///             solution?.iter().collect::<Vec<_>>(),
///             vec![(&Variable::new("foo")?, &Literal::from("test").into())]
///         );
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
pub enum TokioAsyncReaderQueryResultsParserOutput<R: AsyncRead + Unpin> {
    Solutions(TokioAsyncReaderSolutionsParser<R>),
    Boolean(bool),
}

/// A streaming parser of a set of [`QuerySolution`] solutions.
///
/// It implements the [`Iterator`] API to iterate over the solutions.
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use sparesults::{QueryResultsFormat, QueryResultsParser, TokioAsyncReaderQueryResultsParserOutput};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// if let TokioAsyncReaderQueryResultsParserOutput::Solutions(mut solutions) = json_parser.for_tokio_async_reader(br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#.as_slice()).await? {
///     assert_eq!(solutions.variables(), &[Variable::new("foo")?, Variable::new("bar")?]);
///     while let Some(solution) = solutions.next().await {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new("foo")?, &Literal::from("test").into())]);
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
pub struct TokioAsyncReaderSolutionsParser<R: AsyncRead + Unpin> {
    variables: Arc<[Variable]>,
    solutions: TokioAsyncReaderSolutionsParserKind<R>,
}

#[cfg(feature = "async-tokio")]
enum TokioAsyncReaderSolutionsParserKind<R: AsyncRead + Unpin> {
    Json(TokioAsyncReaderJsonSolutionsParser<R>),
    Xml(TokioAsyncReaderXmlSolutionsParser<R>),
    Tsv(TokioAsyncReaderTsvSolutionsParser<R>),
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> TokioAsyncReaderSolutionsParser<R> {
    /// Ordered list of the declared variables at the beginning of the results.
    ///
    /// Example in TSV (the API is the same for JSON and XML):
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdf::Variable;
    /// use sparesults::{
    ///     QueryResultsFormat, QueryResultsParser, TokioAsyncReaderQueryResultsParserOutput,
    /// };
    ///
    /// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
    /// if let TokioAsyncReaderQueryResultsParserOutput::Solutions(solutions) = tsv_parser
    ///     .for_tokio_async_reader(b"?foo\t?bar\n\"ex1\"\t\"ex2\"".as_slice())
    ///     .await?
    /// {
    ///     assert_eq!(
    ///         solutions.variables(),
    ///         &[Variable::new("foo")?, Variable::new("bar")?]
    ///     );
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }

    /// Reads the next solution or returns `None` if the file is finished.
    pub async fn next(&mut self) -> Option<Result<QuerySolution, QueryResultsParseError>> {
        Some(
            match &mut self.solutions {
                TokioAsyncReaderSolutionsParserKind::Json(reader) => reader.parse_next().await,
                TokioAsyncReaderSolutionsParserKind::Xml(reader) => reader.parse_next().await,
                TokioAsyncReaderSolutionsParserKind::Tsv(reader) => reader.parse_next().await,
            }
            .transpose()?
            .map(|values| (Arc::clone(&self.variables), values).into()),
        )
    }
}

/// The reader for a given read of a results file.
///
/// It is either a read boolean ([`bool`]) or a streaming reader of a set of solutions ([`SliceSolutionsParser`]).
///
/// Example in TSV (the API is the same for JSON and XML):
/// ```
/// use oxrdf::{Literal, Variable};
/// use sparesults::{QueryResultsFormat, QueryResultsParser, ReaderQueryResultsParserOutput};
///
/// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
///
/// // boolean
/// if let ReaderQueryResultsParserOutput::Boolean(v) =
///     tsv_parser.clone().for_reader(b"true".as_slice())?
/// {
///     assert_eq!(v, true);
/// }
///
/// // solutions
/// if let ReaderQueryResultsParserOutput::Solutions(solutions) =
///     tsv_parser.for_reader(b"?foo\t?bar\n\"test\"\t".as_slice())?
/// {
///     assert_eq!(
///         solutions.variables(),
///         &[Variable::new("foo")?, Variable::new("bar")?]
///     );
///     for solution in solutions {
///         assert_eq!(
///             solution?.iter().collect::<Vec<_>>(),
///             vec![(&Variable::new("foo")?, &Literal::from("test").into())]
///         );
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub enum SliceQueryResultsParserOutput<'a> {
    Solutions(SliceSolutionsParser<'a>),
    Boolean(bool),
}

/// A streaming parser of a set of [`QuerySolution`] solutions.
///
/// It implements the [`Iterator`] API to iterate over the solutions.
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, SliceQueryResultsParserOutput};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// if let SliceQueryResultsParserOutput::Solutions(solutions) = json_parser.for_slice(r#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#)? {
///     assert_eq!(solutions.variables(), &[Variable::new("foo")?, Variable::new("bar")?]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new("foo")?, &Literal::from("test").into())]);
///     }
/// }
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct SliceSolutionsParser<'a> {
    variables: Arc<[Variable]>,
    solutions: SliceSolutionsParserKind<'a>,
}

enum SliceSolutionsParserKind<'a> {
    Xml(SliceXmlSolutionsParser<'a>),
    Json(SliceJsonSolutionsParser<'a>),
    Tsv(SliceTsvSolutionsParser<'a>),
}

impl SliceSolutionsParser<'_> {
    /// Ordered list of the declared variables at the beginning of the results.
    ///
    /// Example in TSV (the API is the same for JSON and XML):
    /// ```
    /// use oxrdf::Variable;
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, SliceQueryResultsParserOutput};
    ///
    /// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
    /// if let SliceQueryResultsParserOutput::Solutions(solutions) =
    ///     tsv_parser.for_slice("?foo\t?bar\n\"ex1\"\t\"ex2\"")?
    /// {
    ///     assert_eq!(
    ///         solutions.variables(),
    ///         &[Variable::new("foo")?, Variable::new("bar")?]
    ///     );
    /// }
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl Iterator for SliceSolutionsParser<'_> {
    type Item = Result<QuerySolution, QueryResultsSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(
            match &mut self.solutions {
                SliceSolutionsParserKind::Xml(reader) => reader.parse_next(),
                SliceSolutionsParserKind::Json(reader) => reader.parse_next(),
                SliceSolutionsParserKind::Tsv(reader) => reader.parse_next(),
            }
            .transpose()?
            .map(|values| (Arc::clone(&self.variables), values).into()),
        )
    }
}
