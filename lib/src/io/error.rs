use oxiri::IriParseError;
use rio_turtle::TurtleError;
use rio_xml::RdfXmlError;
use std::error::Error;
use std::{fmt, io};

/// Error returned during RDF format parsing.
#[derive(Debug)]
pub enum ParseError {
    /// I/O error during parsing (file not found...).
    Io(io::Error),
    /// An error in the file syntax.
    Syntax(SyntaxError),
}

impl ParseError {
    #[inline]
    pub(crate) fn invalid_base_iri(iri: &str, error: IriParseError) -> Self {
        Self::Syntax(SyntaxError {
            inner: SyntaxErrorKind::InvalidBaseIri {
                iri: iri.to_owned(),
                error,
            },
        })
    }
}

impl fmt::Display for ParseError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Syntax(e) => e.fmt(f),
        }
    }
}

impl Error for ParseError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Syntax(e) => Some(e),
        }
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<TurtleError> for ParseError {
    #[inline]
    fn from(error: TurtleError) -> Self {
        let error = io::Error::from(error);
        if error.get_ref().map_or(false, |e| e.is::<TurtleError>()) {
            Self::Syntax(SyntaxError {
                inner: SyntaxErrorKind::Turtle(*error.into_inner().unwrap().downcast().unwrap()),
            })
        } else {
            Self::Io(error)
        }
    }
}

#[allow(clippy::fallible_impl_from)]
impl From<RdfXmlError> for ParseError {
    #[inline]
    fn from(error: RdfXmlError) -> Self {
        let error = io::Error::from(error);
        if error.get_ref().map_or(false, |e| e.is::<RdfXmlError>()) {
            Self::Syntax(SyntaxError {
                inner: SyntaxErrorKind::RdfXml(*error.into_inner().unwrap().downcast().unwrap()),
            })
        } else {
            Self::Io(error)
        }
    }
}

impl From<io::Error> for ParseError {
    #[inline]
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<SyntaxError> for ParseError {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        Self::Syntax(error)
    }
}

impl From<ParseError> for io::Error {
    #[inline]
    fn from(error: ParseError) -> Self {
        match error {
            ParseError::Io(error) => error,
            ParseError::Syntax(error) => error.into(),
        }
    }
}

/// An error in the syntax of the parsed file.
#[derive(Debug)]
pub struct SyntaxError {
    inner: SyntaxErrorKind,
}

#[derive(Debug)]
enum SyntaxErrorKind {
    Turtle(TurtleError),
    RdfXml(RdfXmlError),
    InvalidBaseIri { iri: String, error: IriParseError },
}

impl fmt::Display for SyntaxError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            SyntaxErrorKind::Turtle(e) => e.fmt(f),
            SyntaxErrorKind::RdfXml(e) => e.fmt(f),
            SyntaxErrorKind::InvalidBaseIri { iri, error } => {
                write!(f, "Invalid base IRI '{}': {}", iri, error)
            }
        }
    }
}

impl Error for SyntaxError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.inner {
            SyntaxErrorKind::Turtle(e) => Some(e),
            SyntaxErrorKind::RdfXml(e) => Some(e),
            SyntaxErrorKind::InvalidBaseIri { .. } => None,
        }
    }
}

impl From<SyntaxError> for io::Error {
    #[inline]
    fn from(error: SyntaxError) -> Self {
        match error.inner {
            SyntaxErrorKind::Turtle(error) => error.into(),
            SyntaxErrorKind::RdfXml(error) => error.into(),
            SyntaxErrorKind::InvalidBaseIri { iri, error } => Self::new(
                io::ErrorKind::InvalidInput,
                format!("Invalid IRI '{}': {}", iri, error),
            ),
        }
    }
}
