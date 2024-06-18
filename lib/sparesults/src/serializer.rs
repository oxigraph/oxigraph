#[cfg(feature = "async-tokio")]
use crate::csv::{
    tokio_async_write_boolean_csv_result, ToTokioAsyncWriteCsvSolutionsWriter,
    ToTokioAsyncWriteTsvSolutionsWriter,
};
use crate::csv::{write_boolean_csv_result, ToWriteCsvSolutionsWriter, ToWriteTsvSolutionsWriter};
use crate::format::QueryResultsFormat;
#[cfg(feature = "async-tokio")]
use crate::json::{tokio_async_write_boolean_json_result, ToTokioAsyncWriteJsonSolutionsWriter};
use crate::json::{write_boolean_json_result, ToWriteJsonSolutionsWriter};
#[cfg(feature = "async-tokio")]
use crate::xml::{tokio_async_write_boolean_xml_result, ToTokioAsyncWriteXmlSolutionsWriter};
use crate::xml::{write_boolean_xml_result, ToWriteXmlSolutionsWriter};
use oxrdf::{TermRef, Variable, VariableRef};
use std::io::{self, Write};
#[cfg(feature = "async-tokio")]
use tokio::io::AsyncWrite;

/// A serializer for [SPARQL query](https://www.w3.org/TR/sparql11-query/) results serialization formats.
///
/// It currently supports the following formats:
/// * [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/) ([`QueryResultsFormat::Xml`](QueryResultsFormat::Xml))
/// * [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/) ([`QueryResultsFormat::Json`](QueryResultsFormat::Json))
/// * [SPARQL Query Results CSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/) ([`QueryResultsFormat::Csv`](QueryResultsFormat::Csv))
/// * [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/) ([`QueryResultsFormat::Tsv`](QueryResultsFormat::Tsv))
///
/// Example in JSON (the API is the same for XML, CSV and TSV):
/// ```
/// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
/// use oxrdf::{LiteralRef, Variable, VariableRef};
/// use std::iter::once;
///
/// let json_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Json);
///
/// // boolean
/// let mut buffer = Vec::new();
/// json_serializer.clone().serialize_boolean_to_write(&mut buffer, true)?;
/// assert_eq!(buffer, br#"{"head":{},"boolean":true}"#);
///
/// // solutions
/// let mut buffer = Vec::new();
/// let mut writer = json_serializer.serialize_solutions_to_write(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")])?;
/// writer.write(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test"))))?;
/// writer.finish()?;
/// assert_eq!(buffer, br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#);
/// # std::io::Result::Ok(())
/// ```
#[must_use]
#[derive(Clone)]
pub struct QueryResultsSerializer {
    format: QueryResultsFormat,
}

impl QueryResultsSerializer {
    /// Builds a serializer for the given format.
    #[inline]
    pub fn from_format(format: QueryResultsFormat) -> Self {
        Self { format }
    }

    /// Write a boolean query result (from an `ASK` query)  into the given [`Write`] implementation.
    ///
    /// Example in XML (the API is the same for JSON, CSV and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
    ///
    /// let xml_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Xml);
    /// let mut buffer = Vec::new();
    /// xml_serializer.serialize_boolean_to_write(&mut buffer, true)?;
    /// assert_eq!(buffer, br#"<?xml version="1.0"?><sparql xmlns="http://www.w3.org/2005/sparql-results#"><head></head><boolean>true</boolean></sparql>"#);
    /// # std::io::Result::Ok(())
    /// ```
    pub fn serialize_boolean_to_write<W: Write>(self, write: W, value: bool) -> io::Result<W> {
        match self.format {
            QueryResultsFormat::Xml => write_boolean_xml_result(write, value),
            QueryResultsFormat::Json => write_boolean_json_result(write, value),
            QueryResultsFormat::Csv | QueryResultsFormat::Tsv => {
                write_boolean_csv_result(write, value)
            }
        }
    }

