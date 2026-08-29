use crate::http::HttpClient;
use crate::model::NamedNode;
use oxiri::Iri;
use oxstr::OxString;
use sparesults::{QueryResultsFormat, QueryResultsParser, ReaderQueryResultsParserOutput};
use spareval::{DefaultServiceHandler, QueryEvaluationError, QuerySolutionIter};
use spargebra::algebra::QueryExpression;
use spargebra::query::SelectQuery;
use std::time::Duration;

pub struct HttpServiceHandler {
    client: HttpClient,
}

impl HttpServiceHandler {
    pub fn new(http_timeout: Option<Duration>, http_redirection_limit: usize) -> Self {
        Self {
            client: HttpClient::new(http_timeout, http_redirection_limit),
        }
    }
}

impl DefaultServiceHandler for HttpServiceHandler {
    type Error = QueryEvaluationError;

    fn handle(
        &self,
        service_name: &NamedNode,
        expression: &QueryExpression,
        base_iri: Option<&Iri<OxString>>,
    ) -> Result<QuerySolutionIter<'static>, Self::Error> {
        let (content_type, body) = self
            .client
            .post(
                service_name.as_str(),
                SelectQuery {
                    dataset: None,
                    expression: expression.clone(),
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
