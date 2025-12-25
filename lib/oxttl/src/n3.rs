//! A [N3](https://w3c.github.io/N3/spec/) streaming parser implemented by [`N3Parser`]
//! and a serializer implemented by [`N3Serializer`].

use crate::lexer::{N3Lexer, N3LexerMode, N3LexerOptions, N3Token, resolve_local_name};
#[cfg(feature = "async-tokio")]
use crate::toolkit::TokioAsyncReaderIterator;
use crate::toolkit::{
    Lexer, Parser, ReaderIterator, RuleRecognizer, RuleRecognizerError, SliceIterator,
    TokenOrLineJump, TurtleSyntaxError,
};
use crate::{MAX_BUFFER_SIZE, MIN_BUFFER_SIZE, TurtleParseError};
use oxiri::{Iri, IriParseError};
#[cfg(feature = "rdf-12")]
use oxrdf::Triple;
use oxrdf::vocab::{rdf, xsd};
use oxrdf::{
    BlankNode, GraphName, Literal, NamedNode, NamedNodeRef, NamedOrBlankNode, Quad, Term, Variable,
};
use std::borrow::Cow;
use std::collections::HashMap;
use std::collections::hash_map::Iter;
use std::collections::BTreeMap;
use std::fmt;
use std::io::{self, Read, Write};
#[cfg(feature = "async-tokio")]
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};

/// A N3 term i.e. a RDF `Term` or a `Variable`.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum N3Term {
    NamedNode(NamedNode),
    BlankNode(BlankNode),
    Literal(Literal),
    #[cfg(feature = "rdf-12")]
    Triple(Box<Triple>),
    Variable(Variable),
}

impl fmt::Display for N3Term {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NamedNode(term) => term.fmt(f),
            Self::BlankNode(term) => term.fmt(f),
            Self::Literal(term) => term.fmt(f),
            #[cfg(feature = "rdf-12")]
            Self::Triple(term) => term.fmt(f),
            Self::Variable(term) => term.fmt(f),
        }
    }
}

impl From<NamedNode> for N3Term {
    #[inline]
    fn from(node: NamedNode) -> Self {
        Self::NamedNode(node)
    }
}

impl From<NamedNodeRef<'_>> for N3Term {
    #[inline]
    fn from(node: NamedNodeRef<'_>) -> Self {
        Self::NamedNode(node.into_owned())
    }
}

impl From<BlankNode> for N3Term {
    #[inline]
    fn from(node: BlankNode) -> Self {
        Self::BlankNode(node)
    }
}

impl From<Literal> for N3Term {
    #[inline]
    fn from(literal: Literal) -> Self {
        Self::Literal(literal)
    }
}

#[cfg(feature = "rdf-12")]
impl From<Triple> for N3Term {
    #[inline]
    fn from(triple: Triple) -> Self {
        Self::Triple(Box::new(triple))
    }
}

#[cfg(feature = "rdf-12")]
impl From<Box<Triple>> for N3Term {
    #[inline]
    fn from(node: Box<Triple>) -> Self {
        Self::Triple(node)
    }
}

impl From<NamedOrBlankNode> for N3Term {
    #[inline]
    fn from(node: NamedOrBlankNode) -> Self {
        match node {
            NamedOrBlankNode::NamedNode(node) => node.into(),
            NamedOrBlankNode::BlankNode(node) => node.into(),
        }
    }
}

impl From<Term> for N3Term {
    #[inline]
    fn from(node: Term) -> Self {
        match node {
            Term::NamedNode(node) => node.into(),
            Term::BlankNode(node) => node.into(),
            Term::Literal(node) => node.into(),
            #[cfg(feature = "rdf-12")]
            Term::Triple(triple) => Self::Triple(triple),
        }
    }
}

impl From<Variable> for N3Term {
    #[inline]
    fn from(variable: Variable) -> Self {
        Self::Variable(variable)
    }
}

/// A N3 quad i.e. a quad composed of [`N3Term`].
///
/// The `graph_name` is used to encode the formula where the triple is in.
/// In this case the formula is encoded by a blank node.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct N3Quad {
    /// The [subject](https://www.w3.org/TR/rdf11-concepts/#dfn-subject) of this triple.
    pub subject: N3Term,

    /// The [predicate](https://www.w3.org/TR/rdf11-concepts/#dfn-predicate) of this triple.
    pub predicate: N3Term,

    /// The [object](https://www.w3.org/TR/rdf11-concepts/#dfn-object) of this triple.
    pub object: N3Term,

    /// The name of the RDF [graph](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph) in which the triple is.
    pub graph_name: GraphName,
}

impl fmt::Display for N3Quad {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.graph_name == GraphName::DefaultGraph {
            write!(f, "{} {} {}", self.subject, self.predicate, self.object)
        } else {
            write!(
                f,
                "{} {} {} {}",
                self.subject, self.predicate, self.object, self.graph_name
            )
        }
    }
}

impl From<Quad> for N3Quad {
    fn from(quad: Quad) -> Self {
        Self {
            subject: quad.subject.into(),
            predicate: quad.predicate.into(),
            object: quad.object.into(),
            graph_name: quad.graph_name,
        }
    }
}

/// A [N3](https://w3c.github.io/N3/spec/) streaming parser.
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNode;
/// use oxrdf::vocab::rdf;
/// use oxttl::n3::{N3Parser, N3Term};
///
/// let file = r#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
///
/// let rdf_type = N3Term::NamedNode(rdf::TYPE.into_owned());
/// let schema_person = N3Term::NamedNode(NamedNode::new("http://schema.org/Person")?);
/// let mut count = 0;
/// for triple in N3Parser::new().for_reader(file.as_bytes()) {
///     let triple = triple?;
///     if triple.predicate == rdf_type && triple.object == schema_person {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
#[must_use]
pub struct N3Parser {
    lenient: bool,
    base: Option<Iri<String>>,
    prefixes: HashMap<String, Iri<String>>,
}

impl N3Parser {
    /// Builds a new [`N3Parser`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Assumes the file is valid to make parsing faster.
    ///
    /// It will skip some validations.
    ///
    /// Note that if the file is actually not valid, the parser might emit broken RDF.
    #[inline]
    pub fn lenient(mut self) -> Self {
        self.lenient = true;
        self
    }

    #[deprecated(note = "Use `lenient()` instead", since = "0.2.0")]
    #[inline]
    pub fn unchecked(self) -> Self {
        self.lenient()
    }

    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base = Some(Iri::parse(base_iri.into())?);
        Ok(self)
    }

    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        self.prefixes
            .insert(prefix_name.into(), Iri::parse(prefix_iri.into())?);
        Ok(self)
    }

    /// Parses a N3 file from a [`Read`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::NamedNode;
    /// use oxttl::n3::{N3Parser, N3Term};
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
    ///
    /// let rdf_type = N3Term::NamedNode(NamedNode::new(
    ///     "http://www.w3.org/1999/02/22-rdf-syntax-ns#type",
    /// )?);
    /// let schema_person = N3Term::NamedNode(NamedNode::new("http://schema.org/Person")?);
    /// let mut count = 0;
    /// for triple in N3Parser::new().for_reader(file.as_bytes()) {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf_type && triple.object == schema_person {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_reader<R: Read>(self, reader: R) -> ReaderN3Parser<R> {
        ReaderN3Parser {
            inner: self.low_level().parser.for_reader(reader),
        }
    }

    /// Parses a N3 file from a [`AsyncRead`] implementation.
    ///
    /// Count the number of people:
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxrdf::NamedNode;
    /// use oxrdf::vocab::rdf;
    /// use oxttl::n3::{N3Parser, N3Term};
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
    ///
    /// let rdf_type = N3Term::NamedNode(rdf::TYPE.into_owned());
    /// let schema_person = N3Term::NamedNode(NamedNode::new("http://schema.org/Person")?);
    /// let mut count = 0;
    /// let mut parser = N3Parser::new().for_tokio_async_reader(file.as_bytes());
    /// while let Some(triple) = parser.next().await {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf_type && triple.object == schema_person {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_reader<R: AsyncRead + Unpin>(
        self,
        reader: R,
    ) -> TokioAsyncReaderN3Parser<R> {
        TokioAsyncReaderN3Parser {
            inner: self.low_level().parser.for_tokio_async_reader(reader),
        }
    }

    /// Parses a N3 file from a byte slice.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::NamedNode;
    /// use oxrdf::vocab::rdf;
    /// use oxttl::n3::{N3Parser, N3Term};
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" .
    /// <bar> a schema:Person ;
    ///     schema:name "Bar" ."#;
    ///
    /// let rdf_type = N3Term::NamedNode(rdf::TYPE.into_owned());
    /// let schema_person = N3Term::NamedNode(NamedNode::new("http://schema.org/Person")?);
    /// let mut count = 0;
    /// for triple in N3Parser::new().for_slice(file) {
    ///     let triple = triple?;
    ///     if triple.predicate == rdf_type && triple.object == schema_person {
    ///         count += 1;
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn for_slice(self, slice: &(impl AsRef<[u8]> + ?Sized)) -> SliceN3Parser<'_> {
        SliceN3Parser {
            inner: N3Recognizer::new_parser(slice.as_ref(), true, false, self.base, self.prefixes)
                .into_iter(),
        }
    }

    /// Allows to parse a N3 file by using a low-level API.
    ///
    /// Count the number of people:
    /// ```
    /// use oxrdf::NamedNode;
    /// use oxrdf::vocab::rdf;
    /// use oxttl::n3::{N3Parser, N3Term};
    ///
    /// let file: [&[u8]; 5] = [
    ///     b"@base <http://example.com/>",
    ///     b". @prefix schema: <http://schema.org/> .",
    ///     b"<foo> a schema:Person",
    ///     b" ; schema:name \"Foo\" . <bar>",
    ///     b" a schema:Person ; schema:name \"Bar\" .",
    /// ];
    ///
    /// let rdf_type = N3Term::NamedNode(rdf::TYPE.into_owned());
    /// let schema_person = N3Term::NamedNode(NamedNode::new("http://schema.org/Person")?);
    /// let mut count = 0;
    /// let mut parser = N3Parser::new().low_level();
    /// let mut file_chunks = file.iter();
    /// while !parser.is_end() {
    ///     // We feed more data to the parser
    ///     if let Some(chunk) = file_chunks.next() {
    ///         parser.extend_from_slice(chunk);
    ///     } else {
    ///         parser.end(); // It's finished
    ///     }
    ///     // We read as many triples from the parser as possible
    ///     while let Some(triple) = parser.parse_next() {
    ///         let triple = triple?;
    ///         if triple.predicate == rdf_type && triple.object == schema_person {
    ///             count += 1;
    ///         }
    ///     }
    /// }
    /// assert_eq!(2, count);
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn low_level(self) -> LowLevelN3Parser {
        LowLevelN3Parser {
            parser: N3Recognizer::new_parser(
                Vec::new(),
                false,
                self.lenient,
                self.base,
                self.prefixes,
            ),
        }
    }
}

