use crate::io::read::ParseError;
use std::error::Error;
use std::fmt;
use std::io;

/// An error related to storage operations (reads, writes...).
#[derive(Debug)]
#[non_exhaustive]
pub enum StorageError {
    /// Error from the OS I/O layer.
    Io(io::Error),
    /// Error related to data corruption.
    Corruption(CorruptionError),
    #[doc(hidden)]
    Other(Box<dyn Error + Send + Sync + 'static>),
}

impl fmt::Display for StorageError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Corruption(e) => e.fmt(f),
            Self::Other(e) => e.fmt(f),
        }
    }
}

impl Error for StorageError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Corruption(e) => Some(e),
            Self::Other(e) => Some(e.as_ref()),
        }
    }
}

impl From<io::Error> for StorageError {
    #[inline]
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
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
#[derive(Debug)]
pub struct CorruptionError {
    inner: CorruptionErrorKind,
}

#[derive(Debug)]
enum CorruptionErrorKind {
    Msg(String),
    Other(Box<dyn Error + Send + Sync + 'static>),
}

impl CorruptionError {
    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn new(error: impl Into<Box<dyn Error + Send + Sync + 'static>>) -> Self {
        Self {
            inner: CorruptionErrorKind::Other(error.into()),
        }
    }

    /// Builds an error from a printable error message.
    #[inline]
    pub(crate) fn msg(msg: impl Into<String>) -> Self {
        Self {
            inner: CorruptionErrorKind::Msg(msg.into()),
        }
    }
}

impl fmt::Display for CorruptionError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.inner {
            CorruptionErrorKind::Msg(e) => e.fmt(f),
            CorruptionErrorKind::Other(e) => e.fmt(f),
        }
    }
}

impl Error for CorruptionError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.inner {
            CorruptionErrorKind::Msg(_) => None,
            CorruptionErrorKind::Other(e) => Some(e.as_ref()),
        }
    }
}

impl From<CorruptionError> for StorageError {
    #[inline]
    fn from(error: CorruptionError) -> Self {
        Self::Corruption(error)
    }
}

impl From<CorruptionError> for io::Error {
    #[inline]
    fn from(error: CorruptionError) -> Self {
        Self::new(io::ErrorKind::InvalidData, error)
    }
}

/// An error raised while loading a file into a [`Store`](crate::store::Store).
#[derive(Debug)]
pub enum LoaderError {
    /// An error raised while reading the file.
    Parsing(ParseError),
    /// An error raised during the insertion in the store.
    Storage(StorageError),
}

impl fmt::Display for LoaderError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parsing(e) => e.fmt(f),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl Error for LoaderError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Parsing(e) => Some(e),
            Self::Storage(e) => Some(e),
        }
    }
}

impl From<ParseError> for LoaderError {
    #[inline]
    fn from(error: ParseError) -> Self {
        Self::Parsing(error)
    }
}

impl From<StorageError> for LoaderError {
    #[inline]
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<LoaderError> for io::Error {
    #[inline]
    fn from(error: LoaderError) -> Self {
        match error {
            LoaderError::Storage(error) => error.into(),
            LoaderError::Parsing(error) => error.into(),
        }
    }
}

/// An error raised while writing a file from a [`Store`](crate::store::Store).
#[derive(Debug)]
pub enum SerializerError {
    /// An error raised while writing the content.
    Io(io::Error),
    /// An error raised during the lookup in the store.
    Storage(StorageError),
}

impl fmt::Display for SerializerError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(e) => e.fmt(f),
            Self::Storage(e) => e.fmt(f),
        }
    }
}

impl Error for SerializerError {
    #[inline]
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Storage(e) => Some(e),
        }
    }
}

impl From<io::Error> for SerializerError {
    #[inline]
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<StorageError> for SerializerError {
    #[inline]
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<SerializerError> for io::Error {
    #[inline]
    fn from(error: SerializerError) -> Self {
        match error {
            SerializerError::Storage(error) => error.into(),
            SerializerError::Io(error) => error,
        }
    }
}
