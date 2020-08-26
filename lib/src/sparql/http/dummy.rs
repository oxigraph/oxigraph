//! Simple HTTP client

use crate::error::invalid_input_error;
use http::{Request, Response};
use std::io;
use std::io::BufRead;

pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Self {}
    }

    pub fn request(
        &self,
        _request: &Request<Option<Vec<u8>>>,
    ) -> io::Result<Response<Box<dyn BufRead>>> {
        Err(invalid_input_error(
            "HTTP client is not available. Enable the feature 'simple_http'",
        ))
    }
}
