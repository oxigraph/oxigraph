use rio_api::iri::IriParseError;
use std::error;
use std::fmt;

#[derive(Debug)]
pub struct ModelError {
    inner: ErrorKind,
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            ErrorKind::Iri(e) => e.fmt(f),
        }
    }
}

impl error::Error for ModelError {
    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match &self.inner {
            ErrorKind::Iri(e) => Some(e),
        }
    }
}

#[derive(Debug)]
enum ErrorKind {
    Iri(IriParseError),
}

impl From<IriParseError> for ModelError {
    fn from(error: IriParseError) -> Self {
        Self {
            inner: ErrorKind::Iri(error),
        }
    }
}
