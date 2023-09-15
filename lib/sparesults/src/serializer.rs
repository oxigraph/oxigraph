use crate::csv::{
    write_boolean_csv_result, write_boolean_tsv_result, CsvSolutionsWriter, TsvSolutionsWriter,
};
use crate::format::QueryResultsFormat;
use crate::json::{write_boolean_json_result, JsonSolutionsWriter};
use crate::xml::{write_boolean_xml_result, XmlSolutionsWriter};
use oxrdf::{TermRef, Variable, VariableRef};
use std::io::{self, Write};

/// A serializer for [SPARQL query](https://www.w3.org/TR/sparql11-query/) results serialization formats.
///
/// It currently supports the following formats:
/// * [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/) ([`QueryResultsFormat::Xml`](QueryResultsFormat::Xml))
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

    /// Write a boolean query result (from an `ASK` query)  into the given [`Write`] implementation.
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

    /// Returns a `SolutionsWriter` allowing writing query solutions into the given [`Write`] implementation.
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
                    SolutionsWriterKind::Xml(XmlSolutionsWriter::start(writer, &variables)?)
                }
                QueryResultsFormat::Json => {
                    SolutionsWriterKind::Json(JsonSolutionsWriter::start(writer, &variables)?)
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
/// <div class="warning">Do not forget to run the [`finish`](SolutionsWriter::finish()) method to properly write the last bytes of the file.</div>
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
/// assert_eq!(buffer, b"?foo\t?bar\n\"test\"\t\n");
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
