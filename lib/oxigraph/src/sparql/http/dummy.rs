//! Simple HTTP client

use std::io::{Empty, Error, ErrorKind, Result};
use std::time::Duration;

pub struct Client;

impl Client {
    pub fn new(_timeout: Option<Duration>, _redirection_limit: usize) -> Self {
        Self
    }

    #[allow(clippy::unused_self)]
    pub fn get(&self, _url: &str, _accept: &'static str) -> Result<(String, Empty)> {
        Err(Error::new(
            ErrorKind::Unsupported,
            "HTTP client is not available. Enable the feature 'http-client'",
        ))
    }

    #[allow(clippy::unused_self, clippy::needless_pass_by_value)]
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