/// Parses a N3 file from a [`Read`] implementation.
///
/// Can be built using [`N3Parser::for_reader`].
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNode;
/// use oxrdf::vocab::rdf;
/// use oxttl::n3::{N3Parser, N3Term};
///
/// let file = r#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
///
/// let rdf_type = N3Term::NamedNode(rdf::TYPE.into_owned());
/// let schema_person = N3Term::NamedNode(NamedNode::new("http://schema.org/Person")?);
/// let mut count = 0;
/// for triple in N3Parser::new().for_reader(file.as_bytes()) {
///     let triple = triple?;
///     if triple.predicate == rdf_type && triple.object == schema_person {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct ReaderN3Parser<R: Read> {
    inner: ReaderIterator<R, N3Recognizer>,
}

impl<R: Read> ReaderN3Parser<R> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::N3Parser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = N3Parser::new().for_reader(file.as_bytes());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> N3PrefixesIter<'_> {
        N3PrefixesIter {
            inner: self.inner.parser.context.prefixes.iter(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::N3Parser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = N3Parser::new().for_reader(file.as_bytes());
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        self.inner
            .parser
            .context
            .lexer_options
            .base_iri
            .as_ref()
            .map(Iri::as_str)
    }
}

impl<R: Read> Iterator for ReaderN3Parser<R> {
    type Item = Result<N3Quad, TurtleParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Parses a N3 file from a [`AsyncRead`] implementation.
///
/// Can be built using [`N3Parser::for_tokio_async_reader`].
///
/// Count the number of people:
/// ```
/// # #[tokio::main(flavor = "current_thread")]
/// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use oxrdf::NamedNode;
/// use oxrdf::vocab::rdf;
/// use oxttl::n3::{N3Parser, N3Term};
///
/// let file = r#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
///
/// let rdf_type = N3Term::NamedNode(rdf::TYPE.into_owned());
/// let schema_person = N3Term::NamedNode(NamedNode::new("http://schema.org/Person")?);
/// let mut count = 0;
/// let mut parser = N3Parser::new().for_tokio_async_reader(file.as_bytes());
/// while let Some(triple) = parser.next().await {
///     let triple = triple?;
///     if triple.predicate == rdf_type && triple.object == schema_person {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Ok(())
/// # }
/// ```
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct TokioAsyncReaderN3Parser<R: AsyncRead + Unpin> {
    inner: TokioAsyncReaderIterator<R, N3Recognizer>,
}

#[cfg(feature = "async-tokio")]
impl<R: AsyncRead + Unpin> TokioAsyncReaderN3Parser<R> {
    /// Reads the next triple or returns `None` if the file is finished.
    pub async fn next(&mut self) -> Option<Result<N3Quad, TurtleParseError>> {
        self.inner.next().await
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxttl::N3Parser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = N3Parser::new().for_tokio_async_reader(file.as_bytes());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().await.unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
    /// # Ok(())
    /// # }
    /// ```
    pub fn prefixes(&self) -> N3PrefixesIter<'_> {
        N3PrefixesIter {
            inner: self.inner.parser.context.prefixes.iter(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// # #[tokio::main(flavor = "current_thread")]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// use oxttl::N3Parser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = N3Parser::new().for_tokio_async_reader(file.as_bytes());
    /// assert!(parser.base_iri().is_none()); // No base IRI at the beginning
    ///
    /// parser.next().await.unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI
    /// //
    /// # Ok(())
    /// # }
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        self.inner
            .parser
            .context
            .lexer_options
            .base_iri
            .as_ref()
            .map(Iri::as_str)
    }
}

/// Parses a N3 file from a byte slice.
///
/// Can be built using [`N3Parser::for_slice`].
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNode;
/// use oxrdf::vocab::rdf;
/// use oxttl::n3::{N3Parser, N3Term};
///
/// let file = r#"@base <http://example.com/> .
/// @prefix schema: <http://schema.org/> .
/// <foo> a schema:Person ;
///     schema:name "Foo" .
/// <bar> a schema:Person ;
///     schema:name "Bar" ."#;
///
/// let rdf_type = N3Term::NamedNode(rdf::TYPE.into_owned());
/// let schema_person = N3Term::NamedNode(NamedNode::new("http://schema.org/Person")?);
/// let mut count = 0;
/// for triple in N3Parser::new().for_slice(file) {
///     let triple = triple?;
///     if triple.predicate == rdf_type && triple.object == schema_person {
///         count += 1;
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[must_use]
pub struct SliceN3Parser<'a> {
    inner: SliceIterator<'a, N3Recognizer>,
}

impl SliceN3Parser<'_> {
    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::N3Parser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = N3Parser::new().for_slice(file);
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> N3PrefixesIter<'_> {
        N3PrefixesIter {
            inner: self.inner.parser.context.prefixes.iter(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::N3Parser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = N3Parser::new().for_slice(file);
    /// assert!(parser.base_iri().is_none()); // No base at the beginning because none has been given to the parser.
    ///
    /// parser.next().unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI.
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        self.inner
            .parser
            .context
            .lexer_options
            .base_iri
            .as_ref()
            .map(Iri::as_str)
    }
}

impl Iterator for SliceN3Parser<'_> {
    type Item = Result<N3Quad, TurtleSyntaxError>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next()
    }
}

/// Parses a N3 file by using a low-level API.
///
/// Can be built using [`N3Parser::low_level`].
///
/// Count the number of people:
/// ```
/// use oxrdf::NamedNode;
/// use oxrdf::vocab::rdf;
/// use oxttl::n3::{N3Parser, N3Term};
///
/// let file: [&[u8]; 5] = [
///     b"@base <http://example.com/>",
///     b". @prefix schema: <http://schema.org/> .",
///     b"<foo> a schema:Person",
///     b" ; schema:name \"Foo\" . <bar>",
///     b" a schema:Person ; schema:name \"Bar\" .",
/// ];
///
/// let rdf_type = N3Term::NamedNode(rdf::TYPE.into_owned());
/// let schema_person = N3Term::NamedNode(NamedNode::new("http://schema.org/Person")?);
/// let mut count = 0;
/// let mut parser = N3Parser::new().low_level();
/// let mut file_chunks = file.iter();
/// while !parser.is_end() {
///     // We feed more data to the parser
///     if let Some(chunk) = file_chunks.next() {
///         parser.extend_from_slice(chunk);
///     } else {
///         parser.end(); // It's finished
///     }
///     // We read as many triples from the parser as possible
///     while let Some(triple) = parser.parse_next() {
///         let triple = triple?;
///         if triple.predicate == rdf_type && triple.object == schema_person {
///             count += 1;
///         }
///     }
/// }
/// assert_eq!(2, count);
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
pub struct LowLevelN3Parser {
    parser: Parser<Vec<u8>, N3Recognizer>,
}

impl LowLevelN3Parser {
    /// Adds some extra bytes to the parser. Should be called when [`parse_next`](Self::parse_next) returns [`None`] and there is still unread data.
    pub fn extend_from_slice(&mut self, other: &[u8]) {
        self.parser.extend_from_slice(other)
    }

    /// Tell the parser that the file is finished.
    ///
    /// This triggers the parsing of the final bytes and might lead [`parse_next`](Self::parse_next) to return some extra values.
    pub fn end(&mut self) {
        self.parser.end()
    }

    /// Returns if the parsing is finished i.e. [`end`](Self::end) has been called and [`parse_next`](Self::parse_next) is always going to return `None`.
    pub fn is_end(&self) -> bool {
        self.parser.is_end()
    }

