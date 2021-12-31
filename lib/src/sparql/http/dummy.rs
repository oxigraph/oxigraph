//! Simple HTTP client

use std::io::{Empty, Error, ErrorKind, Result};
use std::time::Duration;

pub struct Client {}

impl Client {
    pub fn new(_timeout: Option<Duration>) -> Self {
        Self {}
    }

    pub fn get(&self, _url: &str, _accept: &str) -> Result<(String, Empty)> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "HTTP client is not available. Enable the feature 'http_client'",
        ))
    }

    pub fn post(
        &self,
        _url: &str,
        _payload: Vec<u8>,
        _content_type: &str,
        _accept: &str,
    ) -> Result<(String, Empty)> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "HTTP client is not available. Enable the feature 'http_client'",
        ))
    }
}