    /// Write a boolean query result (from an `ASK` query)  into the given [`AsyncWrite`] implementation.
    ///
    /// Example in JSON (the API is the same for XML, CSV and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> std::io::Result<()> {
    /// let json_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Json);
    /// let mut buffer = Vec::new();
    /// json_serializer
    ///     .serialize_boolean_to_tokio_async_write(&mut buffer, false)
    ///     .await?;
    /// assert_eq!(buffer, br#"{"head":{},"boolean":false}"#);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub async fn serialize_boolean_to_tokio_async_write<W: AsyncWrite + Unpin>(
        self,
        write: W,
        value: bool,
    ) -> io::Result<W> {
        match self.format {
            QueryResultsFormat::Xml => tokio_async_write_boolean_xml_result(write, value).await,
            QueryResultsFormat::Json => tokio_async_write_boolean_json_result(write, value).await,
            QueryResultsFormat::Csv | QueryResultsFormat::Tsv => {
                tokio_async_write_boolean_csv_result(write, value).await
            }
        }
    }

    #[deprecated(note = "use serialize_boolean_to_write", since = "0.4.0")]
    pub fn write_boolean_result<W: Write>(&self, writer: W, value: bool) -> io::Result<W> {
        self.clone().serialize_boolean_to_write(writer, value)
    }

    /// Returns a `SolutionsWriter` allowing writing query solutions into the given [`Write`] implementation.
    ///
    /// <div class="warning">
    ///
    /// Do not forget to run the [`finish`](ToWriteSolutionsWriter::finish()) method to properly write the last bytes of the file.</div>
    ///
    /// <div class="warning">
    ///
    /// This writer does unbuffered writes. You might want to use [`BufWriter`](io::BufWriter) to avoid that.</div>
    ///
    /// Example in XML (the API is the same for JSON, CSV and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
    /// use oxrdf::{LiteralRef, Variable, VariableRef};
    /// use std::iter::once;
    ///
    /// let xml_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Xml);
    /// let mut buffer = Vec::new();
    /// let mut writer = xml_serializer.serialize_solutions_to_write(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")])?;
    /// writer.write(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test"))))?;
    /// writer.finish()?;
    /// assert_eq!(buffer, br#"<?xml version="1.0"?><sparql xmlns="http://www.w3.org/2005/sparql-results#"><head><variable name="foo"/><variable name="bar"/></head><results><result><binding name="foo"><literal>test</literal></binding></result></results></sparql>"#);
    /// # std::io::Result::Ok(())
    /// ```
    pub fn serialize_solutions_to_write<W: Write>(
        self,
        write: W,
        variables: Vec<Variable>,
    ) -> io::Result<ToWriteSolutionsWriter<W>> {
        Ok(ToWriteSolutionsWriter {
            formatter: match self.format {
                QueryResultsFormat::Xml => ToWriteSolutionsWriterKind::Xml(
                    ToWriteXmlSolutionsWriter::start(write, &variables)?,
                ),
                QueryResultsFormat::Json => ToWriteSolutionsWriterKind::Json(
                    ToWriteJsonSolutionsWriter::start(write, &variables)?,
                ),
                QueryResultsFormat::Csv => ToWriteSolutionsWriterKind::Csv(
                    ToWriteCsvSolutionsWriter::start(write, variables)?,
                ),
                QueryResultsFormat::Tsv => ToWriteSolutionsWriterKind::Tsv(
                    ToWriteTsvSolutionsWriter::start(write, variables)?,
                ),
            },
        })
    }

    /// Returns a `SolutionsWriter` allowing writing query solutions into the given [`Write`] implementation.
    ///
    /// <div class="warning">
    ///
    /// Do not forget to run the [`finish`](ToWriteSolutionsWriter::finish()) method to properly write the last bytes of the file.</div>
    ///
    /// <div class="warning">
    ///
    /// This writer does unbuffered writes. You might want to use [`BufWriter`](io::BufWriter) to avoid that.</div>
    ///
    /// Example in XML (the API is the same for JSON, CSV and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
    /// use oxrdf::{LiteralRef, Variable, VariableRef};
    /// use std::iter::once;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> std::io::Result<()> {
    /// let json_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Json);
    /// let mut buffer = Vec::new();
    /// let mut writer = json_serializer.serialize_solutions_to_tokio_async_write(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]).await?;
    /// writer.write(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test")))).await?;
    /// writer.finish().await?;
    /// assert_eq!(buffer, br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#);
    /// # Ok(())
    /// # }    
    /// ```
    #[cfg(feature = "async-tokio")]
    pub async fn serialize_solutions_to_tokio_async_write<W: AsyncWrite + Unpin>(
        self,
        write: W,
        variables: Vec<Variable>,
    ) -> io::Result<ToTokioAsyncWriteSolutionsWriter<W>> {
        Ok(ToTokioAsyncWriteSolutionsWriter {
            formatter: match self.format {
                QueryResultsFormat::Xml => ToTokioAsyncWriteSolutionsWriterKind::Xml(
                    ToTokioAsyncWriteXmlSolutionsWriter::start(write, &variables).await?,
                ),
                QueryResultsFormat::Json => ToTokioAsyncWriteSolutionsWriterKind::Json(
                    ToTokioAsyncWriteJsonSolutionsWriter::start(write, &variables).await?,
                ),
                QueryResultsFormat::Csv => ToTokioAsyncWriteSolutionsWriterKind::Csv(
                    ToTokioAsyncWriteCsvSolutionsWriter::start(write, variables).await?,
                ),
                QueryResultsFormat::Tsv => ToTokioAsyncWriteSolutionsWriterKind::Tsv(
                    ToTokioAsyncWriteTsvSolutionsWriter::start(write, variables).await?,
                ),
            },
        })
    }

    #[deprecated(note = "use serialize_solutions_to_write", since = "0.4.0")]
    pub fn solutions_writer<W: Write>(
        &self,
        writer: W,
        variables: Vec<Variable>,
    ) -> io::Result<ToWriteSolutionsWriter<W>> {
        Self {
            format: self.format,
        }
        .serialize_solutions_to_write(writer, variables)
    }
}

