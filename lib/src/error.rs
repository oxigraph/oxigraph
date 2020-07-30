use std::error::Error;
use std::{fmt, io};

//TODO: convert to "!" when "never_type" is going to be stabilized
#[allow(clippy::empty_enum)]
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub(crate) enum Infallible {}

impl Error for Infallible {}

impl fmt::Display for Infallible {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {}
    }
}

impl<T> UnwrapInfallible for Result<T, Infallible> {
    type Value = T;

    fn unwrap_infallible(self) -> T {
        match self {
            Ok(value) => value,
            Err(error) => match error {},
        }
    }
}

/// Traits that allows unwrapping only infallible results
pub(crate) trait UnwrapInfallible {
    type Value;

    fn unwrap_infallible(self) -> Self::Value;
}

impl From<std::convert::Infallible> for Infallible {
    fn from(error: std::convert::Infallible) -> Self {
        match error {}
    }
}

impl From<Infallible> for std::io::Error {
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

impl From<Infallible> for std::convert::Infallible {
    fn from(error: Infallible) -> Self {
        match error {}
    }
}

pub(crate) fn invalid_data_error(error: impl Into<Box<dyn Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

pub(crate) fn invalid_input_error(error: impl Into<Box<dyn Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}
