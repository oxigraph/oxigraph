use crate::csv::{
    FromReadTsvQueryResultsReader, FromReadTsvSolutionsReader, FromSliceTsvQueryResultsReader,
    FromSliceTsvSolutionsReader,
};
#[cfg(feature = "async-tokio")]
use crate::csv::{FromTokioAsyncReadTsvQueryResultsReader, FromTokioAsyncReadTsvSolutionsReader};
use crate::error::{QueryResultsParseError, QueryResultsSyntaxError};
use crate::format::QueryResultsFormat;
use crate::json::{
    FromReadJsonQueryResultsReader, FromReadJsonSolutionsReader, FromSliceJsonQueryResultsReader,
    FromSliceJsonSolutionsReader,
};
#[cfg(feature = "async-tokio")]
use crate::json::{
    FromTokioAsyncReadJsonQueryResultsReader, FromTokioAsyncReadJsonSolutionsReader,
};
use crate::solution::QuerySolution;
use crate::xml::{
    FromReadXmlQueryResultsReader, FromReadXmlSolutionsReader, FromSliceXmlQueryResultsReader,
    FromSliceXmlSolutionsReader,
};
#[cfg(feature = "async-tokio")]
use crate::xml::{FromTokioAsyncReadXmlQueryResultsReader, FromTokioAsyncReadXmlSolutionsReader};
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
/// use sparesults::{QueryResultsFormat, QueryResultsParser, FromReadQueryResultsReader};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// // boolean
/// if let FromReadQueryResultsReader::Boolean(v) = json_parser.clone().parse_read(br#"{"boolean":true}"#.as_slice())? {
///     assert_eq!(v, true);
/// }
/// // solutions
/// if let FromReadQueryResultsReader::Solutions(solutions) = json_parser.parse_read(br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#.as_slice())? {
///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
///     }
/// }
/// # Result::<(),sparesults::QueryResultsParseError>::Ok(())
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
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, FromReadQueryResultsReader};
    /// use oxrdf::{Literal, Variable};
    ///
    /// let xml_parser = QueryResultsParser::from_format(QueryResultsFormat::Xml);
    ///
    /// // boolean
    /// if let FromReadQueryResultsReader::Boolean(v) = xml_parser.clone().parse_read(br#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head/><boolean>true</boolean></sparql>"#.as_slice())? {
    ///     assert_eq!(v, true);
    /// }
    ///
    /// // solutions
    /// if let FromReadQueryResultsReader::Solutions(solutions) = xml_parser.parse_read(br#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head><variable name="foo"/><variable name="bar"/></head><results><result><binding name="foo"><literal>test</literal></binding></result></results></sparql>"#.as_slice())? {
    ///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
    ///     for solution in solutions {
    ///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
    ///     }
    /// }
    /// # Result::<(),sparesults::QueryResultsParseError>::Ok(())
    /// ```
    pub fn parse_read<R: Read>(
        self,
        reader: R,
    ) -> Result<FromReadQueryResultsReader<R>, QueryResultsParseError> {
        Ok(match self.format {
            QueryResultsFormat::Xml => match FromReadXmlQueryResultsReader::read(reader)? {
                FromReadXmlQueryResultsReader::Boolean(r) => FromReadQueryResultsReader::Boolean(r),
                FromReadXmlQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromReadQueryResultsReader::Solutions(FromReadSolutionsReader {
                    variables: variables.into(),
                    solutions: FromReadSolutionsReaderKind::Xml(solutions),
                }),
            },
            QueryResultsFormat::Json => match FromReadJsonQueryResultsReader::read(reader)? {
                FromReadJsonQueryResultsReader::Boolean(r) => FromReadQueryResultsReader::Boolean(r),
                FromReadJsonQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromReadQueryResultsReader::Solutions(FromReadSolutionsReader {
                    variables: variables.into(),
                    solutions: FromReadSolutionsReaderKind::Json(solutions),
                }),
            },
            QueryResultsFormat::Csv => return Err(QueryResultsSyntaxError::msg("CSV SPARQL results syntax is lossy and can't be parsed to a proper RDF representation").into()),
            QueryResultsFormat::Tsv => match FromReadTsvQueryResultsReader::read(reader)? {
                FromReadTsvQueryResultsReader::Boolean(r) => FromReadQueryResultsReader::Boolean(r),
                FromReadTsvQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromReadQueryResultsReader::Solutions(FromReadSolutionsReader {
                    variables: variables.into(),
                    solutions: FromReadSolutionsReaderKind::Tsv(solutions),
                }),
            },
        })
    }

    #[deprecated(note = "use parse_read", since = "0.4.0")]
    pub fn read_results<R: Read>(
        &self,
        reader: R,
    ) -> Result<FromReadQueryResultsReader<R>, QueryResultsParseError> {
        self.clone().parse_read(reader)
    }

    /// Reads a result file from a Tokio [`AsyncRead`] implementation.
    ///
    /// Reads are automatically buffered.
    ///
    /// Example in XML (the API is the same for JSON and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, FromTokioAsyncReadQueryResultsReader};
    /// use oxrdf::{Literal, Variable};
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), sparesults::QueryResultsParseError> {
    /// let xml_parser = QueryResultsParser::from_format(QueryResultsFormat::Xml);
    ///
    /// // boolean
    /// if let FromTokioAsyncReadQueryResultsReader::Boolean(v) = xml_parser.clone().parse_tokio_async_read(br#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head/><boolean>true</boolean></sparql>"#.as_slice()).await? {
    ///     assert_eq!(v, true);
    /// }
    ///
    /// // solutions
    /// if let FromTokioAsyncReadQueryResultsReader::Solutions(mut solutions) = xml_parser.parse_tokio_async_read(br#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head><variable name="foo"/><variable name="bar"/></head><results><result><binding name="foo"><literal>test</literal></binding></result></results></sparql>"#.as_slice()).await? {
    ///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
    ///     while let Some(solution) = solutions.next().await {
    ///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub async fn parse_tokio_async_read<R: AsyncRead + Unpin>(
        self,
        reader: R,
    ) -> Result<FromTokioAsyncReadQueryResultsReader<R>, QueryResultsParseError> {
        Ok(match self.format {
            QueryResultsFormat::Xml => match FromTokioAsyncReadXmlQueryResultsReader::read(reader).await? {
                FromTokioAsyncReadXmlQueryResultsReader::Boolean(r) => FromTokioAsyncReadQueryResultsReader::Boolean(r),
                FromTokioAsyncReadXmlQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromTokioAsyncReadQueryResultsReader::Solutions(FromTokioAsyncReadSolutionsReader {
                    variables: variables.into(),
                    solutions: FromTokioAsyncReadSolutionsReaderKind::Xml(solutions),
                }),
            },
            QueryResultsFormat::Json => match FromTokioAsyncReadJsonQueryResultsReader::read(reader).await? {
                FromTokioAsyncReadJsonQueryResultsReader::Boolean(r) => FromTokioAsyncReadQueryResultsReader::Boolean(r),
                FromTokioAsyncReadJsonQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromTokioAsyncReadQueryResultsReader::Solutions(FromTokioAsyncReadSolutionsReader {
                    variables: variables.into(),
                    solutions: FromTokioAsyncReadSolutionsReaderKind::Json(solutions),
                }),
            },
            QueryResultsFormat::Csv => return Err(QueryResultsSyntaxError::msg("CSV SPARQL results syntax is lossy and can't be parsed to a proper RDF representation").into()),
            QueryResultsFormat::Tsv => match FromTokioAsyncReadTsvQueryResultsReader::read(reader).await? {
                FromTokioAsyncReadTsvQueryResultsReader::Boolean(r) => FromTokioAsyncReadQueryResultsReader::Boolean(r),
                FromTokioAsyncReadTsvQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromTokioAsyncReadQueryResultsReader::Solutions(FromTokioAsyncReadSolutionsReader {
                    variables: variables.into(),
                    solutions: FromTokioAsyncReadSolutionsReaderKind::Tsv(solutions),
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
    /// use sparesults::{QueryResultsFormat, QueryResultsParser, FromSliceQueryResultsReader};
    /// use oxrdf::{Literal, Variable};
    ///
    /// let xml_parser = QueryResultsParser::from_format(QueryResultsFormat::Xml);
    ///
    /// // boolean
    /// if let FromSliceQueryResultsReader::Boolean(v) = xml_parser.clone().parse_slice(br#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head/><boolean>true</boolean></sparql>"#)? {
    ///     assert_eq!(v, true);
    /// }
    ///
    /// // solutions
    /// if let FromSliceQueryResultsReader::Solutions(solutions) = xml_parser.parse_slice(br#"<sparql xmlns="http://www.w3.org/2005/sparql-results#"><head><variable name="foo"/><variable name="bar"/></head><results><result><binding name="foo"><literal>test</literal></binding></result></results></sparql>"#)? {
    ///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
    ///     for solution in solutions {
    ///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
    ///     }
    /// }
    /// # Result::<(),sparesults::QueryResultsParseError>::Ok(())
    /// ```
    pub fn parse_slice(
        self,
        slice: &[u8],
    ) -> Result<FromSliceQueryResultsReader<'_>, QueryResultsSyntaxError> {
        Ok(match self.format {
            QueryResultsFormat::Xml => match FromSliceXmlQueryResultsReader::read(slice)? {
                FromSliceXmlQueryResultsReader::Boolean(r) => FromSliceQueryResultsReader::Boolean(r),
                FromSliceXmlQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromSliceQueryResultsReader::Solutions(FromSliceSolutionsReader {
                    variables: variables.into(),
                    solutions: FromSliceSolutionsReaderKind::Xml(solutions),
                }),
            },
            QueryResultsFormat::Json => match FromSliceJsonQueryResultsReader::read(slice)? {
                FromSliceJsonQueryResultsReader::Boolean(r) => FromSliceQueryResultsReader::Boolean(r),
                FromSliceJsonQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromSliceQueryResultsReader::Solutions(FromSliceSolutionsReader {
                    variables: variables.into(),
                    solutions: FromSliceSolutionsReaderKind::Json(solutions),
                }),
            },
            QueryResultsFormat::Csv => return Err(QueryResultsSyntaxError::msg("CSV SPARQL results syntax is lossy and can't be parsed to a proper RDF representation")),
            QueryResultsFormat::Tsv => match FromSliceTsvQueryResultsReader::read(slice)? {
                FromSliceTsvQueryResultsReader::Boolean(r) => FromSliceQueryResultsReader::Boolean(r),
                FromSliceTsvQueryResultsReader::Solutions {
                    solutions,
                    variables,
                } => FromSliceQueryResultsReader::Solutions(FromSliceSolutionsReader {
                    variables: variables.into(),
                    solutions: FromSliceSolutionsReaderKind::Tsv(solutions),
                }),
            },
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
/// It is either a read boolean ([`bool`]) or a streaming reader of a set of solutions ([`FromReadSolutionsReader`]).
///
/// Example in TSV (the API is the same for JSON and XML):
/// ```
/// use oxrdf::{Literal, Variable};
/// use sparesults::{FromReadQueryResultsReader, QueryResultsFormat, QueryResultsParser};
///
/// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
///
/// // boolean
/// if let FromReadQueryResultsReader::Boolean(v) =
///     tsv_parser.clone().parse_read(b"true".as_slice())?
/// {
///     assert_eq!(v, true);
/// }
///
/// // solutions
/// if let FromReadQueryResultsReader::Solutions(solutions) =
///     tsv_parser.parse_read(b"?foo\t?bar\n\"test\"\t".as_slice())?
/// {
///     assert_eq!(
///         solutions.variables(),
///         &[
///             Variable::new_unchecked("foo"),
///             Variable::new_unchecked("bar")
///         ]
///     );
///     for solution in solutions {
///         assert_eq!(
///             solution?.iter().collect::<Vec<_>>(),
///             vec![(
///                 &Variable::new_unchecked("foo"),
///                 &Literal::from("test").into()
///             )]
///         );
///     }
/// }
/// # Result::<(),sparesults::QueryResultsParseError>::Ok(())
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
/// if let FromReadQueryResultsReader::Solutions(solutions) = json_parser.parse_read(br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#.as_slice())? {
///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
///     }
/// }
/// # Result::<(),sparesults::QueryResultsParseError>::Ok(())
/// ```
pub struct FromReadSolutionsReader<R: Read> {
    variables: Arc<[Variable]>,
    solutions: FromReadSolutionsReaderKind<R>,
}

enum FromReadSolutionsReaderKind<R: Read> {
    Xml(FromReadXmlSolutionsReader<R>),
    Json(FromReadJsonSolutionsReader<R>),
    Tsv(FromReadTsvSolutionsReader<R>),
}

impl<R: Read> FromReadSolutionsReader<R> {
    /// Ordered list of the declared variables at the beginning of the results.
    ///
    /// Example in TSV (the API is the same for JSON and XML):
    /// ```
    /// use oxrdf::Variable;
    /// use sparesults::{FromReadQueryResultsReader, QueryResultsFormat, QueryResultsParser};
    ///
    /// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
    /// if let FromReadQueryResultsReader::Solutions(solutions) =
    ///     tsv_parser.parse_read(b"?foo\t?bar\n\"ex1\"\t\"ex2\"".as_slice())?
    /// {
    ///     assert_eq!(
    ///         solutions.variables(),
    ///         &[
    ///             Variable::new_unchecked("foo"),
    ///             Variable::new_unchecked("bar")
    ///         ]
    ///     );
    /// }
    /// # Result::<(),sparesults::QueryResultsParseError>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl<R: Read> Iterator for FromReadSolutionsReader<R> {
    type Item = Result<QuerySolution, QueryResultsParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(
            match &mut self.solutions {
                FromReadSolutionsReaderKind::Xml(reader) => reader.read_next(),
                FromReadSolutionsReaderKind::Json(reader) => reader.read_next(),
                FromReadSolutionsReaderKind::Tsv(reader) => reader.read_next(),
            }
            .transpose()?
            .map(|values| (Arc::clone(&self.variables), values).into()),
        )
    }
}

/// The reader for a given read of a results file.
///
/// It is either a read boolean ([`bool`]) or a streaming reader of a set of solutions ([`FromReadSolutionsReader`]).
///
/// Example in TSV (the API is the same for JSON and XML):
/// ```
/// use oxrdf::{Literal, Variable};
/// use sparesults::{
///     FromTokioAsyncReadQueryResultsReader, QueryResultsFormat, QueryResultsParser,
/// };
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), sparesults::QueryResultsParseError> {
/// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
///
/// // boolean
/// if let FromTokioAsyncReadQueryResultsReader::Boolean(v) = tsv_parser
///     .clone()
///     .parse_tokio_async_read(b"true".as_slice())
///     .await?
/// {
///     assert_eq!(v, true);
/// }
///
/// // solutions
/// if let FromTokioAsyncReadQueryResultsReader::Solutions(mut solutions) = tsv_parser
///     .parse_tokio_async_read(b"?foo\t?bar\n\"test\"\t".as_slice())
///     .await?
/// {
///     assert_eq!(
///         solutions.variables(),
///         &[
///             Variable::new_unchecked("foo"),
///             Variable::new_unchecked("bar")
///         ]
///     );
///     while let Some(solution) = solutions.next().await {
///         assert_eq!(
///             solution?.iter().collect::<Vec<_>>(),
///             vec![(
///                 &Variable::new_unchecked("foo"),
///                 &Literal::from("test").into()
///             )]
///         );
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
pub enum FromTokioAsyncReadQueryResultsReader<R: AsyncRead + Unpin> {
    Solutions(FromTokioAsyncReadSolutionsReader<R>),
    Boolean(bool),
}

/// A streaming reader of a set of [`QuerySolution`] solutions.
///
/// It implements the [`Iterator`] API to iterate over the solutions.
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, FromTokioAsyncReadQueryResultsReader};
/// use oxrdf::{Literal, Variable};
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), sparesults::QueryResultsParseError> {
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// if let FromTokioAsyncReadQueryResultsReader::Solutions(mut solutions) = json_parser.parse_tokio_async_read(br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#.as_slice()).await? {
///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
///     while let Some(solution) = solutions.next().await {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
pub struct FromTokioAsyncReadSolutionsReader<R: AsyncRead + Unpin> {
    variables: Arc<[Variable]>,
    solutions: FromTokioAsyncReadSolutionsReaderKind<R>,
}

#[cfg(feature = "async-tokio")]
enum FromTokioAsyncReadSolutionsReaderKind<R: AsyncRead + Unpin> {
    Json(FromTokioAsyncReadJsonSolutionsReader<R>),
    Xml(FromTokioAsyncReadXmlSolutionsReader<R>),
    Tsv(FromTokioAsyncReadTsvSolutionsReader<R>),
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> FromTokioAsyncReadSolutionsReader<R> {
    /// Ordered list of the declared variables at the beginning of the results.
    ///
    /// Example in TSV (the API is the same for JSON and XML):
    /// ```
    /// use oxrdf::Variable;
    /// use sparesults::{
    ///     FromTokioAsyncReadQueryResultsReader, QueryResultsFormat, QueryResultsParser,
    /// };
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), sparesults::QueryResultsParseError> {
    /// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
    /// if let FromTokioAsyncReadQueryResultsReader::Solutions(solutions) = tsv_parser
    ///     .parse_tokio_async_read(b"?foo\t?bar\n\"ex1\"\t\"ex2\"".as_slice())
    ///     .await?
    /// {
    ///     assert_eq!(
    ///         solutions.variables(),
    ///         &[
    ///             Variable::new_unchecked("foo"),
    ///             Variable::new_unchecked("bar")
    ///         ]
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
                FromTokioAsyncReadSolutionsReaderKind::Json(reader) => reader.read_next().await,
                FromTokioAsyncReadSolutionsReaderKind::Xml(reader) => reader.read_next().await,
                FromTokioAsyncReadSolutionsReaderKind::Tsv(reader) => reader.read_next().await,
            }
            .transpose()?
            .map(|values| (Arc::clone(&self.variables), values).into()),
        )
    }
}

/// The reader for a given read of a results file.
///
/// It is either a read boolean ([`bool`]) or a streaming reader of a set of solutions ([`FromSliceSolutionsReader`]).
///
/// Example in TSV (the API is the same for JSON and XML):
/// ```
/// use oxrdf::{Literal, Variable};
/// use sparesults::{FromReadQueryResultsReader, QueryResultsFormat, QueryResultsParser};
///
/// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
///
/// // boolean
/// if let FromReadQueryResultsReader::Boolean(v) =
///     tsv_parser.clone().parse_read(b"true".as_slice())?
/// {
///     assert_eq!(v, true);
/// }
///
/// // solutions
/// if let FromReadQueryResultsReader::Solutions(solutions) =
///     tsv_parser.parse_read(b"?foo\t?bar\n\"test\"\t".as_slice())?
/// {
///     assert_eq!(
///         solutions.variables(),
///         &[
///             Variable::new_unchecked("foo"),
///             Variable::new_unchecked("bar")
///         ]
///     );
///     for solution in solutions {
///         assert_eq!(
///             solution?.iter().collect::<Vec<_>>(),
///             vec![(
///                 &Variable::new_unchecked("foo"),
///                 &Literal::from("test").into()
///             )]
///         );
///     }
/// }
/// # Result::<(),sparesults::QueryResultsParseError>::Ok(())
/// ```
pub enum FromSliceQueryResultsReader<'a> {
    Solutions(FromSliceSolutionsReader<'a>),
    Boolean(bool),
}

/// A streaming reader of a set of [`QuerySolution`] solutions.
///
/// It implements the [`Iterator`] API to iterate over the solutions.
///
/// Example in JSON (the API is the same for XML and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsParser, FromSliceQueryResultsReader};
/// use oxrdf::{Literal, Variable};
///
/// let json_parser = QueryResultsParser::from_format(QueryResultsFormat::Json);
/// if let FromSliceQueryResultsReader::Solutions(solutions) = json_parser.parse_slice(br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#)? {
///     assert_eq!(solutions.variables(), &[Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]);
///     for solution in solutions {
///         assert_eq!(solution?.iter().collect::<Vec<_>>(), vec![(&Variable::new_unchecked("foo"), &Literal::from("test").into())]);
///     }
/// }
/// # Result::<(),sparesults::QueryResultsParseError>::Ok(())
/// ```
pub struct FromSliceSolutionsReader<'a> {
    variables: Arc<[Variable]>,
    solutions: FromSliceSolutionsReaderKind<'a>,
}

enum FromSliceSolutionsReaderKind<'a> {
    Xml(FromSliceXmlSolutionsReader<'a>),
    Json(FromSliceJsonSolutionsReader<'a>),
    Tsv(FromSliceTsvSolutionsReader<'a>),
}

impl<'a> FromSliceSolutionsReader<'a> {
    /// Ordered list of the declared variables at the beginning of the results.
    ///
    /// Example in TSV (the API is the same for JSON and XML):
    /// ```
    /// use oxrdf::Variable;
    /// use sparesults::{FromSliceQueryResultsReader, QueryResultsFormat, QueryResultsParser};
    ///
    /// let tsv_parser = QueryResultsParser::from_format(QueryResultsFormat::Tsv);
    /// if let FromSliceQueryResultsReader::Solutions(solutions) =
    ///     tsv_parser.parse_slice(b"?foo\t?bar\n\"ex1\"\t\"ex2\"")?
    /// {
    ///     assert_eq!(
    ///         solutions.variables(),
    ///         &[
    ///             Variable::new_unchecked("foo"),
    ///             Variable::new_unchecked("bar")
    ///         ]
    ///     );
    /// }
    /// # Result::<(),sparesults::QueryResultsParseError>::Ok(())
    /// ```
    #[inline]
    pub fn variables(&self) -> &[Variable] {
        &self.variables
    }
}

impl<'a> Iterator for FromSliceSolutionsReader<'a> {
    type Item = Result<QuerySolution, QueryResultsSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(
            match &mut self.solutions {
                FromSliceSolutionsReaderKind::Xml(reader) => reader.read_next(),
                FromSliceSolutionsReaderKind::Json(reader) => reader.read_next(),
                FromSliceSolutionsReaderKind::Tsv(reader) => reader.read_next(),
            }
            .transpose()?
            .map(|values| (Arc::clone(&self.variables), values).into()),
        )
    }
}
