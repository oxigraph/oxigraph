//! Simple HTTP client

use crate::error::invalid_input_error;
use std::io;
use std::io::{BufRead, Empty, Read, Result};

pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Self {}
    }

    pub fn get(&self, _url: &str, _accept: &str) -> Result<(String, Empty)> {
        Err(invalid_input_error(
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
        Err(invalid_input_error(
            "HTTP client is not available. Enable the feature 'http_client'",
        ))
    }
}