    /// Attempt to parse a new quad from the already provided data.
    ///
    /// Returns [`None`] if the parsing is finished or more data is required.
    /// If it is the case more data should be fed using [`extend_from_slice`](Self::extend_from_slice).
    pub fn parse_next(&mut self) -> Option<Result<N3Quad, TurtleSyntaxError>> {
        self.parser.parse_next()
    }

    /// The list of IRI prefixes considered at the current step of the parsing.
    ///
    /// This method returns (prefix name, prefix value) tuples.
    /// It is empty at the beginning of the parsing and gets updated when prefixes are encountered.
    /// It should be full at the end of the parsing (but if a prefix is overridden, only the latest version will be returned).
    ///
    /// ```
    /// use oxttl::N3Parser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = N3Parser::new().low_level();
    /// parser.extend_from_slice(file.as_bytes());
    /// assert_eq!(parser.prefixes().collect::<Vec<_>>(), []); // No prefix at the beginning
    ///
    /// parser.parse_next().unwrap()?; // We read the first triple
    /// assert_eq!(
    ///     parser.prefixes().collect::<Vec<_>>(),
    ///     [("schema", "http://schema.org/")]
    /// ); // There are now prefixes
    /// //
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn prefixes(&self) -> N3PrefixesIter<'_> {
        N3PrefixesIter {
            inner: self.parser.context.prefixes.iter(),
        }
    }

    /// The base IRI considered at the current step of the parsing.
    ///
    /// ```
    /// use oxttl::N3Parser;
    ///
    /// let file = r#"@base <http://example.com/> .
    /// @prefix schema: <http://schema.org/> .
    /// <foo> a schema:Person ;
    ///     schema:name "Foo" ."#;
    ///
    /// let mut parser = N3Parser::new().low_level();
    /// parser.extend_from_slice(file.as_bytes());
    /// assert!(parser.base_iri().is_none()); // No base IRI at the beginning
    ///
    /// parser.parse_next().unwrap()?; // We read the first triple
    /// assert_eq!(parser.base_iri(), Some("http://example.com/")); // There is now a base IRI
    /// //
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    pub fn base_iri(&self) -> Option<&str> {
        self.parser
            .context
            .lexer_options
            .base_iri
            .as_ref()
            .map(Iri::as_str)
    }
}

#[derive(Clone)]
enum Predicate {
    Regular(N3Term),
    Inverted(N3Term),
}

struct N3Recognizer {
    stack: Vec<N3State>,
    terms: Vec<N3Term>,
    predicates: Vec<Predicate>,
    contexts: Vec<BlankNode>,
}

struct N3RecognizerContext {
    lexer_options: N3LexerOptions,
    prefixes: HashMap<String, Iri<String>>,
}

impl RuleRecognizer for N3Recognizer {
    type TokenRecognizer = N3Lexer;
    type Output = N3Quad;
    type Context = N3RecognizerContext;

    fn error_recovery_state(mut self) -> Self {
        self.stack.clear();
        self.terms.clear();
        self.predicates.clear();
        self.contexts.clear();
        self
    }

