use oxilangtag::LanguageTagParseError;
use oxiri::IriParseError;
use peg::error::ParseError;
use peg::str::LineCol;
use rio_turtle::TurtleError;
use rio_xml::RdfXmlError;
use std::error;
use std::fmt;
use std::io;
use std::string::FromUtf8Error;

/// The Oxigraph error type.
///
/// The `wrap` method allows to make this type wrap any implementation of `std::error::Error`.
/// This type also avoids heap allocations for the most common cases of Oxigraph errors.
#[derive(Debug)]
pub struct Error {
    inner: ErrorKind,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            ErrorKind::Msg { msg } => write!(f, "{}", msg),
            ErrorKind::Io(e) => e.fmt(f),
            ErrorKind::FromUtf8(e) => e.fmt(f),
            ErrorKind::Iri(e) => e.fmt(f),
            ErrorKind::LanguageTag(e) => e.fmt(f),
            ErrorKind::Other(e) => e.fmt(f),
        }
    }
}

impl error::Error for Error {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.inner {
            ErrorKind::Msg { .. } => None,
            ErrorKind::Io(e) => Some(e),
            ErrorKind::FromUtf8(e) => Some(e),
            ErrorKind::Iri(e) => Some(e),
            ErrorKind::LanguageTag(e) => Some(e),
            ErrorKind::Other(e) => Some(e.as_ref()),
        }
    }
}

impl Error {
    /// Wraps an other error.
    pub fn wrap(error: impl error::Error + Send + Sync + 'static) -> Self {
        Self {
            inner: ErrorKind::Other(Box::new(error)),
        }
    }

    /// Builds an error from a printable error message.
    pub fn msg(msg: impl Into<String>) -> Self {
        Self {
            inner: ErrorKind::Msg { msg: msg.into() },
        }
    }
}

#[derive(Debug)]
enum ErrorKind {
    Msg { msg: String },
    Io(io::Error),
    FromUtf8(FromUtf8Error),
    Iri(IriParseError),
    LanguageTag(LanguageTagParseError),
    Other(Box<dyn error::Error + Send + Sync + 'static>),
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self {
            inner: ErrorKind::Io(error),
        }
    }
}

impl From<FromUtf8Error> for Error {
    fn from(error: FromUtf8Error) -> Self {
        Self {
            inner: ErrorKind::FromUtf8(error),
        }
    }
}

impl From<IriParseError> for Error {
    fn from(error: IriParseError) -> Self {
        Self {
            inner: ErrorKind::Iri(error),
        }
    }
}

impl From<LanguageTagParseError> for Error {
    fn from(error: LanguageTagParseError) -> Self {
        Self {
            inner: ErrorKind::LanguageTag(error),
        }
    }
}

impl From<TurtleError> for Error {
    fn from(error: TurtleError) -> Self {
        Self::wrap(error)
    }
}

impl From<RdfXmlError> for Error {
    fn from(error: RdfXmlError) -> Self {
        Self::wrap(error)
    }
}

impl From<quick_xml::Error> for Error {
    fn from(error: quick_xml::Error) -> Self {
        Self::wrap(error)
    }
}

impl From<ParseError<LineCol>> for Error {
    fn from(error: ParseError<LineCol>) -> Self {
        Self::wrap(error)
    }
}

#[cfg(feature = "rocksdb")]
impl From<rocksdb::Error> for Error {
    fn from(error: rocksdb::Error) -> Self {
        Self::wrap(error)
    }
}
