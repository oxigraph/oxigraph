#[cfg(feature = "async-tokio")]
use crate::csv::{
    tokio_async_write_boolean_csv_result, TokioAsyncWriterCsvSolutionsSerializer,
    TokioAsyncWriterTsvSolutionsSerializer,
};
use crate::csv::{
    write_boolean_csv_result, WriterCsvSolutionsSerializer, WriterTsvSolutionsSerializer,
};
use crate::format::QueryResultsFormat;
#[cfg(feature = "async-tokio")]
use crate::json::{tokio_async_write_boolean_json_result, TokioAsyncWriterJsonSolutionsSerializer};
use crate::json::{write_boolean_json_result, WriterJsonSolutionsSerializer};
#[cfg(feature = "async-tokio")]
use crate::xml::{tokio_async_write_boolean_xml_result, TokioAsyncWriterXmlSolutionsSerializer};
use crate::xml::{write_boolean_xml_result, WriterXmlSolutionsSerializer};
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
/// json_serializer.clone().serialize_boolean_to_writer(&mut buffer, true)?;
/// assert_eq!(buffer, br#"{"head":{},"boolean":true}"#);
///
/// // solutions
/// let mut buffer = Vec::new();
/// let mut serializer = json_serializer.serialize_solutions_to_writer(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")])?;
/// serializer.serialize(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test"))))?;
/// serializer.finish()?;
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
    /// xml_serializer.serialize_boolean_to_writer(&mut buffer, true)?;
    /// assert_eq!(buffer, br#"<?xml version="1.0"?><sparql xmlns="http://www.w3.org/2005/sparql-results#"><head></head><boolean>true</boolean></sparql>"#);
    /// # std::io::Result::Ok(())
    /// ```
    pub fn serialize_boolean_to_writer<W: Write>(self, writer: W, value: bool) -> io::Result<W> {
        match self.format {
            QueryResultsFormat::Xml => write_boolean_xml_result(writer, value),
            QueryResultsFormat::Json => write_boolean_json_result(writer, value),
            QueryResultsFormat::Csv | QueryResultsFormat::Tsv => {
                write_boolean_csv_result(writer, value)
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
        writer: W,
        value: bool,
    ) -> io::Result<W> {
        match self.format {
            QueryResultsFormat::Xml => tokio_async_write_boolean_xml_result(writer, value).await,
            QueryResultsFormat::Json => tokio_async_write_boolean_json_result(writer, value).await,
            QueryResultsFormat::Csv | QueryResultsFormat::Tsv => {
                tokio_async_write_boolean_csv_result(writer, value).await
            }
        }
    }

    #[deprecated(note = "use serialize_boolean_to_writer", since = "0.4.0")]
    pub fn write_boolean_result<W: Write>(&self, writer: W, value: bool) -> io::Result<W> {
        self.clone().serialize_boolean_to_writer(writer, value)
    }

    /// Returns a `SolutionsSerializer` allowing writing query solutions into the given [`Write`] implementation.
    ///
    /// <div class="warning">
    ///
    /// Do not forget to run the [`finish`](WriterSolutionsSerializer::finish()) method to properly write the last bytes of the file.</div>
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
    /// let mut serializer = xml_serializer.serialize_solutions_to_writer(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")])?;
    /// serializer.serialize(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test"))))?;
    /// serializer.finish()?;
    /// assert_eq!(buffer, br#"<?xml version="1.0"?><sparql xmlns="http://www.w3.org/2005/sparql-results#"><head><variable name="foo"/><variable name="bar"/></head><results><result><binding name="foo"><literal>test</literal></binding></result></results></sparql>"#);
    /// # std::io::Result::Ok(())
    /// ```
    pub fn serialize_solutions_to_writer<W: Write>(
        self,
        writer: W,
        variables: Vec<Variable>,
    ) -> io::Result<WriterSolutionsSerializer<W>> {
        Ok(WriterSolutionsSerializer {
            formatter: match self.format {
                QueryResultsFormat::Xml => WriterSolutionsSerializerKind::Xml(
                    WriterXmlSolutionsSerializer::start(writer, &variables)?,
                ),
                QueryResultsFormat::Json => WriterSolutionsSerializerKind::Json(
                    WriterJsonSolutionsSerializer::start(writer, &variables)?,
                ),
                QueryResultsFormat::Csv => WriterSolutionsSerializerKind::Csv(
                    WriterCsvSolutionsSerializer::start(writer, variables)?,
                ),
                QueryResultsFormat::Tsv => WriterSolutionsSerializerKind::Tsv(
                    WriterTsvSolutionsSerializer::start(writer, variables)?,
                ),
            },
        })
    }

    /// Returns a `SolutionsSerializer` allowing writing query solutions into the given [`Write`] implementation.
    ///
    /// <div class="warning">
    ///
    /// Do not forget to run the [`finish`](WriterSolutionsSerializer::finish()) method to properly write the last bytes of the file.</div>
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
    /// let mut serializer = json_serializer.serialize_solutions_to_tokio_async_write(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]).await?;
    /// serializer.serialize(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test")))).await?;
    /// serializer.finish().await?;
    /// assert_eq!(buffer, br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}}]}}"#);
    /// # Ok(())
    /// # }    
    /// ```
    #[cfg(feature = "async-tokio")]
    pub async fn serialize_solutions_to_tokio_async_write<W: AsyncWrite + Unpin>(
        self,
        writer: W,
        variables: Vec<Variable>,
    ) -> io::Result<TokioAsyncWriterSolutionsSerializer<W>> {
        Ok(TokioAsyncWriterSolutionsSerializer {
            formatter: match self.format {
                QueryResultsFormat::Xml => TokioAsyncWriterSolutionsSerializerKind::Xml(
                    TokioAsyncWriterXmlSolutionsSerializer::start(writer, &variables).await?,
                ),
                QueryResultsFormat::Json => TokioAsyncWriterSolutionsSerializerKind::Json(
                    TokioAsyncWriterJsonSolutionsSerializer::start(writer, &variables).await?,
                ),
                QueryResultsFormat::Csv => TokioAsyncWriterSolutionsSerializerKind::Csv(
                    TokioAsyncWriterCsvSolutionsSerializer::start(writer, variables).await?,
                ),
                QueryResultsFormat::Tsv => TokioAsyncWriterSolutionsSerializerKind::Tsv(
                    TokioAsyncWriterTsvSolutionsSerializer::start(writer, variables).await?,
                ),
            },
        })
    }

    #[deprecated(note = "use serialize_solutions_to_writer", since = "0.4.0")]
    pub fn solutions_writer<W: Write>(
        &self,
        writer: W,
        variables: Vec<Variable>,
    ) -> io::Result<WriterSolutionsSerializer<W>> {
        Self {
            format: self.format,
        }
        .serialize_solutions_to_writer(writer, variables)
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
/// Do not forget to run the [`finish`](WriterSolutionsSerializer::finish()) method to properly write the last bytes of the file.</div>
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
/// let mut serializer = tsv_serializer.serialize_solutions_to_writer(
///     &mut buffer,
///     vec![
///         Variable::new_unchecked("foo"),
///         Variable::new_unchecked("bar"),
///     ],
/// )?;
/// serializer.serialize(once((
///     VariableRef::new_unchecked("foo"),
///     LiteralRef::from("test"),
/// )))?;
/// serializer.finish()?;
/// assert_eq!(buffer, b"?foo\t?bar\n\"test\"\t\n");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct WriterSolutionsSerializer<W: Write> {
    formatter: WriterSolutionsSerializerKind<W>,
}

enum WriterSolutionsSerializerKind<W: Write> {
    Xml(WriterXmlSolutionsSerializer<W>),
    Json(WriterJsonSolutionsSerializer<W>),
    Csv(WriterCsvSolutionsSerializer<W>),
    Tsv(WriterTsvSolutionsSerializer<W>),
}

impl<W: Write> WriterSolutionsSerializer<W> {
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
    /// let mut serializer = json_serializer.serialize_solutions_to_writer(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")])?;
    /// serializer.serialize(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test"))))?;
    /// serializer.serialize(&QuerySolution::from((vec![Variable::new_unchecked("bar")], vec![Some(Literal::from("test").into())])))?;
    /// serializer.finish()?;
    /// assert_eq!(buffer, br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}},{"bar":{"type":"literal","value":"test"}}]}}"#);
    /// # std::io::Result::Ok(())
    /// ```
    pub fn serialize<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (impl Into<VariableRef<'a>>, impl Into<TermRef<'a>>)>,
    ) -> io::Result<()> {
        let solution = solution.into_iter().map(|(v, s)| (v.into(), s.into()));
        match &mut self.formatter {
            WriterSolutionsSerializerKind::Xml(writer) => writer.serialize(solution),
            WriterSolutionsSerializerKind::Json(writer) => writer.serialize(solution),
            WriterSolutionsSerializerKind::Csv(writer) => writer.serialize(solution),
            WriterSolutionsSerializerKind::Tsv(writer) => writer.serialize(solution),
        }
    }

    /// Writes the last bytes of the file.
    pub fn finish(self) -> io::Result<W> {
        match self.formatter {
            WriterSolutionsSerializerKind::Xml(serializer) => serializer.finish(),
            WriterSolutionsSerializerKind::Json(serializer) => serializer.finish(),
            WriterSolutionsSerializerKind::Csv(serializer) => Ok(serializer.finish()),
            WriterSolutionsSerializerKind::Tsv(serializer) => Ok(serializer.finish()),
        }
    }
}

/// Allows writing query results into an [`AsyncWrite`] implementation.
///
/// Could be built using a [`QueryResultsSerializer`].
///
/// <div class="warning">
///
/// Do not forget to run the [`finish`](TokioAsyncWriterSolutionsSerializer::finish()) method to properly write the last bytes of the file.</div>
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
/// let mut serializer = tsv_serializer
///     .serialize_solutions_to_tokio_async_write(
///         &mut buffer,
///         vec![
///             Variable::new_unchecked("foo"),
///             Variable::new_unchecked("bar"),
///         ],
///     )
///     .await?;
/// serializer
///     .serialize(once((
///         VariableRef::new_unchecked("foo"),
///         LiteralRef::from("test"),
///     )))
///     .await?;
/// serializer.finish().await?;
/// assert_eq!(buffer, b"?foo\t?bar\n\"test\"\t\n");
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct TokioAsyncWriterSolutionsSerializer<W: AsyncWrite + Unpin> {
    formatter: TokioAsyncWriterSolutionsSerializerKind<W>,
}

#[cfg(feature = "async-tokio")]
enum TokioAsyncWriterSolutionsSerializerKind<W: AsyncWrite + Unpin> {
    Xml(TokioAsyncWriterXmlSolutionsSerializer<W>),
    Json(TokioAsyncWriterJsonSolutionsSerializer<W>),
    Csv(TokioAsyncWriterCsvSolutionsSerializer<W>),
    Tsv(TokioAsyncWriterTsvSolutionsSerializer<W>),
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> TokioAsyncWriterSolutionsSerializer<W> {
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
    /// let mut serializer = json_serializer.serialize_solutions_to_tokio_async_write(&mut buffer, vec![Variable::new_unchecked("foo"), Variable::new_unchecked("bar")]).await?;
    /// serializer.serialize(once((VariableRef::new_unchecked("foo"), LiteralRef::from("test")))).await?;
    /// serializer.serialize(&QuerySolution::from((vec![Variable::new_unchecked("bar")], vec![Some(Literal::from("test").into())]))).await?;
    /// serializer.finish().await?;
    /// assert_eq!(buffer, br#"{"head":{"vars":["foo","bar"]},"results":{"bindings":[{"foo":{"type":"literal","value":"test"}},{"bar":{"type":"literal","value":"test"}}]}}"#);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn serialize<'a>(
        &mut self,
        solution: impl IntoIterator<Item = (impl Into<VariableRef<'a>>, impl Into<TermRef<'a>>)>,
    ) -> io::Result<()> {
        let solution = solution.into_iter().map(|(v, s)| (v.into(), s.into()));
        match &mut self.formatter {
            TokioAsyncWriterSolutionsSerializerKind::Xml(writer) => {
                writer.serialize(solution).await
            }
            TokioAsyncWriterSolutionsSerializerKind::Json(writer) => {
                writer.serialize(solution).await
            }
            TokioAsyncWriterSolutionsSerializerKind::Csv(writer) => {
                writer.serialize(solution).await
            }
            TokioAsyncWriterSolutionsSerializerKind::Tsv(writer) => {
                writer.serialize(solution).await
            }
        }
    }

    /// Writes the last bytes of the file.
    pub async fn finish(self) -> io::Result<W> {
        match self.formatter {
            TokioAsyncWriterSolutionsSerializerKind::Xml(serializer) => serializer.finish().await,
            TokioAsyncWriterSolutionsSerializerKind::Json(serializer) => serializer.finish().await,
            TokioAsyncWriterSolutionsSerializerKind::Csv(serializer) => Ok(serializer.finish()),
            TokioAsyncWriterSolutionsSerializerKind::Tsv(serializer) => Ok(serializer.finish()),
        }
    }
}