impl From<QueryResultsFormat> for QueryResultsSerializer {
    fn from(format: QueryResultsFormat) -> Self {
        Self::from_format(format)
    }
}

/// Allows writing query results into a [`Write`] implementation.
///
/// Could be built using a [`QueryResultsSerializer`].
///
/// <div class="warning">
///
/// Do not forget to run the [`finish`](ToWriteSolutionsWriter::finish()) method to properly write the last bytes of the file.</div>
///
/// <div class="warning">
///
/// This writer does unbuffered writes. You might want to use [`BufWriter`](io::BufWriter) to avoid that.</div>
///
/// Example in TSV (the API is the same for JSON, XML and CSV):
/// ```
/// use oxrdf::{LiteralRef, Variable, VariableRef};
/// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
/// use std::iter::once;
///
/// let tsv_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Tsv);
/// let mut buffer = Vec::new();
/// let mut writer = tsv_serializer.serialize_solutions_to_write(
///     &mut buffer,
///     vec![
///         Variable::new_unchecked("foo"),
///         Variable::new_unchecked("bar"),
///     ],
/// )?;
/// writer.write(once((
///     VariableRef::new_unchecked("foo"),
///     LiteralRef::from("test"),
/// )))?;
/// writer.finish()?;
/// assert_eq!(buffer, b"?foo\t?bar\n\"test\"\t\n");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct ToWriteSolutionsWriter<W: Write> {
    formatter: ToWriteSolutionsWriterKind<W>,
}

enum ToWriteSolutionsWriterKind<W: Write> {
    Xml(ToWriteXmlSolutionsWriter<W>),
    Json(ToWriteJsonSolutionsWriter<W>),
    Csv(ToWriteCsvSolutionsWriter<W>),
    Tsv(ToWriteTsvSolutionsWriter<W>),
}

impl<W: Write> ToWriteSolutionsWriter<W> {
    /// Writes a solution.
    ///
    /// Example in JSON (the API is the same for XML, CSV and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsSerializer, QuerySolution};
    /// use oxrdf::{Literal, LiteralRef, Variable, VariableRef};
    /// use std::iter::once;
    ///
    /// let json_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Json);
    /// let mut buffer = Vec::new();
    /// let mut writer = json_serializer.serialize_solutions_to_write(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")])?;
    /// writer.write(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test"))))?;
    /// writer.write(&QuerySolution::from((vec![Variable::new_unchecked("bar")], vec![Some(Literal::from("test").into())])))?;
    /// writer.finish()?;
    /// assert_eq!(buffer, br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}},{"bar":{"type":"literal","value":"test"}}]}}"#);
    /// # std::io::Result::Ok(())
    /// ```
    pub fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (impl Into<VariableRef<'a>>, impl Into<TermRef<'a>>)>,
    ) -> io::Result<()> {
        let solution = solution.into_iter().map(|(v, s)| (v.into(), s.into()));
        match &mut self.formatter {
            ToWriteSolutionsWriterKind::Xml(writer) => writer.write(solution),
            ToWriteSolutionsWriterKind::Json(writer) => writer.write(solution),
            ToWriteSolutionsWriterKind::Csv(writer) => writer.write(solution),
            ToWriteSolutionsWriterKind::Tsv(writer) => writer.write(solution),
        }
    }

    /// Writes the last bytes of the file.
    pub fn finish(self) -> io::Result<W> {
        match self.formatter {
            ToWriteSolutionsWriterKind::Xml(write) => write.finish(),
            ToWriteSolutionsWriterKind::Json(write) => write.finish(),
            ToWriteSolutionsWriterKind::Csv(write) => Ok(write.finish()),
            ToWriteSolutionsWriterKind::Tsv(write) => Ok(write.finish()),
        }
    }
}

