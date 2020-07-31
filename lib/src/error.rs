use std::convert::Infallible;
use std::error::Error;
use std::io;

/// Traits that allows unwrapping only infallible results
pub(crate) trait UnwrapInfallible {
    type Value;

    fn unwrap_infallible(self) -> Self::Value;
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

pub(crate) fn invalid_data_error(error: impl Into<Box<dyn Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

pub(crate) fn invalid_input_error(error: impl Into<Box<dyn Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}
