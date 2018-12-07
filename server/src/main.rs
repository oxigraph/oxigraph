extern crate clap;
#[macro_use]
extern crate failure;
extern crate futures;
extern crate gotham;
#[macro_use]
extern crate gotham_derive;
extern crate hyper;
#[macro_use]
extern crate lazy_static;
extern crate mime;
extern crate rudf;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate tera;
extern crate url;

use clap::App;
use clap::Arg;
use clap::ArgMatches;
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
use mime::Mime;
use rudf::model::Graph;
use rudf::rio::ntriples::read_ntriples;
use rudf::sparql::algebra::QueryResult;
use rudf::sparql::xml_results::write_xml_results;
use rudf::sparql::PreparedQuery;
use rudf::sparql::SparqlDataset;
use rudf::store::MemoryDataset;
use rudf::store::MemoryGraph;
use rudf::store::RocksDbDataset;
use std::fs::File;
use std::panic::RefUnwindSafe;
use std::str::FromStr;
use std::sync::Arc;
use tera::Context;
use tera::Tera;
use url::form_urlencoded;

lazy_static! {
    static ref TERA: Tera = {
        let mut tera = compile_templates!("templates/**/*");
        tera.autoescape_on(vec![]);
        tera
    };
    static ref APPLICATION_SPARQL_QUERY_UTF_8: Mime =
        "application/sparql-query; charset=utf-8".parse().unwrap();
    static ref APPLICATION_SPARQL_RESULTS_UTF_8: Mime =
        "application/sparql-results; charset=utf-8".parse().unwrap();
    static ref APPLICATION_N_TRIPLES_UTF_8: Mime =
        "application/n-triples; charset=utf-8".parse().unwrap();
}

pub fn main() -> Result<(), failure::Error> {
    let matches = App::new("Rudf SPARQL server")
        .arg(
            Arg::with_name("bind")
                .short("b")
                .long("bind")
                .help("Specify a server socket to bind using the format $(HOST):$(PORT)")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("ntriples")
                .long("ntriples")
                .help("Load a N-Triples file in the server at startup")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("file")
                .long("file")
                .short("f")
                .help("File in which persist the dataset")
                .takes_value(true),
        )
        .get_matches();

    let file = matches.value_of("file").map(|v| v.to_string());
    if let Some(file) = file {
        main_with_dataset(Arc::new(RocksDbDataset::open(file)?), &matches)
    } else {
        main_with_dataset(Arc::new(MemoryDataset::default()), &matches)
    }
}

fn main_with_dataset<D: SparqlDataset + Send + Sync + RefUnwindSafe + 'static>(
    dataset: Arc<D>,
    matches: &ArgMatches,
) -> Result<(), failure::Error> {
    if let Some(nt_file) = matches.value_of("ntriples") {
        println!("Loading NTriples file {}", nt_file);
        let default_graph = dataset.default_graph();
        for quad in read_ntriples(File::open(nt_file)?) {
            default_graph.insert(&quad?)?
        }
    }

    let addr = matches.value_of("bind").unwrap_or("127.0.0.1:7878");
    println!("Listening for requests at http://{}", addr);
    gotham::start(addr.to_string(), router(dataset, addr.to_string()));
    Ok(())
}