    fn recognize_next(
        mut self,
        token: TokenOrLineJump<N3Token<'_>>,
        context: &mut N3RecognizerContext,
        results: &mut Vec<N3Quad>,
        errors: &mut Vec<RuleRecognizerError>,
    ) -> Self {
        let TokenOrLineJump::Token(token) = token else {
            return self;
        };
        while let Some(rule) = self.stack.pop() {
            match rule {
                // [1]  n3Doc            ::=  ( ( n3Statement ".") | sparqlDirective) *
                // [2]  n3Statement      ::=  n3Directive | triples
                // [3]  n3Directive      ::=  prefixID | base
                // [4]  sparqlDirective  ::=  sparqlBase | sparqlPrefix
                // [5]  sparqlBase       ::=  BASE IRIREF
                // [6]  sparqlPrefix     ::=  PREFIX PNAME_NS IRIREF
                // [7]  prefixID         ::=  "@prefix" PNAME_NS IRIREF
                // [8]  base             ::=  "@base" IRIREF
                N3State::N3Doc => {
                    self.stack.push(N3State::N3Doc);
                    match token {
                        N3Token::PlainKeyword(k) if k.eq_ignore_ascii_case("base") => {
                            self.stack.push(N3State::BaseExpectIri);
                            return self;
                        }
                        N3Token::PlainKeyword(k) if k.eq_ignore_ascii_case("prefix") => {
                            self.stack.push(N3State::PrefixExpectPrefix);
                            return self;
                        }
                        N3Token::LangTag {
                            language: "prefix", #[cfg(
                            feature = "rdf-12"
                        )] direction: None
                        } => {
                            self.stack.push(N3State::N3DocExpectDot);
                            self.stack.push(N3State::PrefixExpectPrefix);
                            return self;
                        }
                        N3Token::LangTag {
                            language: "base", #[cfg(
                            feature = "rdf-12"
                        )] direction: None
                        } => {
                            self.stack.push(N3State::N3DocExpectDot);
                            self.stack.push(N3State::BaseExpectIri);
                            return self;
                        }
                        _ => {
                            self.stack.push(N3State::N3DocExpectDot);
                            self.stack.push(N3State::Triples);
                        }
                    }
                }
                N3State::N3DocExpectDot => {
                    if token == N3Token::Punctuation(".") {
                        return self;
                    }
                    errors.push("Expected a dot '.' at the end of N3 statement".into());
                }
                N3State::BaseExpectIri => return if let N3Token::IriRef(iri) = token {
                    context.lexer_options.base_iri = Some(Iri::parse_unchecked(iri));
                    self
                } else {
                    self.error(errors, "The BASE keyword should be followed by an IRI")
                },
                N3State::PrefixExpectPrefix => return match token {
                    N3Token::PrefixedName { prefix, local, .. } if local.is_empty() => {
                        self.stack.push(N3State::PrefixExpectIri { name: prefix.to_owned() });
                        self
                    }
                    _ => {
                        self.error(errors, "The PREFIX keyword should be followed by a prefix like 'ex:'")
                    }
                },
                N3State::PrefixExpectIri { name } => return if let N3Token::IriRef(iri) = token {
                    context.prefixes.insert(name, Iri::parse_unchecked(iri));
                    self
                } else {
                    self.error(errors, "The PREFIX declaration should be followed by a prefix and its value as an IRI")
                },
                // [9]  triples  ::=  subject predicateObjectList?
                N3State::Triples => {
                    self.stack.push(N3State::TriplesMiddle);
                    self.stack.push(N3State::Path);
                }
                N3State::TriplesMiddle => if matches!(token, N3Token::Punctuation("." | "]" | "}" | ")")) {} else {
                    self.stack.push(N3State::TriplesEnd);
                    self.stack.push(N3State::PredicateObjectList);
                },
                N3State::TriplesEnd => {
                    self.terms.pop();
                }
                // [10]  predicateObjectList  ::=  verb objectList ( ";" ( verb objectList) ? ) *
                N3State::PredicateObjectList => {
                    self.stack.push(N3State::PredicateObjectListEnd);
                    self.stack.push(N3State::ObjectsList);
                    self.stack.push(N3State::Verb);
                }
                N3State::PredicateObjectListEnd => {
                    self.predicates.pop();
                    if token == N3Token::Punctuation(";") {
                        self.stack.push(N3State::PredicateObjectListPossibleContinuation);
                        return self;
                    }
                }
                N3State::PredicateObjectListPossibleContinuation => if token == N3Token::Punctuation(";") {
                    self.stack.push(N3State::PredicateObjectListPossibleContinuation);
                    return self;
                } else if matches!(token, N3Token::Punctuation(";" | "." | "}" | "]" | ")")) {} else {
                    self.stack.push(N3State::PredicateObjectListEnd);
                    self.stack.push(N3State::ObjectsList);
                    self.stack.push(N3State::Verb);
                },
                // [11]  objectList  ::=  object ( "," object) *
                N3State::ObjectsList => {
                    self.stack.push(N3State::ObjectsListEnd);
                    self.stack.push(N3State::Path);
                }
                N3State::ObjectsListEnd => {
                    let object = self.terms.pop().unwrap();
                    let subject = self.terms.last().unwrap().clone();
                    results.push(match self.predicates.last().unwrap().clone() {
                        Predicate::Regular(predicate) => self.quad(
                            subject,
                            predicate,
                            object,
                        ),
                        Predicate::Inverted(predicate) => self.quad(
                            object,
                            predicate,
                            subject,
                        )
                    });
                    if token == N3Token::Punctuation(",") {
                        self.stack.push(N3State::ObjectsListEnd);
                        self.stack.push(N3State::Path);
                        return self;
                    }
                }
                // [12]  verb       ::=  predicate | "a" | ( "has" expression) | ( "is" expression "of") | "=" | "<=" | "=>"
                // [14]  predicate  ::=  expression | ( "<-" expression)
                // N3-specific verbs: "=>" (implies), "<=" (implied by), "=" (owl:sameAs), "has" (forward), "is...of" (inverse)
                N3State::Verb => match token {
                    N3Token::PlainKeyword("a") => {
                        self.predicates.push(Predicate::Regular(rdf::TYPE.into()));
                        return self;
                    }
                    N3Token::PlainKeyword("has") => {
                        self.stack.push(N3State::AfterRegularVerb);
                        self.stack.push(N3State::Path);
                        return self;
                    }
                    N3Token::PlainKeyword("is") => {
                        self.stack.push(N3State::AfterVerbIs);
                        self.stack.push(N3State::Path);
                        return self;
                    }
                    N3Token::Punctuation("=") => {
                        self.predicates.push(Predicate::Regular(NamedNode::new_unchecked("http://www.w3.org/2002/07/owl#sameAs").into()));
                        return self;
                    }
                    N3Token::Punctuation("=>") => {
                        self.predicates.push(Predicate::Regular(NamedNode::new_unchecked("http://www.w3.org/2000/10/swap/log#implies").into()));
                        return self;
                    }
                    N3Token::Punctuation("<=") => {
                        self.predicates.push(Predicate::Inverted(NamedNode::new_unchecked("http://www.w3.org/2000/10/swap/log#implies").into()));
                        return self;
                    }
                    N3Token::Punctuation("<-") => {
                        self.stack.push(N3State::AfterInvertedVerb);
                        self.stack.push(N3State::Path);
                        return self;
                    }
                    _ => {
                        self.stack.push(N3State::AfterRegularVerb);
                        self.stack.push(N3State::Path);
                    }
                }
                N3State::AfterRegularVerb => {
                    self.predicates.push(Predicate::Regular(self.terms.pop().unwrap()));
                }
                N3State::AfterInvertedVerb => {
                    self.predicates.push(Predicate::Inverted(self.terms.pop().unwrap()));
                }
                N3State::AfterVerbIs => return match token {
                    N3Token::PlainKeyword("of") => {
                        self.predicates.push(Predicate::Inverted(self.terms.pop().unwrap()));
                        self
                    }
                    _ => {
                        self.error(errors, "Expected keyword 'of' after predicate in 'is...of' construct (e.g., '?x is :parent of ?y' means '?y :parent ?x')")
                    }
                },
                // [13]  subject     ::=  expression
                // [15]  object      ::=  expression
                // [16]  expression  ::=  path
                // [17]  path        ::=  pathItem ( ( "!" path) | ( "^" path) ) ?
                // N3 path expressions: "!" for forward property, "^" for inverse property
                N3State::Path => {
                    self.stack.push(N3State::PathFollowUp);
                    self.stack.push(N3State::PathItem);
                }
                N3State::PathFollowUp => match token {
                    N3Token::Punctuation("!") => {
                        self.stack.push(N3State::PathAfterIndicator { is_inverse: false });
                        self.stack.push(N3State::PathItem);
                        return self;
                    }
                    N3Token::Punctuation("^") => {
                        self.stack.push(N3State::PathAfterIndicator { is_inverse: true });
                        self.stack.push(N3State::PathItem);
                        return self;
                    }
                    _ => ()
                },
                N3State::PathAfterIndicator { is_inverse } => {
                    let predicate = self.terms.pop().unwrap();
                    let previous = self.terms.pop().unwrap();
                    let current = BlankNode::default();
                    results.push(if is_inverse { self.quad(current.clone(), predicate, previous) } else { self.quad(previous, predicate, current.clone()) });
                    self.terms.push(current.into());
                    self.stack.push(N3State::PathFollowUp);
                }
                // [18]  pathItem               ::=  iri | blankNode | quickVar | collection | blankNodePropertyList | iriPropertyList | literal | formula
                // [19]  literal                ::=  rdfLiteral | numericLiteral | BOOLEAN_LITERAL
                // [20]  blankNodePropertyList  ::=  "[" predicateObjectList "]"
                // [21]  iriPropertyList        ::=  IPLSTART iri predicateObjectList "]"
                // [22]  collection             ::=  "(" object* ")"
                // [23]  formula                ::=  "{" formulaContent? "}"
                // [25]  numericLiteral         ::=  DOUBLE | DECIMAL | INTEGER
                // [26]  rdfLiteral             ::=  STRING ( LANGTAG | ( "^^" iri) ) ?
                // [27]  iri                    ::=  IRIREF | prefixedName
                // [28]  prefixedName           ::=  PNAME_LN | PNAME_NS
                // [29]  blankNode              ::=  BLANK_NODE_LABEL | ANON
                // [30]  quickVar               ::=  QUICK_VAR_NAME
                // N3-specific: variables (?var or $var), formulas ({...}), path expressions
                N3State::PathItem => {
                    return match token {
                        N3Token::IriRef(iri) => {
                            self.terms.push(NamedNode::new_unchecked(iri).into());
                            self
                        }
                        N3Token::PrefixedName { prefix, local, might_be_invalid_iri } => match resolve_local_name(prefix, &local, might_be_invalid_iri, &context.prefixes) {
                            Ok(t) => {
                                self.terms.push(t.into());
                                self
                            }
                            Err(e) => self.error(errors, e)
                        }
                        N3Token::BlankNodeLabel(bnode) => {
                            self.terms.push(BlankNode::new_unchecked(bnode).into());
                            self
                        }
                        N3Token::Variable(name) => {
                            self.terms.push(Variable::new_unchecked(name).into());
                            self
                        }
                        N3Token::Punctuation("[") => {
                            self.stack.push(N3State::PropertyListMiddle);
                            self
                        }
                        N3Token::Punctuation("(") => {
                            self.stack.push(N3State::CollectionBeginning);
                            self
                        }
                        N3Token::String(value) | N3Token::LongString(value) => {
                            self.stack.push(N3State::LiteralPossibleSuffix { value });
                            self
                        }
                        N3Token::Integer(v) => {
                            self.terms.push(Literal::new_typed_literal(v, xsd::INTEGER).into());
                            self
                        }
                        N3Token::Decimal(v) => {
                            self.terms.push(Literal::new_typed_literal(v, xsd::DECIMAL).into());
                            self
                        }
                        N3Token::Double(v) => {
                            self.terms.push(Literal::new_typed_literal(v, xsd::DOUBLE).into());
                            self
                        }
                        N3Token::PlainKeyword("true") => {
                            self.terms.push(Literal::new_typed_literal("true", xsd::BOOLEAN).into());
                            self
                        }
                        N3Token::PlainKeyword("false") => {
                            self.terms.push(Literal::new_typed_literal("false", xsd::BOOLEAN).into());
                            self
                        }
                        N3Token::Punctuation("{") => {
                            self.contexts.push(BlankNode::default());
                            self.stack.push(N3State::FormulaContent);
                            self
                        }
                        _ =>
                            self.error(errors, "Expected a valid N3 term (IRI, blank node, literal, variable using ?var or $var syntax, formula using {...}, collection using (...), or property list using [...]) but found an invalid token")
                    }
                }
                N3State::PropertyListMiddle => match token {
                    N3Token::Punctuation("]") => {
                        self.terms.push(BlankNode::default().into());
                        return self;
                    }
                    N3Token::PlainKeyword("id") => {
                        self.stack.push(N3State::IriPropertyList);
                        return self;
                    }
                    _ => {
                        self.terms.push(BlankNode::default().into());
                        self.stack.push(N3State::PropertyListEnd);
                        self.stack.push(N3State::PredicateObjectList);
                    }
                }
                N3State::PropertyListEnd => if token == N3Token::Punctuation("]") {
                    return self;
                } else {
                    errors.push("Expected closing bracket ']' to end blank node property list (opened with '[')".into());
                }
                N3State::IriPropertyList => return match token {
                    N3Token::IriRef(id) => {
                        self.terms.push(NamedNode::new_unchecked(id).into());
                        self.stack.push(N3State::PropertyListEnd);
                        self.stack.push(N3State::PredicateObjectList);
                        self
                    }
                    N3Token::PrefixedName { prefix, local, might_be_invalid_iri } => match resolve_local_name(prefix, &local, might_be_invalid_iri, &context.prefixes) {
                        Ok(t) => {
                            self.terms.push(t.into());
                            self.stack.push(N3State::PropertyListEnd);
                            self.stack.push(N3State::PredicateObjectList);
                            self
                        }
                        Err(e) => {
                            self.error(errors, e)
                        }
                    }
                    _ => {
                        self.error(errors, "Expected an IRI after '[ id' in IRI property list construction (e.g., '[ id <http://example.org/foo> ... ]')")
                    }
                },
                N3State::CollectionBeginning => if let N3Token::Punctuation(")") = token {
                    self.terms.push(rdf::NIL.into());
                    return self;
                } else {
                    let root = BlankNode::default();
                    self.terms.push(root.clone().into());
                    self.terms.push(root.into());
                    self.stack.push(N3State::CollectionPossibleEnd);
                    self.stack.push(N3State::Path);
                },
                N3State::CollectionPossibleEnd => {
                    let value = self.terms.pop().unwrap();
                    let old = self.terms.pop().unwrap();
                    results.push(self.quad(
                        old.clone(),
                        rdf::FIRST,
                        value,
                    ));
                    if let N3Token::Punctuation(")") = token {
                        results.push(self.quad(
                            old,
                            rdf::REST,
                            rdf::NIL,
                        ));
                        return self;
                    }
                    let new = BlankNode::default();
                    results.push(self.quad(
                        old,
                        rdf::REST,
                        new.clone(),
                    ));
                    self.terms.push(new.into());
                    self.stack.push(N3State::CollectionPossibleEnd);
                    self.stack.push(N3State::Path);
                }
                N3State::LiteralPossibleSuffix { value } => {
                    match token {
                        N3Token::LangTag { language, #[cfg(feature = "rdf-12")]direction } => {
                            #[cfg(feature = "rdf-12")]
                            if direction.is_some() {
                                return self.error(errors, "rdf:dirLangString is not supported in N3");
                            }
                            self.terms.push(Literal::new_language_tagged_literal_unchecked(value, language.to_ascii_lowercase()).into());
                            return self;
                        }
                        N3Token::Punctuation("^^") => {
                            self.stack.push(N3State::LiteralExpectDatatype { value });
                            return self;
                        }
                        _ => {
                            self.terms.push(Literal::new_simple_literal(value).into());
                        }
                    }
                }
                N3State::LiteralExpectDatatype { value } => {
                    match token {
                        N3Token::IriRef(datatype) => {
                            self.terms.push(Literal::new_typed_literal(value, NamedNode::new_unchecked(datatype)).into());
                            return self;
                        }
                        N3Token::PrefixedName { prefix, local, might_be_invalid_iri } => match resolve_local_name(prefix, &local, might_be_invalid_iri, &context.prefixes) {
                            Ok(datatype) => {
                                self.terms.push(Literal::new_typed_literal(value, datatype).into());
                                return self;
                            }
                            Err(e) => {
                                return self.error(errors, e);
                            }
                        }
                        _ => {
                            errors.push("Expected a datatype IRI after '^^' in typed literal (e.g., \"value\"^^xsd:integer or \"value\"^^<http://example.org/type>)".into());
                            self.stack.clear();
                        }
                    }
                }
                // [24]  formulaContent  ::=  ( n3Statement ( "." formulaContent? ) ? ) | ( sparqlDirective formulaContent? )
                // N3 formulas: {...} enclose statements that can be used as terms (for quoting/reification)
                N3State::FormulaContent => {
                    match token {
                        N3Token::Punctuation("}") => {
                            self.terms.push(self.contexts.pop().unwrap().into());
                            return self;
                        }
                        N3Token::PlainKeyword(k)if k.eq_ignore_ascii_case("base") => {
                            self.stack.push(N3State::FormulaContent);
                            self.stack.push(N3State::BaseExpectIri);
                            return self;
                        }
                        N3Token::PlainKeyword(k)if k.eq_ignore_ascii_case("prefix") => {
                            self.stack.push(N3State::FormulaContent);
                            self.stack.push(N3State::PrefixExpectPrefix);
                            return self;
                        }
                        N3Token::LangTag {
                            language: "prefix", #[cfg(
                            feature = "rdf-12"
                        )] direction: None
                        } => {
                            self.stack.push(N3State::FormulaContentExpectDot);
                            self.stack.push(N3State::PrefixExpectPrefix);
                            return self;
                        }
                        N3Token::LangTag {
                            language: "base", #[cfg(
                            feature = "rdf-12"
                        )] direction: None
                        } => {
                            self.stack.push(N3State::FormulaContentExpectDot);
                            self.stack.push(N3State::BaseExpectIri);
                            return self;
                        }
                        _ => {
                            self.stack.push(N3State::FormulaContentExpectDot);
                            self.stack.push(N3State::Triples);
                        }
                    }
                }
                N3State::FormulaContentExpectDot => {
                    match token {
                        N3Token::Punctuation("}") => {
                            self.terms.push(self.contexts.pop().unwrap().into());
                            return self;
                        }
                        N3Token::Punctuation(".") => {
                            self.stack.push(N3State::FormulaContent);
                            return self;
                        }
                        _ => {
                            errors.push("Expected a dot '.' at the end of N3 statement inside formula, or closing brace '}' to end formula".into());
                            self.stack.push(N3State::FormulaContent);
                        }
                    }
                }
            }
        }
        // Empty stack
        if token == N3Token::Punctuation(".") {
            self.stack.push(N3State::N3Doc);
            self
        } else {
            self
        }
    }

    fn recognize_end(
        self,
        _state: &mut N3RecognizerContext,
        _results: &mut Vec<Self::Output>,
        errors: &mut Vec<RuleRecognizerError>,
    ) {
        match &*self.stack {
            [] | [N3State::N3Doc] => (),
            _ => {
                // Check for specific unclosed constructs to give better error messages
                if self.stack.iter().any(|s| matches!(s, N3State::FormulaContent | N3State::FormulaContentExpectDot)) {
                    errors.push("Unexpected end of input: unclosed formula (missing closing brace '}')".into());
                } else if self.stack.iter().any(|s| matches!(s, N3State::CollectionBeginning | N3State::CollectionPossibleEnd)) {
                    errors.push("Unexpected end of input: unclosed collection (missing closing parenthesis ')')".into());
                } else if self.stack.iter().any(|s| matches!(s, N3State::PropertyListMiddle | N3State::PropertyListEnd | N3State::IriPropertyList)) {
                    errors.push("Unexpected end of input: unclosed property list (missing closing bracket ']')".into());
                } else if self.stack.iter().any(|s| matches!(s, N3State::PathFollowUp | N3State::PathAfterIndicator { .. })) {
                    errors.push("Unexpected end of input: incomplete path expression (path operators '!' and '^' require a following term)".into());
                } else {
                    errors.push("Unexpected end of input: incomplete N3 statement".into());
                }
            }
        }
    }

    fn lexer_options(context: &N3RecognizerContext) -> &N3LexerOptions {
        &context.lexer_options
    }
}