/// Allows writing query results into an [`AsyncWrite`] implementation.

/// Could be built using a [`QueryResultsSerializer`].
///
/// <div class="warning">
///
/// Do not forget to run the [`finish`](ToTokioAsyncWriteSolutionsWriter::finish()) method to properly write the last bytes of the file.</div>
///
/// <div class="warning">
///
/// This writer does unbuffered writes. You might want to use [`BufWriter`](tokio::io::BufWriter) to avoid that.</div>
///
/// Example in TSV (the API is the same for JSON, CSV and XML):
/// ```
/// use oxrdf::{LiteralRef, Variable, VariableRef};
/// use sparesults::{QueryResultsFormat, QueryResultsSerializer};
/// use std::iter::once;
///
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> std::io::Result<()> {
/// let tsv_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Tsv);
/// let mut buffer = Vec::new();
/// let mut writer = tsv_serializer
///     .serialize_solutions_to_tokio_async_write(
///         &mut buffer,
///         vec![
///             Variable::new_unchecked("foo"),
///             Variable::new_unchecked("bar"),
///         ],
///     )
///     .await?;
/// writer
///     .write(once((
///         VariableRef::new_unchecked("foo"),
///         LiteralRef::from("test"),
///     )))
///     .await?;
/// writer.finish().await?;
/// assert_eq!(buffer, b"?foo\t?bar\n\"test\"\t\n");
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct ToTokioAsyncWriteSolutionsWriter<W: AsyncWrite + Unpin> {
    formatter: ToTokioAsyncWriteSolutionsWriterKind<W>,
}

#[cfg(feature = "async-tokio")]
enum ToTokioAsyncWriteSolutionsWriterKind<W: AsyncWrite + Unpin> {
    Xml(ToTokioAsyncWriteXmlSolutionsWriter<W>),
    Json(ToTokioAsyncWriteJsonSolutionsWriter<W>),
    Csv(ToTokioAsyncWriteCsvSolutionsWriter<W>),
    Tsv(ToTokioAsyncWriteTsvSolutionsWriter<W>),
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> ToTokioAsyncWriteSolutionsWriter<W> {
    /// Writes a solution.
    ///
    /// Example in JSON (the API is the same for XML, CSV and TSV):
    /// ```
    /// use sparesults::{QueryResultsFormat, QueryResultsSerializer, QuerySolution};
    /// use oxrdf::{Literal, LiteralRef, Variable, VariableRef};
    /// use std::iter::once;
    ///
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> std::io::Result<()> {
    /// let json_serializer = QueryResultsSerializer::from_format(QueryResultsFormat::Json);
    /// let mut buffer = Vec::new();
    /// let mut writer = json_serializer.serialize_solutions_to_tokio_async_write(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]).await?;
    /// writer.write(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test")))).await?;
    /// writer.write(&QuerySolution::from((vec![Variable::new_unchecked("bar")], vec![Some(Literal::from("test").into())]))).await?;
    /// writer.finish().await?;
    /// assert_eq!(buffer, br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}},{"bar":{"type":"literal","value":"test"}}]}}"#);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn write<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (impl Into<VariableRef<'a>>, impl Into<TermRef<'a>>)>,
    ) -> io::Result<()> {
        let solution = solution.into_iter().map(|(v, s)| (v.into(), s.into()));
        match &mut self.formatter {
            ToTokioAsyncWriteSolutionsWriterKind::Xml(writer) => writer.write(solution).await,
            ToTokioAsyncWriteSolutionsWriterKind::Json(writer) => writer.write(solution).await,
            ToTokioAsyncWriteSolutionsWriterKind::Csv(writer) => writer.write(solution).await,
            ToTokioAsyncWriteSolutionsWriterKind::Tsv(writer) => writer.write(solution).await,
        }
    }

    /// Writes the last bytes of the file.
    pub async fn finish(self) -> io::Result<W> {
        match self.formatter {
            ToTokioAsyncWriteSolutionsWriterKind::Xml(write) => write.finish().await,
            ToTokioAsyncWriteSolutionsWriterKind::Json(write) => write.finish().await,
            ToTokioAsyncWriteSolutionsWriterKind::Csv(write) => Ok(write.finish()),
            ToTokioAsyncWriteSolutionsWriterKind::Tsv(write) => Ok(write.finish()),
        }
    }
}