fn router<D: SparqlDataset + Send + Sync + RefUnwindSafe + 'static>(
    dataset: Arc<D>,
    base: String,
) -> Router {
    let middleware = StateMiddleware::new(GothamState { dataset, base });
    let pipeline = single_middleware(middleware);
    let (chain, pipelines) = single_pipeline(pipeline);
    build_router(chain, pipelines, |route| {
        route
            .get("/")
            .to(|mut state: State| -> (State, Response<Body>) {
                let gotham_state: GothamState<D> = GothamState::take_from(&mut state);
                let mut context = Context::new();
                context.insert("endpoint", &format!("//{}/query", gotham_state.base));
                let response = create_response(
                    &state,
                    StatusCode::OK,
                    mime::TEXT_HTML_UTF_8,
                    TERA.render("query.html", &context).unwrap(),
                );
                (state, response)
            });
        route.associate("/query", |assoc| {
            assoc
                .get()
                .with_query_string_extractor::<QueryRequest>()
                .to(|mut state: State| -> (State, Response<Body>) {
                    let parsed_request = QueryRequest::take_from(&mut state);
                    let response =
                        evaluate_sparql_query::<D>(&mut state, &parsed_request.query.as_bytes());
                    (state, response)
                });
            assoc.post().to(|mut state: State| -> Box<HandlerFuture> {
                Box::new(
                    Body::take_from(&mut state)
                        .concat2()
                        .then(|body| match body {
                            Ok(body) => {
                                let content_type: Option<Result<Mime,failure::Error>> = HeaderMap::borrow_from(&state)
                                    .get(CONTENT_TYPE)
                                    .map(|content_type| Ok(Mime::from_str(content_type.to_str()?)?));
                                let response = match content_type {
                                        Some(Ok(content_type)) => match (content_type.type_(), content_type.subtype()) {
                                            (mime::APPLICATION, subtype) if subtype == APPLICATION_SPARQL_QUERY_UTF_8.subtype() => {
                                                evaluate_sparql_query::<D>(
                                                    &mut state,
                                                    &body.into_bytes(),
                                                )
                                            },
                                            (mime::APPLICATION, mime::WWW_FORM_URLENCODED) => {
                                                match parse_urlencoded_query_request(&body.into_bytes())
                                                    {
                                                        Ok(parsed_request) => evaluate_sparql_query::<D>(
                                                            &mut state,
                                                            &parsed_request.query.as_bytes(),
                                                        ),
                                                        Err(error) => error_to_response(
                                                            &state,
                                                            &error,
                                                            StatusCode::BAD_REQUEST,
                                                        ),
                                                    }
                                            },
                                            _ => error_to_response(
                                                    &state,
                                                    &format_err!("Unsupported Content-Type: {:?}", content_type),
                                                    StatusCode::BAD_REQUEST,
                                                )
                                        }
                                        Some(Err(error)) => error_to_response(
                                            &state,
                                            &format_err!("The request  contains an invalid Content-Type header: {}", error),
                                            StatusCode::BAD_REQUEST,
                                        ),
                                        None => error_to_response(
                                            &state,
                                            &format_err!("The request should contain a Content-Type header"),
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

#[derive(StateData)]
struct GothamState<D: SparqlDataset + Send + Sync + RefUnwindSafe + 'static> {
    dataset: Arc<D>,
    base: String,
}

impl<D: SparqlDataset + Send + Sync + RefUnwindSafe + 'static> Clone for GothamState<D> {
    fn clone(&self) -> Self {
        Self {
            dataset: self.dataset.clone(),
            base: self.base.clone(),
        }
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
        })
        .ok_or_else(|| format_err!("'query' parameter not found"))
}

fn evaluate_sparql_query<D: SparqlDataset + Send + Sync + RefUnwindSafe + 'static>(
    state: &mut State,
    query: &[u8],
) -> Response<Body> {
    let gotham_state: GothamState<D> = GothamState::take_from(state);
    match gotham_state.dataset.prepare_query(query) {
        Ok(query) => match query.exec() {
            Ok(QueryResult::Graph(triples)) => {
                let triples: Result<MemoryGraph, failure::Error> = triples.collect();
                create_response(
                    &state,
                    StatusCode::OK,
                    APPLICATION_N_TRIPLES_UTF_8.clone(),
                    triples.unwrap().to_string(),
                )
            }
            Ok(result) => create_response(
                &state,
                StatusCode::OK,
                APPLICATION_SPARQL_RESULTS_UTF_8.clone(),
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
    fn get_ui() {
        let test_server =
            TestServer::new(router(Arc::new(MemoryDataset::default()), "".to_string())).unwrap();
        let response = test_server
            .client()
            .get("http://localhost/")
            .perform()
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn get_query() {
        let test_server =
            TestServer::new(router(Arc::new(MemoryDataset::default()), "".to_string())).unwrap();
        let response = test_server
            .client()
            .get("http://localhost/query?query=SELECT+*+WHERE+{+?s+?p+?o+}")
            .perform()
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[test]
    fn post_query() {
        let test_server =
            TestServer::new(router(Arc::new(MemoryDataset::default()), "".to_string())).unwrap();
        let response = test_server
            .client()
            .post(
                "http://localhost/query",
                "SELECT * WHERE { ?s ?p ?o }",
                Mime::from_str("application/sparql-query").unwrap(),
            )
            .perform()
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }
}
