extern crate gotham;
#[macro_use]
extern crate gotham_derive;
extern crate futures;
extern crate hyper;
extern crate mime;
extern crate rudf;
extern crate serde;
extern crate url;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate failure;
extern crate clap;

use clap::App;
use clap::Arg;
use futures::future;
use futures::Future;
use futures::Stream;
use gotham::handler::{HandlerFuture, IntoHandlerError};
use gotham::helpers::http::response::create_response;
use gotham::middleware::state::StateMiddleware;
use gotham::pipeline::single::single_pipeline;
use gotham::pipeline::single_middleware;
use gotham::router::builder::build_router;
use gotham::router::builder::DefineSingleRoute;
use gotham::router::builder::DrawRoutes;
use gotham::router::Router;
use gotham::state::FromState;
use gotham::state::State;
use hyper::header::CONTENT_TYPE;
use hyper::Body;
use hyper::HeaderMap;
use hyper::Response;
use hyper::StatusCode;
use rudf::model::Dataset;
use rudf::model::Graph;
use rudf::rio::ntriples::read_ntriples;
use rudf::sparql::xml_results::write_xml_results;
use rudf::sparql::PreparedQuery;
use rudf::sparql::SparqlDataset;
use rudf::store::MemoryDataset;
use std::fs::File;
use std::sync::Arc;
use url::form_urlencoded;

pub fn main() -> Result<(), failure::Error> {
    let matches = App::new("Rudf SPARQL server")
        .arg(
            Arg::with_name("bind")
                .short("b")
                .long("bind")
                .help("Specify a server socket to bind using the format $(HOST):$(PORT)")
                .takes_value(true),
        ).arg(
            Arg::with_name("ntriples")
                .long("ntriples")
                .help("Load a N-Triples file in the server at startup")
                .takes_value(true),
        ).get_matches();

    let dataset = MemoryDataset::default();
    if let Some(nt_file) = matches.value_of("ntriples") {
        println!("Loading NTriples file {}", nt_file);
        let default_graph = dataset.default_graph();
        for quad in read_ntriples(File::open(nt_file)?) {
            default_graph.insert(&quad?)?
        }
    }

    let addr = matches.value_of("bind").unwrap_or("127.0.0.1:7878");
    println!("Listening for requests at http://{}", addr);
    gotham::start(addr.to_string(), router(dataset));
    Ok(())
}

fn router(dataset: MemoryDataset) -> Router {
    let store = SparqlStore::new(dataset);
    let middleware = StateMiddleware::new(store);
    let pipeline = single_middleware(middleware);
    let (chain, pipelines) = single_pipeline(pipeline);
    build_router(chain, pipelines, |route| {
        route.associate("/query", |assoc| {
            assoc
                .get()
                .with_query_string_extractor::<QueryRequest>()
                .to(|mut state: State| -> (State, Response<Body>) {
                    let parsed_request = QueryRequest::take_from(&mut state);
                    let response =
                        evaluate_sparql_query(&mut state, &parsed_request.query.as_bytes());
                    (state, response)
                });
            assoc.post().to(|mut state: State| -> Box<HandlerFuture> {
                Box::new(
                    Body::take_from(&mut state)
                        .concat2()
                        .then(|body| match body {
                            Ok(body) => {
                                let response = match HeaderMap::borrow_from(&state)
                                    .get(CONTENT_TYPE)
                                    .cloned()
                                {
                                    Some(content_type) => {
                                        if content_type == "application/sparql-query" {
                                            evaluate_sparql_query(&mut state, &body.into_bytes())
                                        } else if content_type
                                            == "application/x-www-form-urlencoded"
                                        {
                                            match parse_urlencoded_query_request(&body.into_bytes())
                                            {
                                                Ok(parsed_request) => evaluate_sparql_query(
                                                    &mut state,
                                                    &parsed_request.query.as_bytes(),
                                                ),
                                                Err(error) => error_to_response(
                                                    &state,
                                                    &error,
                                                    StatusCode::BAD_REQUEST,
                                                ),
                                            }
                                        } else {
                                            error_to_response(
                                                &state,
                                                &format_err!(
                                                    "Unsupported Content-Type: {:?}",
                                                    content_type
                                                ),
                                                StatusCode::BAD_REQUEST,
                                            )
                                        }
                                    }
                                    None => error_to_response(
                                        &state,
                                        &format_err!(
                                            "The request should contain a Content-Type header"
                                        ),
                                        StatusCode::BAD_REQUEST,
                                    ),
                                };
                                future::ok((state, response))
                            }
                            Err(e) => future::err((state, e.into_handler_error())),
                        }),
                )
            });
        })
    })
}

#[derive(Clone, StateData)]
struct SparqlStore(Arc<MemoryDataset>);

impl SparqlStore {
    fn new(dataset: MemoryDataset) -> Self {
        SparqlStore(Arc::new(dataset))
    }
}

impl AsRef<MemoryDataset> for SparqlStore {
    fn as_ref(&self) -> &MemoryDataset {
        &*self.0
    }
}

#[derive(Deserialize, StateData, StaticResponseExtender)]
struct QueryRequest {
    query: String,
}

fn parse_urlencoded_query_request(query: &[u8]) -> Result<QueryRequest, failure::Error> {
    form_urlencoded::parse(query)
        .find(|(key, _)| key == "query")
        .map(|(_, value)| QueryRequest {
            query: value.to_string(),
        }).ok_or_else(|| format_err!("'query' parameter not found"))
}

fn evaluate_sparql_query(state: &mut State, query: &[u8]) -> Response<Body> {
    let dataset = SparqlStore::take_from(state);
    match dataset.as_ref().prepare_query(query) {
        Ok(query) => match query.exec() {
            Ok(result) => create_response(
                &state,
                StatusCode::OK,
                "application/sparql-results+xml".parse().unwrap(),
                write_xml_results(result, Vec::default()).unwrap(),
            ),
            Err(error) => error_to_response(&state, &error, StatusCode::INTERNAL_SERVER_ERROR),
        },
        Err(error) => error_to_response(&state, &error, StatusCode::BAD_REQUEST),
    }
}

fn error_to_response(state: &State, error: &failure::Error, code: StatusCode) -> Response<Body> {
    create_response(state, code, mime::TEXT_PLAIN_UTF_8, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gotham::test::TestServer;
    use mime::Mime;
    use std::str::FromStr;

    #[test]
    fn get_query() {
        let test_server = TestServer::new(router(MemoryDataset::default())).unwrap();
        let response = test_server
            .client()
            .get("http://localhost/query?query=SELECT+*+WHERE+{+?s+?p+?o+}")
            .perform()
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn post_query() {
        let test_server = TestServer::new(router(MemoryDataset::default())).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/query",
                "SELECT * WHERE { ?s ?p ?o }",
                Mime::from_str("application/sparql-query").unwrap(),
            ).perform()
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
