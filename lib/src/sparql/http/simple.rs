use crate::error::{invalid_data_error, invalid_input_error};
use oxhttp::model::{Body, HeaderName, Method, Request};
use std::io::Result;
use std::time::Duration;

const USER_AGENT: &str = concat!("Oxigraph/", env!("CARGO_PKG_VERSION"));

pub struct Client {
    client: oxhttp::Client,
}

impl Client {
    pub fn new(timeout: Option<Duration>) -> Self {
        let mut client = oxhttp::Client::new();
        client.set_global_timeout(timeout);
        Self { client }
    }

    pub fn get(&self, url: &str, accept: &str) -> Result<(String, Body)> {
        let mut request = Request::new(Method::GET, url.parse().map_err(invalid_input_error)?);
        request.headers_mut().append(
            HeaderName::ACCEPT,
            accept.parse().map_err(invalid_input_error)?,
        );
        request.headers_mut().append(
            HeaderName::USER_AGENT,
            USER_AGENT.parse().map_err(invalid_input_error)?,
        );
        let response = self.client.request(request)?;
        let content_type = response
            .headers()
            .get(&HeaderName::CONTENT_TYPE)
            .ok_or_else(|| invalid_data_error(format!("No Content-Type returned by {}", url)))?
            .to_str()
            .map_err(invalid_data_error)?
            .to_owned();
        Ok((content_type, response.into_body()))
    }

    pub fn post(
        &self,
        url: &str,
        payload: Vec<u8>,
        content_type: &str,
        accept: &str,
    ) -> Result<(String, Body)> {
        let mut request = Request::new(Method::GET, url.parse().map_err(invalid_input_error)?);
        request.headers_mut().append(
            HeaderName::ACCEPT,
            accept.parse().map_err(invalid_input_error)?,
        );
        request.headers_mut().append(
            HeaderName::USER_AGENT,
            USER_AGENT.parse().map_err(invalid_input_error)?,
        );
        request.headers_mut().append(
            HeaderName::CONTENT_TYPE,
            content_type.parse().map_err(invalid_input_error)?,
        );
        let response = self.client.request(request.with_body(payload))?;
        let content_type = response
            .headers()
            .get(&HeaderName::CONTENT_TYPE)
            .ok_or_else(|| invalid_data_error(format!("No Content-Type returned by {}", url)))?
            .to_str()
            .map_err(invalid_data_error)?
            .to_owned();
        Ok((content_type, response.into_body()))
    }
}