impl N3Recognizer {
    pub fn new_parser<B>(
        data: B,
        is_ending: bool,
        unchecked: bool,
        base_iri: Option<Iri<String>>,
        prefixes: HashMap<String, Iri<String>>,
    ) -> Parser<B, Self> {
        Parser::new(
            Lexer::new(
                N3Lexer::new(N3LexerMode::N3, unchecked),
                data,
                is_ending,
                MIN_BUFFER_SIZE,
                MAX_BUFFER_SIZE,
                Some(b"#"),
            ),
            Self {
                stack: vec![N3State::N3Doc],
                terms: Vec::new(),
                predicates: Vec::new(),
                contexts: Vec::new(),
            },
            N3RecognizerContext {
                lexer_options: N3LexerOptions { base_iri },
                prefixes,
            },
        )
    }

    #[must_use]
    fn error(
        mut self,
        errors: &mut Vec<RuleRecognizerError>,
        msg: impl Into<RuleRecognizerError>,
    ) -> Self {
        errors.push(msg.into());
        self.stack.clear();
        self
    }

    fn quad(
        &self,
        subject: impl Into<N3Term>,
        predicate: impl Into<N3Term>,
        object: impl Into<N3Term>,
    ) -> N3Quad {
        N3Quad {
            subject: subject.into(),
            predicate: predicate.into(),
            object: object.into(),
            graph_name: self
                .contexts
                .last()
                .map_or(GraphName::DefaultGraph, |g| g.clone().into()),
        }
    }
}

#[derive(Debug)]
enum N3State {
    N3Doc,
    N3DocExpectDot,
    BaseExpectIri,
    PrefixExpectPrefix,
    PrefixExpectIri { name: String },
    Triples,
    TriplesMiddle,
    TriplesEnd,
    PredicateObjectList,
    PredicateObjectListEnd,
    PredicateObjectListPossibleContinuation,
    ObjectsList,
    ObjectsListEnd,
    Verb,
    AfterRegularVerb,
    AfterInvertedVerb,
    AfterVerbIs,
    Path,
    PathFollowUp,
    PathAfterIndicator { is_inverse: bool },
    PathItem,
    PropertyListMiddle,
    PropertyListEnd,
    IriPropertyList,
    CollectionBeginning,
    CollectionPossibleEnd,
    LiteralPossibleSuffix { value: String },
    LiteralExpectDatatype { value: String },
    FormulaContent,
    FormulaContentExpectDot,
}

/// Iterator on the file prefixes.
///
/// See [`LowLevelN3Parser::prefixes`].
pub struct N3PrefixesIter<'a> {
    inner: Iter<'a, String, Iri<String>>,
}

impl<'a> Iterator for N3PrefixesIter<'a> {
    type Item = (&'a str, &'a str);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let (key, value) = self.inner.next()?;
        Some((key.as_str(), value.as_str()))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

/// A [N3](https://w3c.github.io/N3/spec/) serializer.
///
/// ```
/// use oxrdf::vocab::rdf;
/// use oxrdf::{NamedNodeRef, Variable};
/// use oxttl::n3::{N3Quad, N3Serializer, N3Term};
///
/// let mut serializer = N3Serializer::new()
///     .with_prefix("schema", "http://schema.org/")?
///     .for_writer(Vec::new());
///
/// let quad = N3Quad {
///     subject: N3Term::Variable(Variable::new_unchecked("x")),
///     predicate: N3Term::NamedNode(rdf::TYPE.into_owned()),
///     object: N3Term::NamedNode(NamedNodeRef::new("http://schema.org/Person")?.into_owned()),
///     graph_name: oxrdf::GraphName::DefaultGraph,
/// };
/// serializer.serialize_quad(&quad)?;
///
/// let output = serializer.finish()?;
/// # Result::<_, Box<dyn std::error::Error>>::Ok(())
/// ```
#[derive(Default, Clone)]
#[must_use]
pub struct N3Serializer {
    base_iri: Option<Iri<String>>,
    prefixes: BTreeMap<String, String>,
}

impl N3Serializer {
    /// Builds a new [`N3Serializer`].
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a prefix to the serialization.
    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        self.prefixes.insert(
            prefix_name.into(),
            Iri::parse(prefix_iri.into())?.into_inner(),
        );
        Ok(self)
    }

