use oxilangtag::LanguageTagParseError;
use oxiri::IriParseError;
use std::error::Error;
use std::sync::Arc;
use std::{fmt, io};

/// Error that might be returned during parsing.
///
/// It might wrap an IO error or be a parsing error.
#[derive(Debug)]
pub struct RdfXmlError {
    pub(crate) kind: RdfXmlErrorKind,
}

#[derive(Debug)]
pub(crate) enum RdfXmlErrorKind {
    Xml(quick_xml::Error),
    XmlAttribute(quick_xml::events::attributes::AttrError),
    InvalidIri {
        iri: String,
        error: IriParseError,
    },
    InvalidLanguageTag {
        tag: String,
        error: LanguageTagParseError,
    },
    Other(String),
}

impl RdfXmlError {
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self {
            kind: RdfXmlErrorKind::Other(msg.into()),
        }
    }
}

impl fmt::Display for RdfXmlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            RdfXmlErrorKind::Xml(error) => error.fmt(f),
            RdfXmlErrorKind::XmlAttribute(error) => error.fmt(f),
            RdfXmlErrorKind::InvalidIri { iri, error } => {
                write!(f, "error while parsing IRI '{}': {}", iri, error)
            }
            RdfXmlErrorKind::InvalidLanguageTag { tag, error } => {
                write!(f, "error while parsing language tag '{}': {}", tag, error)
            }
            RdfXmlErrorKind::Other(message) => write!(f, "{}", message),
        }
    }
}

impl Error for RdfXmlError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            RdfXmlErrorKind::Xml(error) => Some(error),
            RdfXmlErrorKind::XmlAttribute(error) => Some(error),
            RdfXmlErrorKind::InvalidIri { error, .. } => Some(error),
            RdfXmlErrorKind::InvalidLanguageTag { error, .. } => Some(error),
            RdfXmlErrorKind::Other(_) => None,
        }
    }
}

impl From<quick_xml::Error> for RdfXmlError {
    fn from(error: quick_xml::Error) -> Self {
        Self {
            kind: RdfXmlErrorKind::Xml(error),
        }
    }
}

impl From<quick_xml::events::attributes::AttrError> for RdfXmlError {
    fn from(error: quick_xml::events::attributes::AttrError) -> Self {
        Self {
            kind: RdfXmlErrorKind::XmlAttribute(error),
        }
    }
}

impl From<io::Error> for RdfXmlError {
    fn from(error: io::Error) -> Self {
        Self {
            kind: RdfXmlErrorKind::Xml(quick_xml::Error::Io(Arc::new(error))),
        }
    }
}

impl From<RdfXmlError> for io::Error {
    fn from(error: RdfXmlError) -> Self {
        match error.kind {
            RdfXmlErrorKind::Xml(error) => match error {
                quick_xml::Error::Io(error) => match Arc::try_unwrap(error) {
                    Ok(error) => error,
                    Err(error) => io::Error::new(error.kind(), error),
                },
                quick_xml::Error::UnexpectedEof(error) => {
                    io::Error::new(io::ErrorKind::UnexpectedEof, error)
                }
                error => io::Error::new(io::ErrorKind::InvalidData, error),
            },
            RdfXmlErrorKind::Other(error) => io::Error::new(io::ErrorKind::InvalidData, error),
            _ => io::Error::new(io::ErrorKind::InvalidData, error),
        }
    }
}
