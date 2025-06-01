//! Simple HTTP client

use std::io::{Empty, Error, ErrorKind, Result};
use std::time::Duration;

#[derive(Clone)]
pub struct Client;

impl Client {
    pub fn new(_timeout: Option<Duration>, _redirection_limit: usize) -> Self {
        Self
    }

    #[expect(clippy::unused_self)]
    pub fn get(&self, _url: &str, _accept: &'static str) -> Result<(String, Empty)> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "HTTP client is not available. Enable the feature 'http-client'",
        ))
    }

    #[expect(clippy::unused_self)]
    pub fn post(
        &self,
        _url: &str,
        _payload: Vec<u8>,
        _content_type: &'static str,
        _accept: &'static str,
    ) -> Result<(String, Empty)> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "HTTP client is not available. Enable the feature 'http-client'",
        ))
    }
}
