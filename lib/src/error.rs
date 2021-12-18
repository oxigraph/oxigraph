use std::error::Error;
use std::io;

pub fn invalid_data_error(error: impl Into<Box<dyn Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

pub fn invalid_input_error(error: impl Into<Box<dyn Error + Send + Sync>>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}