    /// Adds a base IRI to the serialization.
    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base_iri = Some(Iri::parse(base_iri.into())?);
        Ok(self)
    }

    /// Writes a N3 file to a [`Write`] implementation.
    pub fn for_writer<W: Write>(self, writer: W) -> WriterN3Serializer<W> {
        WriterN3Serializer {
            writer,
            low_level_writer: self.low_level(),
        }
    }

    /// Writes a N3 file to a [`AsyncWrite`] implementation.
    #[cfg(feature = "async-tokio")]
    pub fn for_tokio_async_writer<W: AsyncWrite + Unpin>(
        self,
        writer: W,
    ) -> TokioAsyncWriterN3Serializer<W> {
        TokioAsyncWriterN3Serializer {
            writer,
            low_level_writer: self.low_level(),
            buffer: Vec::new(),
        }
    }

    /// Builds a low-level N3 writer.
    pub fn low_level(self) -> LowLevelN3Serializer {
        // We sort prefixes by decreasing length
        let mut prefixes = self.prefixes.into_iter().collect::<Vec<_>>();
        prefixes.sort_unstable_by(|(_, l), (_, r)| r.len().cmp(&l.len()));
        LowLevelN3Serializer {
            prefixes,
            base_iri: self.base_iri,
            prelude_written: false,
            current_graph_name: GraphName::DefaultGraph,
            current_subject_predicate: None,
        }
    }
}

/// Writes a N3 file to a [`Write`] implementation.
///
/// Can be built using [`N3Serializer::for_writer`].
#[must_use]
pub struct WriterN3Serializer<W: Write> {
    writer: W,
    low_level_writer: LowLevelN3Serializer,
}

impl<W: Write> WriterN3Serializer<W> {
    /// Writes an extra quad.
    pub fn serialize_quad(&mut self, q: &N3Quad) -> io::Result<()> {
        self.low_level_writer.serialize_quad(q, &mut self.writer)
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub fn finish(mut self) -> io::Result<W> {
        self.low_level_writer.finish(&mut self.writer)?;
        Ok(self.writer)
    }
}

/// Writes a N3 file to a [`AsyncWrite`] implementation.
///
/// Can be built using [`N3Serializer::for_tokio_async_writer`].
#[cfg(feature = "async-tokio")]
#[must_use]
pub struct TokioAsyncWriterN3Serializer<W: AsyncWrite + Unpin> {
    writer: W,
    low_level_writer: LowLevelN3Serializer,
    buffer: Vec<u8>,
}

#[cfg(feature = "async-tokio")]
impl<W: AsyncWrite + Unpin> TokioAsyncWriterN3Serializer<W> {
    /// Writes an extra quad.
    pub async fn serialize_quad(&mut self, q: &N3Quad) -> io::Result<()> {
        self.low_level_writer.serialize_quad(q, &mut self.buffer)?;
        self.writer.write_all(&self.buffer).await?;
        self.buffer.clear();
        Ok(())
    }

    /// Ends the write process and returns the underlying [`Write`].
    pub async fn finish(mut self) -> io::Result<W> {
        self.low_level_writer.finish(&mut self.buffer)?;
        self.writer.write_all(&self.buffer).await?;
        self.buffer.clear();
        Ok(self.writer)
    }
}

/// Writes a N3 file by using a low-level API.
///
/// Can be built using [`N3Serializer::low_level`].
pub struct LowLevelN3Serializer {
    prefixes: Vec<(String, String)>,
    base_iri: Option<Iri<String>>,
    prelude_written: bool,
    current_graph_name: GraphName,
    current_subject_predicate: Option<(N3Term, N3Term)>,
}

impl LowLevelN3Serializer {
    /// Writes an extra quad.
    pub fn serialize_quad(&mut self, q: &N3Quad, mut writer: impl Write) -> io::Result<()> {
        if !self.prelude_written {
            self.prelude_written = true;
            if let Some(base_iri) = &self.base_iri {
                writeln!(writer, "@base <{base_iri}> .")?;
            }
            for (prefix_name, prefix_iri) in &self.prefixes {
                writeln!(
                    writer,
                    "@prefix {prefix_name}: <{}> .",
                    relative_iri(prefix_iri, &self.base_iri)
                )?;
            }
        }

        // Handle formulas (graph_name in N3 encodes formulas as blank nodes)
        if q.graph_name != self.current_graph_name {
            if self.current_subject_predicate.is_some() {
                writeln!(writer, " .")?;
            }
            if !self.current_graph_name.is_default_graph() {
                writeln!(writer, "}}")?;
            }
            self.current_graph_name = q.graph_name.clone();
            self.current_subject_predicate = None;

            if let GraphName::BlankNode(bn) = &self.current_graph_name {
                writeln!(writer, "{} {{", self.term(&N3Term::BlankNode(bn.clone())))?;
            }
        }

        // Handle triple serialization with subject/predicate grouping
        if q.graph_name == self.current_graph_name {
            if let Some((current_subject, current_predicate)) =
                self.current_subject_predicate.take()
            {
                if q.subject == current_subject {
                    if q.predicate == current_predicate {
                        self.current_subject_predicate = Some((current_subject, current_predicate));
                        write!(writer, " , {}", self.term(&q.object))
                    } else {
                        self.current_subject_predicate =
                            Some((current_subject, q.predicate.clone()));
                        writeln!(writer, " ;")?;
                        if !self.current_graph_name.is_default_graph() {
                            write!(writer, "\t")?;
                        }
                        write!(
                            writer,
                            "\t{} {}",
                            self.predicate(&q.predicate),
                            self.term(&q.object)
                        )
                    }
                } else {
                    self.current_subject_predicate =
                        Some((q.subject.clone(), q.predicate.clone()));
                    writeln!(writer, " .")?;
                    if !self.current_graph_name.is_default_graph() {
                        write!(writer, "\t")?;
                    }
                    write!(
                        writer,
                        "{} {} {}",
                        self.term(&q.subject),
                        self.predicate(&q.predicate),
                        self.term(&q.object)
                    )
                }
            } else {
                self.current_subject_predicate =
                    Some((q.subject.clone(), q.predicate.clone()));
                if !self.current_graph_name.is_default_graph() {
                    write!(writer, "\t")?;
                }
                write!(
                    writer,
                    "{} {} {}",
                    self.term(&q.subject),
                    self.predicate(&q.predicate),
                    self.term(&q.object)
                )
            }
        } else {
            self.current_subject_predicate = Some((q.subject.clone(), q.predicate.clone()));
            write!(
                writer,
                "{} {} {}",
                self.term(&q.subject),
                self.predicate(&q.predicate),
                self.term(&q.object)
            )
        }
    }

    fn predicate<'a>(&'a self, term: &'a N3Term) -> N3Predicate<'a> {
        N3Predicate {
            term,
            prefixes: &self.prefixes,
            base_iri: &self.base_iri,
        }
    }

    fn term<'a>(&'a self, term: &'a N3Term) -> N3TermFormatter<'a> {
        N3TermFormatter {
            term,
            prefixes: &self.prefixes,
            base_iri: &self.base_iri,
        }
    }

    /// Finishes to write the file.
    pub fn finish(&mut self, mut writer: impl Write) -> io::Result<()> {
        if self.current_subject_predicate.is_some() {
            writeln!(writer, " .")?;
        }
        if !self.current_graph_name.is_default_graph() {
            writeln!(writer, "}}")?;
        }
        Ok(())
    }
}

struct N3Predicate<'a> {
    term: &'a N3Term,
    prefixes: &'a Vec<(String, String)>,
    base_iri: &'a Option<Iri<String>>,
}

impl fmt::Display for N3Predicate<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let N3Term::NamedNode(n) = self.term {
            if n.as_ref() == rdf::TYPE {
                return f.write_str("a");
            }
        }
        N3TermFormatter {
            term: self.term,
            prefixes: self.prefixes,
            base_iri: self.base_iri,
        }
        .fmt(f)
    }
}

struct N3TermFormatter<'a> {
    term: &'a N3Term,
    prefixes: &'a Vec<(String, String)>,
    base_iri: &'a Option<Iri<String>>,
}

