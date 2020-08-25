//! Simple HTTP client

use crate::error::invalid_input_error;
use http::{Request, Response};
use std::io;

pub struct Client {}

impl Client {
    pub fn new() -> Self {
        Self {}
    }

    pub fn request(&self, _request: &Request<Option<Vec<u8>>>) -> io::Result<Response<Vec<u8>>> {
        Err(invalid_input_error(
            "HTTP client is not available. Enable the feature 'simple_http'",
        ))
    }
}
