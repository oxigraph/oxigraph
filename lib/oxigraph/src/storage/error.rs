use crate::io::{ParseError, RdfFormat};
use oxiri::IriParseError;
use std::error::Error;
use std::io;
use thiserror::Error;

/// An error related to storage operations (reads, writes...).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StorageError {
    /// Error from the OS I/O layer.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// Error related to data corruption.
    #[error(transparent)]
    Corruption(#[from] CorruptionError),
    #[doc(hidden)]
    #[error("{0}")]
    Other(#[source] Box<dyn Error + Send + Sync + 'static>),
}

impl From<StorageError> for io::Error {
    #[inline]
    fn from(error: StorageError) -> Self {
        match error {
            StorageError::Io(error) => error,
            StorageError::Corruption(error) => error.into(),
            StorageError::Other(error) => Self::new(io::ErrorKind::Other, error),
        }
    }
}

/// An error return if some content in the database is corrupted.
#[derive(Debug, Error)]
pub enum CorruptionError {
    #[error("{0}")]
    Msg(String),
    #[error("{0}")]
    Other(#[source] Box<dyn Error + Send + Sync + 'static>),
}

impl CorruptionError {
    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn new(error: impl Into<Box<dyn Error + Send + Sync + 'static>>) -> Self {
        Self::Other(error.into())
    }

    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self::Msg(msg.into())
    }
}

impl From<CorruptionError> for io::Error {
    #[inline]
    fn from(error: CorruptionError) -> Self {
        Self::new(io::ErrorKind::InvalidData, error)
    }
}

/// An error raised while loading a file into a [`Store`](crate::store::Store).
#[derive(Debug, Error)]
pub enum LoaderError {
    /// An error raised while reading the file.
    #[error(transparent)]
    Parsing(#[from] ParseError),
    /// An error raised during the insertion in the store.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// The base IRI is invalid.
    #[error("Invalid base IRI '{iri}': {error}")]
    InvalidBaseIri {
        /// The IRI itself.
        iri: String,
        /// The parsing error.
        #[source]
        error: IriParseError,
    },
}

impl From<LoaderError> for io::Error {
    #[inline]
    fn from(error: LoaderError) -> Self {
        match error {
            LoaderError::Storage(error) => error.into(),
            LoaderError::Parsing(error) => error.into(),
            LoaderError::InvalidBaseIri { .. } => {
                Self::new(io::ErrorKind::InvalidInput, error.to_string())
            }
        }
    }
}

/// An error raised while writing a file from a [`Store`](crate::store::Store).
#[derive(Debug, Error)]
pub enum SerializerError {
    /// An error raised while writing the content.
    #[error(transparent)]
    Io(#[from] io::Error),
    /// An error raised during the lookup in the store.
    #[error(transparent)]
    Storage(#[from] StorageError),
    /// A format compatible with [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) is required.
    #[error("A RDF format supporting datasets was expected, {0} found")]
    DatasetFormatExpected(RdfFormat),
}

impl From<SerializerError> for io::Error {
    #[inline]
    fn from(error: SerializerError) -> Self {
        match error {
            SerializerError::Storage(error) => error.into(),
            SerializerError::Io(error) => error,
            SerializerError::DatasetFormatExpected(_) => {
                Self::new(io::ErrorKind::InvalidInput, error.to_string())
            }
        }
    }
}
