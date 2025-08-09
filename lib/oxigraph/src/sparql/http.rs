use oxhttp::model::header::{ACCEPT, CONTENT_TYPE};
use oxhttp::model::{Body, Method, Request};
use oxiri::Iri;
use oxrdf::NamedNode;
use sparesults::{QueryResultsFormat, QueryResultsParser, ReaderQueryResultsParserOutput};
use spareval::{DefaultServiceHandler, QueryEvaluationError, QuerySolutionIter};
use spargebra::algebra::GraphPattern;
use std::io::{Error, ErrorKind, Read, Result};
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct Client {
    client: Arc<oxhttp::Client>,
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
        Self {
            client: Arc::new(client),
        }
    }

    pub fn get(&self, url: &str, accept: &'static str) -> Result<(String, impl Read)> {
        let request = Request::builder()
            .uri(url)
            .header(ACCEPT, accept)
            .body(())
            .map_err(invalid_input_error)?;
        let response = self.client.request(request)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::other(format!(
                "Error {} returned by {} with payload:\n{}",
                status,
                url,
                response.into_body().to_string()?
            )));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
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
        let request = Request::builder()
            .method(Method::POST)
            .uri(url)
            .header(ACCEPT, accept)
            .header(CONTENT_TYPE, content_type)
            .body(payload)
            .map_err(invalid_input_error)?;
        let response = self.client.request(request)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::other(format!(
                "Error {} returned by {} with payload:\n{}",
                status,
                url,
                response.into_body().to_string()?
            )));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .ok_or_else(|| invalid_data_error(format!("No Content-Type returned by {url}")))?
            .to_str()
            .map_err(invalid_data_error)?
            .to_owned();
        Ok((content_type, response.into_body()))
    }
}

pub struct HttpServiceHandler {
    client: Client,
}

impl HttpServiceHandler {
    pub fn new(http_timeout: Option<Duration>, http_redirection_limit: usize) -> Self {
        Self {
            client: Client::new(http_timeout, http_redirection_limit),
        }
    }
}

impl DefaultServiceHandler for HttpServiceHandler {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        service_name: &NamedNode,
        pattern: &GraphPattern,
        base_iri: Option<&Iri<String>>,
    ) -> std::result::Result<QuerySolutionIter<'static>, Self::Error> {
        let (content_type, body) = self
            .client
            .post(
                service_name.as_str(),
                spargebra::Query::Select {
                    dataset: None,
                    pattern: pattern.clone(),
                    base_iri: base_iri.cloned(),
                }
                .to_string()
                .into_bytes(),
                "application/sparql-query",
                "application/sparql-results+json, application/sparql-results+xml",
            )
            .map_err(|e| QueryEvaluationError::Service(Box::new(e)))?;
        let format = QueryResultsFormat::from_media_type(&content_type).ok_or_else(|| {
            QueryEvaluationError::Service(
                format!(
                    "Unsupported Content-Type returned by service {service_name}: {content_type}"
                )
                .into(),
            )
        })?;
        let ReaderQueryResultsParserOutput::Solutions(reader) =
            QueryResultsParser::from_format(format)
                .for_reader(body)
                .map_err(|e| QueryEvaluationError::Service(Box::new(e)))?
        else {
            return Err(QueryEvaluationError::Service(
                "No valid SPARQL solutions returned by {service_name}".into(),
            ));
        };
        Ok(QuerySolutionIter::new(
            reader.variables().into(),
            Box::new(reader.map(|t| t.map_err(|e| QueryEvaluationError::Service(Box::new(e))))),
        ))
    }
}

fn invalid_data_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
    Error::new(ErrorKind::InvalidData, error)
}

fn invalid_input_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
    Error::new(ErrorKind::InvalidInput, error)
}