impl fmt::Display for N3TermFormatter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.term {
            N3Term::NamedNode(v) => {
                for (prefix_name, prefix_iri) in self.prefixes {
                    if let Some(local_name) = v.as_str().strip_prefix(prefix_iri) {
                        if local_name.is_empty() {
                            return write!(f, "{prefix_name}:");
                        } else if let Some(escaped_local_name) = escape_local_name(local_name) {
                            return write!(f, "{prefix_name}:{escaped_local_name}");
                        }
                    }
                }
                write!(f, "<{}>", relative_iri(v.as_str(), self.base_iri))
            }
            N3Term::BlankNode(v) => write!(f, "{v}"),
            N3Term::Literal(v) => {
                let value = v.value();
                let is_plain = {
                    #[cfg(feature = "rdf-12")]
                    {
                        matches!(
                            v.datatype(),
                            xsd::STRING | rdf::LANG_STRING | rdf::DIR_LANG_STRING
                        )
                    }
                    #[cfg(not(feature = "rdf-12"))]
                    {
                        matches!(v.datatype(), xsd::STRING | rdf::LANG_STRING)
                    }
                };
                if is_plain {
                    write!(f, "{v}")
                } else {
                    let inline = match v.datatype() {
                        xsd::BOOLEAN => is_n3_boolean(value),
                        xsd::INTEGER => is_n3_integer(value),
                        xsd::DECIMAL => is_n3_decimal(value),
                        xsd::DOUBLE => is_n3_double(value),
                        _ => false,
                    };
                    if inline {
                        f.write_str(value)
                    } else {
                        write!(
                            f,
                            "{}^^{}",
                            Literal::new_simple_literal(v.value()),
                            N3TermFormatter {
                                term: &N3Term::NamedNode(v.datatype().into_owned()),
                                prefixes: self.prefixes,
                                base_iri: self.base_iri,
                            }
                        )
                    }
                }
            }
            #[cfg(feature = "rdf-12")]
            N3Term::Triple(t) => {
                write!(
                    f,
                    "<<( {} {} {} )>>",
                    N3TermFormatter {
                        term: &N3Term::from(t.subject.clone()),
                        prefixes: self.prefixes,
                        base_iri: self.base_iri,
                    },
                    N3TermFormatter {
                        term: &N3Term::NamedNode(t.predicate.clone()),
                        prefixes: self.prefixes,
                        base_iri: self.base_iri,
                    },
                    N3TermFormatter {
                        term: &N3Term::from(t.object.clone()),
                        prefixes: self.prefixes,
                        base_iri: self.base_iri,
                    }
                )
            }
            N3Term::Variable(v) => write!(f, "{v}"),
        }
    }
}

fn relative_iri<'a>(iri: &'a str, base_iri: &Option<Iri<String>>) -> Cow<'a, str> {
    if let Some(base_iri) = base_iri {
        if let Ok(relative) = base_iri.relativize(&Iri::parse_unchecked(iri)) {
            return relative.into_inner().into();
        }
    }
    iri.into()
}

fn is_n3_boolean(value: &str) -> bool {
    matches!(value, "true" | "false")
}

fn is_n3_integer(value: &str) -> bool {
    // [19]  INTEGER  ::=  [+-]? [0-9]+
    let mut value = value.as_bytes();
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    !value.is_empty() && value.iter().all(u8::is_ascii_digit)
}

fn is_n3_decimal(value: &str) -> bool {
    // [20]  DECIMAL  ::=  [+-]? [0-9]* '.' [0-9]+
    let mut value = value.as_bytes();
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    while value.first().is_some_and(u8::is_ascii_digit) {
        value = &value[1..];
    }
    let Some(value) = value.strip_prefix(b".") else {
        return false;
    };
    !value.is_empty() && value.iter().all(u8::is_ascii_digit)
}

