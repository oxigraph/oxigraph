#![allow(deprecated)]

//! Utilities to read RDF graphs and datasets.

use crate::io::{DatasetFormat, GraphFormat};
use crate::model::*;
use oxrdfio::{RdfParseError, RdfParser, ReaderQuadParser};
use std::io::Read;

/// Parsers for RDF graph serialization formats.
///
/// It currently supports the following formats:
/// * [N-Triples](https://www.w3.org/TR/n-triples/) ([`GraphFormat::NTriples`])
/// * [Turtle](https://www.w3.org/TR/turtle/) ([`GraphFormat::Turtle`])
/// * [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/) ([`GraphFormat::RdfXml`])
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphParser};
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let parser = GraphParser::from_format(GraphFormat::NTriples);
/// let triples = parser
///     .read_triples(file.as_bytes())
///     .collect::<Result<Vec<_>, _>>()?;
///
/// assert_eq!(triples.len(), 1);
/// assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[deprecated(note = "use RdfParser instead", since = "0.4.0")]
pub struct GraphParser {
    inner: RdfParser,
}

impl GraphParser {
    /// Builds a parser for the given format.
    #[inline]
    pub fn from_format(format: GraphFormat) -> Self {
        Self {
            inner: RdfParser::from_format(format.into())
                .without_named_graphs()
                .rename_blank_nodes(),
        }
    }

    /// Provides an IRI that could be used to resolve the file relative IRIs.
    ///
    /// ```
    /// use oxigraph::io::{GraphFormat, GraphParser};
    ///
    /// let file = "</s> </p> </o> .";
    ///
    /// let parser =
    ///     GraphParser::from_format(GraphFormat::Turtle).with_base_iri("http://example.com")?;
    /// let triples = parser
    ///     .read_triples(file.as_bytes())
    ///     .collect::<Result<Vec<_>, _>>()?;
    ///
    /// assert_eq!(triples.len(), 1);
    /// assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        Ok(Self {
            inner: self.inner.with_base_iri(base_iri)?,
        })
    }

    /// Executes the parsing itself on a [`Read`] implementation and returns an iterator of triples.
    pub fn read_triples<R: Read>(self, reader: R) -> TripleReader<R> {
        TripleReader {
            parser: self.inner.for_reader(reader),
        }
    }
}

/// An iterator yielding read triples.
/// Could be built using a [`GraphParser`].
///
/// ```
/// use oxigraph::io::{GraphFormat, GraphParser};
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> .";
///
/// let parser = GraphParser::from_format(GraphFormat::NTriples);
/// let triples = parser
///     .read_triples(file.as_bytes())
///     .collect::<Result<Vec<_>, _>>()?;
///
/// assert_eq!(triples.len(), 1);
/// assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct TripleReader<R: Read> {
    parser: ReaderQuadParser<R>,
}

impl<R: Read> Iterator for TripleReader<R> {
    type Item = Result<Triple, RdfParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        Some(self.parser.next()?.map(Into::into))
    }
}

/// A parser for RDF dataset serialization formats.
///
/// It currently supports the following formats:
/// * [N-Quads](https://www.w3.org/TR/n-quads/) ([`DatasetFormat::NQuads`])
/// * [TriG](https://www.w3.org/TR/trig/) ([`DatasetFormat::TriG`])
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetParser};
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .";
///
/// let parser = DatasetParser::from_format(DatasetFormat::NQuads);
/// let quads = parser.read_quads(file.as_bytes()).collect::<Result<Vec<_>,_>>()?;
///
/// assert_eq!(quads.len(), 1);
/// assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[deprecated(note = "use RdfParser instead", since = "0.4.0")]
pub struct DatasetParser {
    inner: RdfParser,
}

impl DatasetParser {
    /// Builds a parser for the given format.
    #[inline]
    pub fn from_format(format: DatasetFormat) -> Self {
        Self {
            inner: RdfParser::from_format(format.into()).rename_blank_nodes(),
        }
    }

    /// Provides an IRI that could be used to resolve the file relative IRIs.
    ///
    /// ```
    /// use oxigraph::io::{DatasetFormat, DatasetParser};
    ///
    /// let file = "<g> { </s> </p> </o> }";
    ///
    /// let parser =
    ///     DatasetParser::from_format(DatasetFormat::TriG).with_base_iri("http://example.com")?;
    /// let triples = parser
    ///     .read_quads(file.as_bytes())
    ///     .collect::<Result<Vec<_>, _>>()?;
    ///
    /// assert_eq!(triples.len(), 1);
    /// assert_eq!(triples[0].subject.to_string(), "<http://example.com/s>");
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        Ok(Self {
            inner: self.inner.with_base_iri(base_iri)?,
        })
    }

    /// Executes the parsing itself on a [`Read`] implementation and returns an iterator of quads.
    pub fn read_quads<R: Read>(self, reader: R) -> QuadReader<R> {
        QuadReader {
            parser: self.inner.for_reader(reader),
        }
    }
}

/// An iterator yielding read quads.
/// Could be built using a [`DatasetParser`].
///
/// ```
/// use oxigraph::io::{DatasetFormat, DatasetParser};
///
/// let file = "<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .";
///
/// let parser = DatasetParser::from_format(DatasetFormat::NQuads);
/// let quads = parser.read_quads(file.as_bytes()).collect::<Result<Vec<_>,_>>()?;
///
/// assert_eq!(quads.len(), 1);
/// assert_eq!(quads[0].subject.to_string(), "<http://example.com/s>");
/// # std::io::Result::Ok(())
/// ```
#[must_use]
pub struct QuadReader<R: Read> {
    parser: ReaderQuadParser<R>,
}

impl<R: Read> Iterator for QuadReader<R> {
    type Item = Result<Quad, RdfParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.parser.next()
    }
}
