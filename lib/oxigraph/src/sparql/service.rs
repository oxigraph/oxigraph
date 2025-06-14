use crate::model::NamedNode;
use crate::sparql::error::EvaluationError;
use crate::sparql::http::Client;
use crate::sparql::results::QueryResultsFormat;
use oxiri::Iri;
use sparesults::{QueryResultsParser, ReaderQueryResultsParserOutput};
use spareval::{DefaultServiceHandler, QueryEvaluationError, QuerySolutionIter};
use spargebra::algebra::GraphPattern;
use std::time::Duration;

pub struct SimpleServiceHandler {
    client: Client,
}

impl SimpleServiceHandler {
    pub fn new(http_timeout: Option<Duration>, http_redirection_limit: usize) -> Self {
        Self {
            client: Client::new(http_timeout, http_redirection_limit),
        }
    }
}

impl DefaultServiceHandler for SimpleServiceHandler {
    type Error = EvaluationError;

    fn handle(
        &self,
        service_name: NamedNode,
        pattern: GraphPattern,
        base_iri: Option<String>,
    ) -> Result<QuerySolutionIter, Self::Error> {
        let (content_type, body) = self
            .client
            .post(
                service_name.as_str(),
                spargebra::Query::Select {
                    dataset: None,
                    pattern,
                    base_iri: base_iri
                        .map(Iri::parse)
                        .transpose()
                        .map_err(|e| EvaluationError::Service(Box::new(e)))?,
                }
                .to_string()
                .into_bytes(),
                "application/sparql-query",
                "application/sparql-results+json, application/sparql-results+xml",
            )
            .map_err(|e| EvaluationError::Service(Box::new(e)))?;
        let format = QueryResultsFormat::from_media_type(&content_type)
            .ok_or_else(|| EvaluationError::UnsupportedContentType(content_type))?;
        let ReaderQueryResultsParserOutput::Solutions(reader) =
            QueryResultsParser::from_format(format).for_reader(body)?
        else {
            return Err(EvaluationError::ServiceDoesNotReturnSolutions);
        };
        Ok(QuerySolutionIter::new(
            reader.variables().into(),
            Box::new(reader.map(|t| t.map_err(|e| QueryEvaluationError::Service(Box::new(e))))),
        ))
    }
}