fn is_n3_double(value: &str) -> bool {
    // [21]    DOUBLE    ::=  [+-]? ([0-9]+ '.' [0-9]* EXPONENT | '.' [0-9]+ EXPONENT | [0-9]+ EXPONENT)
    // [154s]  EXPONENT  ::=  [eE] [+-]? [0-9]+
    let mut value = value.as_bytes();
    if let Some(v) = value.strip_prefix(b"+") {
        value = v;
    } else if let Some(v) = value.strip_prefix(b"-") {
        value = v;
    }
    let mut with_before = false;
    while value.first().is_some_and(u8::is_ascii_digit) {
        value = &value[1..];
        with_before = true;
    }
    let mut with_after = false;
    if let Some(v) = value.strip_prefix(b".") {
        value = v;
        while value.first().is_some_and(u8::is_ascii_digit) {
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

fn escape_local_name(value: &str) -> Option<String> {
    // TODO: PLX
    // [168s] 	PN_LOCAL 	::= 	(PN_CHARS_U | ':' | [0-9] | PLX) ((PN_CHARS | '.' | ':' | PLX)* (PN_CHARS | ':' | PLX))?
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();
    let first = chars.next()?;
    if N3Lexer::is_possible_pn_chars_u(first) || first == ':' || first.is_ascii_digit() {
        output.push(first);
    } else if can_be_escaped_in_local_name(first) {
        output.push('\\');
        output.push(first);
    } else {
        return None;
    }

    while let Some(c) = chars.next() {
        if N3Lexer::is_possible_pn_chars(c) || c == ':' || (c == '.' && !chars.as_str().is_empty())
        {
            output.push(c);
        } else if can_be_escaped_in_local_name(c) {
            output.push('\\');
            output.push(c);
        } else {
            return None;
        }
    }

    Some(output)
}

fn can_be_escaped_in_local_name(c: char) -> bool {
    matches!(
        c,
        '_' | '~'
            | '.'
            | '-'
            | '!'
            | '$'
            | '&'
            | '\''
            | '('
            | ')'
            | '*'
            | '+'
            | ','
            | ';'
            | '='
            | '/'
            | '?'
            | '#'
            | '@'
            | '%'
    )
}

#[cfg(test)]
#[expect(clippy::panic_in_result_fn)]
mod tests {
    use super::*;
    use oxrdf::{Literal, NamedNode};

    #[test]
    fn test_basic_triple_parsing() {
        let data = r#"<http://example.com/s> <http://example.com/p> <http://example.com/o> ."#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0].subject,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/s"))
        );
        assert_eq!(
            quads[0].predicate,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/p"))
        );
        assert_eq!(
            quads[0].object,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/o"))
        );
    }

    #[test]
    fn test_prefix_handling() {
        let data = r#"
            @prefix ex: <http://example.com/> .
            ex:subject ex:predicate ex:object .
        "#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0].subject,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/subject"))
        );
        assert_eq!(
            quads[0].predicate,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/predicate"))
        );
        assert_eq!(
            quads[0].object,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/object"))
        );
    }

    #[test]
    fn test_base_iri_handling() {
        let data = r#"
            @base <http://example.com/> .
            <subject> <predicate> <object> .
        "#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0].subject,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/subject"))
        );
    }

    #[test]
    fn test_base_iri_with_parser_option() {
        let data = r#"<subject> <predicate> <object> ."#;
        let quads: Vec<_> = N3Parser::new()
            .with_base_iri("http://example.org/")
            .unwrap()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0].subject,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.org/subject"))
        );
    }

    #[test]
    fn test_variable_serialization() {
        let data = r#"?x <http://example.com/p> ?y ."#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0].subject,
            N3Term::Variable(Variable::new_unchecked("x"))
        );
        assert_eq!(
            quads[0].object,
            N3Term::Variable(Variable::new_unchecked("y"))
        );

        // Test serialization via Display
        assert_eq!(quads[0].subject.to_string(), "?x");
        assert_eq!(quads[0].object.to_string(), "?y");
    }

    #[test]
    fn test_n3_term_display() {
        // Test NamedNode
        let term = N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/test"));
        assert_eq!(term.to_string(), "<http://example.com/test>");

        // Test BlankNode
        let term = N3Term::BlankNode(BlankNode::new_unchecked("b1"));
        assert!(term.to_string().starts_with("_:"));

        // Test Literal
        let term = N3Term::Literal(Literal::new_simple_literal("hello"));
        assert_eq!(term.to_string(), "\"hello\"");

        // Test Variable
        let term = N3Term::Variable(Variable::new_unchecked("var"));
        assert_eq!(term.to_string(), "?var");
    }

    #[test]
    fn test_n3_quad_display() {
        let quad = N3Quad {
            subject: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/s")),
            predicate: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/p")),
            object: N3Term::Literal(Literal::new_simple_literal("test")),
            graph_name: GraphName::DefaultGraph,
        };

        let output = quad.to_string();
        assert!(output.contains("<http://example.com/s>"));
        assert!(output.contains("<http://example.com/p>"));
        assert!(output.contains("\"test\""));
    }

    #[test]
    fn test_round_trip_simple() {
        let original = r#"<http://example.com/s> <http://example.com/p> <http://example.com/o> ."#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(original)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);

        // Verify we can serialize back using Display
        let serialized = format!(
            "{} {} {} .",
            quads[0].subject,
            quads[0].predicate,
            quads[0].object
        );

        // Parse again
        let quads2: Vec<_> = N3Parser::new()
            .for_slice(&serialized)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads, quads2);
    }

    #[test]
    fn test_round_trip_with_variables() {
        let original = r#"?subject <http://example.com/p> ?object ."#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(original)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);

        // Serialize using Display
        let serialized = format!(
            "{} {} {} .",
            quads[0].subject,
            quads[0].predicate,
            quads[0].object
        );

        // Parse again
        let quads2: Vec<_> = N3Parser::new()
            .for_slice(&serialized)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads, quads2);
    }

    #[test]
    fn test_round_trip_with_literals() {
        let original = r#"<http://example.com/s> <http://example.com/p> "hello world" ."#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(original)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0].object,
            N3Term::Literal(Literal::new_simple_literal("hello world"))
        );

        // Serialize and re-parse
        let serialized = format!(
            "{} {} {} .",
            quads[0].subject,
            quads[0].predicate,
            quads[0].object
        );

        let quads2: Vec<_> = N3Parser::new()
            .for_slice(&serialized)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads, quads2);
    }

    #[test]
    fn test_round_trip_with_language_tag() {
        let original = r#"<http://example.com/s> <http://example.com/p> "hello"@en ."#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(original)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0].object,
            N3Term::Literal(Literal::new_language_tagged_literal_unchecked("hello", "en"))
        );

        let serialized = format!(
            "{} {} {} .",
            quads[0].subject,
            quads[0].predicate,
            quads[0].object
        );

        let quads2: Vec<_> = N3Parser::new()
            .for_slice(&serialized)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads, quads2);
    }

    #[test]
    fn test_multiple_prefixes() {
        let data = r#"
            @prefix ex: <http://example.com/> .
            @prefix foaf: <http://xmlns.com/foaf/0.1/> .
            ex:alice foaf:knows ex:bob .
        "#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0].subject,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/alice"))
        );
        assert_eq!(
            quads[0].predicate,
            N3Term::NamedNode(NamedNode::new_unchecked("http://xmlns.com/foaf/0.1/knows"))
        );
        assert_eq!(
            quads[0].object,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/bob"))
        );
    }

    #[test]
    fn test_parser_with_prefix() {
        let data = r#"ex:subject ex:predicate ex:object ."#;
        let quads: Vec<_> = N3Parser::new()
            .with_prefix("ex", "http://example.com/")
            .unwrap()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);
        assert_eq!(
            quads[0].subject,
            N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/subject"))
        );
    }

    #[test]
    fn test_blank_nodes() {
        let data = r#"_:b1 <http://example.com/p> _:b2 ."#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);

        match &quads[0].subject {
            N3Term::BlankNode(_) => {}
            _ => panic!("Expected blank node subject"),
        }

        match &quads[0].object {
            N3Term::BlankNode(_) => {}
            _ => panic!("Expected blank node object"),
        }
    }

    #[test]
    fn test_formulas() {
        let data = r#"
            { <http://example.com/s> <http://example.com/p> <http://example.com/o> }
            <http://example.com/says> <http://example.com/something> .
        "#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        // Should have at least 2 quads: one inside the formula and one outside
        assert!(quads.len() >= 2);
    }

    #[test]
    fn test_lenient_parsing() {
        // Test that lenient mode can handle some edge cases
        let data = r#"<http://example.com/s> <http://example.com/p> "test" ."#;
        let quads: Vec<_> = N3Parser::new()
            .lenient()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 1);
    }

    #[test]
    fn test_numeric_literals() {
        let data = r#"
            <http://example.com/s> <http://example.com/p1> 42 .
            <http://example.com/s> <http://example.com/p2> 3.14 .
            <http://example.com/s> <http://example.com/p3> 1.0e10 .
        "#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 3);

        // Verify the literals have appropriate datatypes
        match &quads[0].object {
            N3Term::Literal(lit) => {
                assert_eq!(lit.datatype(), xsd::INTEGER);
            }
            _ => panic!("Expected literal"),
        }
    }

    #[test]
    fn test_boolean_literals() {
        let data = r#"
            <http://example.com/s> <http://example.com/p1> true .
            <http://example.com/s> <http://example.com/p2> false .
        "#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        assert_eq!(quads.len(), 2);

        match &quads[0].object {
            N3Term::Literal(lit) => {
                assert_eq!(lit.datatype(), xsd::BOOLEAN);
                assert_eq!(lit.value(), "true");
            }
            _ => panic!("Expected literal"),
        }
    }

    #[test]
    fn test_collections() {
        let data = r#"
            <http://example.com/s> <http://example.com/p> (1 2 3) .
        "#;
        let quads: Vec<_> = N3Parser::new()
            .for_slice(data)
            .collect::<Result<_, _>>()
            .unwrap();

        // Collections expand to multiple triples
        assert!(quads.len() > 1);
    }

    #[test]
    fn test_prefixes_iterator() {
        let data = r#"
            @prefix ex: <http://example.com/> .
            @prefix foaf: <http://xmlns.com/foaf/0.1/> .
            ex:subject ex:predicate ex:object .
        "#;

        let mut parser = N3Parser::new().for_reader(data.as_bytes());

        // Initially no prefixes
        assert_eq!(parser.prefixes().count(), 0);

        // Parse first triple
        parser.next().unwrap().unwrap();

        // Now we should have prefixes
        let prefixes: Vec<_> = parser.prefixes().collect();
        assert_eq!(prefixes.len(), 2);

        // Check both prefixes are present
        assert!(prefixes.iter().any(|(name, _)| *name == "ex"));
        assert!(prefixes.iter().any(|(name, _)| *name == "foaf"));
    }

    #[test]
    fn test_base_iri_getter() {
        let data = r#"
            @base <http://example.com/> .
            <subject> <predicate> <object> .
        "#;

        let mut parser = N3Parser::new().for_reader(data.as_bytes());

        // Initially no base IRI
        assert!(parser.base_iri().is_none());

        // Parse first triple
        parser.next().unwrap().unwrap();

        // Now we should have a base IRI
        assert_eq!(parser.base_iri(), Some("http://example.com/"));
    }

    #[test]
    fn test_n3_serializer_simple() {
        let mut serializer = N3Serializer::new()
            .with_prefix("ex", "http://example.com/")
            .unwrap()
            .for_writer(Vec::new());

        serializer
            .serialize_quad(&N3Quad {
                subject: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/alice")),
                predicate: N3Term::NamedNode(rdf::TYPE.into_owned()),
                object: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/Person")),
                graph_name: GraphName::DefaultGraph,
            })
            .unwrap();

        let output = String::from_utf8(serializer.finish().unwrap()).unwrap();
        assert!(output.contains("@prefix ex: <http://example.com/> ."));
        assert!(output.contains("ex:alice a ex:Person"));
    }

    #[test]
    fn test_n3_serializer_variables() {
        let mut serializer = N3Serializer::new().for_writer(Vec::new());

        serializer
            .serialize_quad(&N3Quad {
                subject: N3Term::Variable(Variable::new_unchecked("x")),
                predicate: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/knows")),
                object: N3Term::Variable(Variable::new_unchecked("y")),
                graph_name: GraphName::DefaultGraph,
            })
            .unwrap();

        let output = String::from_utf8(serializer.finish().unwrap()).unwrap();
        assert!(output.contains("?x"));
        assert!(output.contains("?y"));
    }

    #[test]
    fn test_n3_serializer_grouped_predicates() {
        let mut serializer = N3Serializer::new()
            .with_prefix("ex", "http://example.com/")
            .unwrap()
            .for_writer(Vec::new());

        let subject = N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/alice"));

        // First triple
        serializer
            .serialize_quad(&N3Quad {
                subject: subject.clone(),
                predicate: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/name")),
                object: N3Term::Literal(Literal::new_simple_literal("Alice")),
                graph_name: GraphName::DefaultGraph,
            })
            .unwrap();

        // Second triple (same subject, different predicate)
        serializer
            .serialize_quad(&N3Quad {
                subject: subject.clone(),
                predicate: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/age")),
                object: N3Term::Literal(Literal::new_typed_literal("30", xsd::INTEGER)),
                graph_name: GraphName::DefaultGraph,
            })
            .unwrap();

        let output = String::from_utf8(serializer.finish().unwrap()).unwrap();
        assert!(output.contains(";")); // Should use semicolon for same subject, different predicate
    }

    #[test]
    fn test_n3_serializer_grouped_objects() {
        let mut serializer = N3Serializer::new()
            .with_prefix("ex", "http://example.com/")
            .unwrap()
            .for_writer(Vec::new());

        let subject = N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/alice"));
        let predicate = N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/knows"));

        // First triple
        serializer
            .serialize_quad(&N3Quad {
                subject: subject.clone(),
                predicate: predicate.clone(),
                object: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/bob")),
                graph_name: GraphName::DefaultGraph,
            })
            .unwrap();

        // Second triple (same subject and predicate)
        serializer
            .serialize_quad(&N3Quad {
                subject: subject.clone(),
                predicate: predicate.clone(),
                object: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/charlie")),
                graph_name: GraphName::DefaultGraph,
            })
            .unwrap();

        let output = String::from_utf8(serializer.finish().unwrap()).unwrap();
        assert!(output.contains(",")); // Should use comma for same subject and predicate
    }

    #[test]
    fn test_n3_serializer_with_base() {
        let mut serializer = N3Serializer::new()
            .with_base_iri("http://example.com")
            .unwrap()
            .for_writer(Vec::new());

        serializer
            .serialize_quad(&N3Quad {
                subject: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/alice")),
                predicate: N3Term::NamedNode(rdf::TYPE.into_owned()),
                object: N3Term::NamedNode(NamedNode::new_unchecked("http://example.com/Person")),
                graph_name: GraphName::DefaultGraph,
            })
            .unwrap();

        let output = String::from_utf8(serializer.finish().unwrap()).unwrap();
        assert!(output.contains("@base <http://example.com> ."));
        assert!(output.contains("</alice>")); // Relative IRI
    }
}
