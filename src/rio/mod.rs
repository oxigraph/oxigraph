use std::error::Error;
use std::fmt;

pub mod ntriples;
pub mod turtle;

pub type RioResult<T> = Result<T, RioError>;

#[derive(Debug)]
pub struct RioError {
    error: Box<Error + Send + Sync>,
}

impl RioError {
    pub fn new<E>(error: E) -> RioError
    where
        E: Into<Box<Error + Send + Sync>>,
    {
        RioError {
            error: error.into(),
        }
    }
}

impl fmt::Display for RioError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl Error for RioError {
    fn description(&self) -> &str {
        self.error.description()
    }

    fn cause(&self) -> Option<&Error> {
        Some(&*self.error)
    }
}
