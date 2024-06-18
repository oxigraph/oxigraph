use oxhttp::model::{Body, HeaderName, Method, Request};
use std::io::{Error, ErrorKind, Result};
use std::time::Duration;

pub struct Client {
    client: oxhttp::Client,
}

impl Client {
    pub fn new(timeout: Option<Duration>, redirection_limit: usize) -> Self {
        let mut client = oxhttp::Client::new()
            .with_redirection_limit(redirection_limit)
            .with_user_agent(concat!("Oxigraph/", env!("CARGO_PKG_VERSION")))
            .unwrap();
        if let Some(timeout) = timeout {
            client = client.with_global_timeout(timeout);
        }
        Self { client }
    }

    pub fn get(&self, url: &str, accept: &'static str) -> Result<(String, Body)> {
        let request = Request::builder(Method::GET, url.parse().map_err(invalid_input_error)?)
            .with_header(HeaderName::ACCEPT, accept)
            .map_err(invalid_input_error)?
            .build();
        let response = self.client.request(request)?;
        let status = response.status();
        if !status.is_successful() {
            return Err(Error::other(format!(
                "Error {} returned by {} with payload:\n{}",
                status,
                url,
                response.into_body().to_string()?
            )));
        }
        let content_type = response
            .header(&HeaderName::CONTENT_TYPE)
            .ok_or_else(|| invalid_data_error(format!("No Content-Type returned by {url}")))?
            .to_str()
            .map_err(invalid_data_error)?
            .to_owned();
        Ok((content_type, response.into_body()))
    }

    pub fn post(
        &self,
        url: &str,
        payload: Vec<u8>,
        content_type: &'static str,
        accept: &'static str,
    ) -> Result<(String, Body)> {
        let request = Request::builder(Method::POST, url.parse().map_err(invalid_input_error)?)
            .with_header(HeaderName::ACCEPT, accept)
            .map_err(invalid_input_error)?
            .with_header(HeaderName::CONTENT_TYPE, content_type)
            .map_err(invalid_input_error)?
            .with_body(payload);
        let response = self.client.request(request)?;
        let status = response.status();
        if !status.is_successful() {
            return Err(Error::other(format!(
                "Error {} returned by {} with payload:\n{}",
                status,
                url,
                response.into_body().to_string()?
            )));
        }
        let content_type = response
            .header(&HeaderName::CONTENT_TYPE)
            .ok_or_else(|| invalid_data_error(format!("No Content-Type returned by {url}")))?
            .to_str()
            .map_err(invalid_data_error)?
            .to_owned();
        Ok((content_type, response.into_body()))
    }
}

fn invalid_data_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
    Error::new(ErrorKind::InvalidData, error)
}

fn invalid_input_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
    Error::new(ErrorKind::InvalidInput, error)
}
