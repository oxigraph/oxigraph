#![deny(
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_qualifications
)]

use argh::FromArgs;
use async_std::future::Future;
use async_std::io::Read;
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task::{block_on, spawn, spawn_blocking};
use http_types::{headers, Body, Error, Method, Mime, Request, Response, Result, StatusCode};
use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::{GraphName, NamedNode, NamedOrBlankNode};
use oxigraph::sparql::{Query, QueryResults, QueryResultsFormat, Update};
use std::io::BufReader;
use std::str::FromStr;
use url::form_urlencoded;

#[cfg(feature = "rocksdb")]
use oxigraph::RocksDbStore as Store;
#[cfg(all(feature = "sled", not(feature = "rocksdb")))]
use oxigraph::SledStore as Store;

const MAX_SPARQL_BODY_SIZE: u64 = 1_048_576;
const HTML_ROOT_PAGE: &str = include_str!("../templates/query.html");
const LOGO: &str = include_str!("../../logo.svg");
const SERVER: &str = concat!("Oxigraph/", env!("CARGO_PKG_VERSION"));

#[derive(FromArgs)]
/// Oxigraph SPARQL server
struct Args {
    /// specify a server socket to bind using the format $(HOST):$(PORT)
    #[argh(option, short = 'b', default = "\"localhost:7878\".to_string()")]
    bind: String,

    /// directory in which persist the data
    #[argh(option, short = 'f')]
    file: String,
}

#[async_std::main]
pub async fn main() -> Result<()> {
    let args: Args = argh::from_env();
    let store = Store::open(args.file)?;

    println!("Listening for requests at http://{}", &args.bind);
    http_server(&args.bind, move |request| {
        handle_request(request, store.clone())
    })
    .await
}

async fn handle_request(request: Request, store: Store) -> Result<Response> {
    let mut response = match (request.url().path(), request.method()) {
        ("/", Method::Get) => {
            let mut response = Response::new(StatusCode::Ok);
            response.append_header(headers::CONTENT_TYPE, "text/html");
            response.set_body(HTML_ROOT_PAGE);
            response
        }
        ("/logo.svg", Method::Get) => {
            let mut response = Response::new(StatusCode::Ok);
            response.append_header(headers::CONTENT_TYPE, "image/svg+xml");
            response.set_body(LOGO);
            response
        }
        ("/", Method::Post) => {
            if let Some(content_type) = request.content_type() {
                match if let Some(format) = GraphFormat::from_media_type(content_type.essence()) {
                    spawn_blocking(move || {
                        store.load_graph(
                            BufReader::new(SyncAsyncReader::from(request)),
                            format,
                            &GraphName::DefaultGraph,
                            None,
                        )
                    })
                } else if let Some(format) = DatasetFormat::from_media_type(content_type.essence())
                {
                    spawn_blocking(move || {
                        store.load_dataset(
                            BufReader::new(SyncAsyncReader::from(request)),
                            format,
                            None,
                        )
                    })
                } else {
                    return Ok(simple_response(
                        StatusCode::UnsupportedMediaType,
                        format!("No supported content Content-Type given: {}", content_type),
                    ));
                }
                .await
                {
                    Ok(()) => Response::new(StatusCode::NoContent),
                    Err(error) => {
                        return Err(bad_request(error));
                    }
                }
            } else {
                simple_response(StatusCode::BadRequest, "No Content-Type given")
            }
        }
        ("/query", Method::Get) => {
            evaluate_urlencoded_sparql_query(
                store,
                request.url().query().unwrap_or("").as_bytes().to_vec(),
                request,
            )
            .await?
        }
        ("/query", Method::Post) => {
            if let Some(content_type) = request.content_type() {
                if content_type.essence() == "application/sparql-query" {
                    let mut buffer = String::new();
                    let mut request = request;
                    request
                        .take_body()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_string(&mut buffer)
                        .await?;
                    evaluate_sparql_query(store, buffer, Vec::new(), Vec::new(), request).await?
                } else if content_type.essence() == "application/x-www-form-urlencoded" {
                    let mut buffer = Vec::new();
                    let mut request = request;
                    request
                        .take_body()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_end(&mut buffer)
                        .await?;
                    evaluate_urlencoded_sparql_query(store, buffer, request).await?
                } else {
                    simple_response(
                        StatusCode::UnsupportedMediaType,
                        format!("Not supported Content-Type given: {}", content_type),
                    )
                }
            } else {
                simple_response(StatusCode::BadRequest, "No Content-Type given")
            }
        }
        ("/update", Method::Post) => {
            if let Some(content_type) = request.content_type() {
                if content_type.essence() == "application/sparql-update" {
                    let mut buffer = String::new();
                    let mut request = request;
                    request
                        .take_body()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_string(&mut buffer)
                        .await?;
                    evaluate_sparql_update(store, buffer, Vec::new(), Vec::new()).await?
                } else if content_type.essence() == "application/x-www-form-urlencoded" {
                    let mut buffer = Vec::new();
                    let mut request = request;
                    request
                        .take_body()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_end(&mut buffer)
                        .await?;
                    evaluate_urlencoded_sparql_update(store, buffer).await?
                } else {
                    simple_response(
                        StatusCode::UnsupportedMediaType,
                        format!("Not supported Content-Type given: {}", content_type),
                    )
                }
            } else {
                simple_response(StatusCode::BadRequest, "No Content-Type given")
            }
        }
        _ => Response::new(StatusCode::NotFound),
    };
    response.append_header(headers::SERVER, SERVER);
    Ok(response)
}

fn simple_response(status: StatusCode, body: impl Into<Body>) -> Response {
    let mut response = Response::new(status);
    response.set_body(body);
    response
}

async fn evaluate_urlencoded_sparql_query(
    store: Store,
    encoded: Vec<u8>,
    request: Request,
) -> Result<Response> {
    let mut query = None;
    let mut default_graph_uris = Vec::new();
    let mut named_graph_uris = Vec::new();
    for (k, v) in form_urlencoded::parse(&encoded) {
        match k.as_ref() {
            "query" => query = Some(v.into_owned()),
            "default-graph-uri" => default_graph_uris.push(v.into_owned()),
            "named-graph-uri" => named_graph_uris.push(v.into_owned()),
            _ => {
                return Ok(simple_response(
                    StatusCode::BadRequest,
                    format!("Unexpected parameter: {}", k),
                ))
            }
        }
    }
    if let Some(query) = query {
        evaluate_sparql_query(store, query, default_graph_uris, named_graph_uris, request).await
    } else {
        Ok(simple_response(
            StatusCode::BadRequest,
            "You should set the 'query' parameter",
        ))
    }
}

async fn evaluate_sparql_query(
    store: Store,
    query: String,
    default_graph_uris: Vec<String>,
    named_graph_uris: Vec<String>,
    request: Request,
) -> Result<Response> {
    spawn_blocking(move || {
        let mut query = Query::parse(&query, None).map_err(bad_request)?;
        let default_graph_uris = default_graph_uris
            .into_iter()
            .map(|e| Ok(NamedNode::new(e)?.into()))
            .collect::<Result<Vec<GraphName>>>()
            .map_err(bad_request)?;
        let named_graph_uris = named_graph_uris
            .into_iter()
            .map(|e| Ok(NamedNode::new(e)?.into()))
            .collect::<Result<Vec<NamedOrBlankNode>>>()
            .map_err(bad_request)?;

        if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
            query.dataset_mut().set_default_graph(default_graph_uris);
            query
                .dataset_mut()
                .set_available_named_graphs(named_graph_uris);
        }

        let results = store.query(query)?;
        //TODO: stream
        if let QueryResults::Graph(_) = results {
            let format = content_negotiation(
                request,
                &[
                    GraphFormat::NTriples.media_type(),
                    GraphFormat::Turtle.media_type(),
                    GraphFormat::RdfXml.media_type(),
                ],
                GraphFormat::from_media_type,
            )?;
            let mut body = Vec::default();
            results.write_graph(&mut body, format)?;
            let mut response = Response::from(body);
            response.insert_header(headers::CONTENT_TYPE, format.media_type());
            Ok(response)
        } else {
            let format = content_negotiation(
                request,
                &[
                    QueryResultsFormat::Xml.media_type(),
                    QueryResultsFormat::Json.media_type(),
                    QueryResultsFormat::Csv.media_type(),
                    QueryResultsFormat::Tsv.media_type(),
                ],
                QueryResultsFormat::from_media_type,
            )?;
            let mut body = Vec::default();
            results.write(&mut body, format)?;
            let mut response = Response::from(body);
            response.insert_header(headers::CONTENT_TYPE, format.media_type());
            Ok(response)
        }
    })
    .await
}

async fn evaluate_urlencoded_sparql_update(store: Store, encoded: Vec<u8>) -> Result<Response> {
    let mut update = None;
    let mut default_graph_uris = Vec::new();
    let mut named_graph_uris = Vec::new();
    for (k, v) in form_urlencoded::parse(&encoded) {
        match k.as_ref() {
            "update" => update = Some(v.into_owned()),
            "using-graph-uri" => default_graph_uris.push(v.into_owned()),
            "using-named-graph-uri" => named_graph_uris.push(v.into_owned()),
            _ => {
                return Ok(simple_response(
                    StatusCode::BadRequest,
                    format!("Unexpected parameter: {}", k),
                ))
            }
        }
    }
    if let Some(update) = update {
        evaluate_sparql_update(store, update, default_graph_uris, named_graph_uris).await
    } else {
        Ok(simple_response(
            StatusCode::BadRequest,
            "You should set the 'update' parameter",
        ))
    }
}

async fn evaluate_sparql_update(
    store: Store,
    update: String,
    default_graph_uris: Vec<String>,
    named_graph_uris: Vec<String>,
) -> Result<Response> {
    if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
        return Ok(simple_response(
            StatusCode::BadRequest,
            "using-graph-uri and using-named-graph-uri parameters are not supported yet",
        ));
    }
    spawn_blocking(move || {
        let update = Update::parse(&update, None).map_err(|e| {
            let mut e = Error::from(e);
            e.set_status(StatusCode::BadRequest);
            e
        })?;
        store.update(update)?;
        Ok(Response::new(StatusCode::NoContent))
    })
    .await
}

async fn http_server<
    F: Clone + Send + Sync + 'static + Fn(Request) -> Fut,
    Fut: Send + Future<Output = Result<Response>>,
>(
    host: &str,
    handle: F,
) -> Result<()> {
    async fn accept<F: Fn(Request) -> Fut, Fut: Future<Output = Result<Response>>>(
        stream: TcpStream,
        handle: F,
    ) -> Result<()> {
        async_h1::accept(stream, |request| async {
            Ok(match handle(request).await {
                Ok(result) => result,
                Err(error) => simple_response(error.status(), error.to_string()),
            })
        })
        .await
    }

    let listener = TcpListener::bind(host).await?;
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        let handle = handle.clone();
        spawn(async {
            if let Err(err) = accept(stream, handle).await {
                eprintln!("{}", err);
            };
        });
    }
    Ok(())
}

fn content_negotiation<F>(
    request: Request,
    supported: &[&str],
    parse: impl Fn(&str) -> Option<F>,
) -> Result<F> {
    let header = request
        .header(headers::ACCEPT)
        .map(|h| h.last().as_str().trim())
        .unwrap_or("");
    let supported: Vec<Mime> = supported
        .iter()
        .map(|h| Mime::from_str(h).unwrap())
        .collect();

    let mut result = supported.first().unwrap();
    let mut result_score = 0f32;

    if !header.is_empty() {
        for possible in header.split(',') {
            let possible = Mime::from_str(possible.trim())?;
            let score = if let Some(q) = possible.param("q") {
                f32::from_str(&q.to_string())?
            } else {
                1.
            };
            if score <= result_score {
                continue;
            }
            for candidate in &supported {
                if (possible.basetype() == candidate.basetype() || possible.basetype() == "*")
                    && (possible.subtype() == candidate.subtype() || possible.subtype() == "*")
                {
                    result = candidate;
                    result_score = score;
                    break;
                }
            }
        }
    }

    parse(result.essence())
        .ok_or_else(|| Error::from_str(StatusCode::InternalServerError, "Unknown mime type"))
}

fn bad_request(e: impl Into<Error>) -> Error {
    let mut e = e.into();
    e.set_status(StatusCode::BadRequest);
    e
}

struct SyncAsyncReader<R: Unpin> {
    inner: R,
}

impl<R: Unpin> From<R> for SyncAsyncReader<R> {
    fn from(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: Read + Unpin> std::io::Read for SyncAsyncReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        block_on(self.inner.read(buf))
    }

    //TODO: implement other methods
}

#[cfg(test)]
mod tests {
    use super::Store;
    use crate::handle_request;
    use async_std::task::block_on;
    use http_types::{Method, Request, StatusCode, Url};
    use std::collections::hash_map::DefaultHasher;
    use std::env::temp_dir;
    use std::fs::remove_dir_all;
    use std::hash::{Hash, Hasher};

    #[test]
    fn get_ui() {
        exec(
            Request::new(Method::Get, Url::parse("http://localhost/").unwrap()),
            StatusCode::Ok,
        )
    }

    #[test]
    fn post_file() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/").unwrap());
        request.insert_header("Content-Type", "text/turtle");
        request.set_body("<http://example.com> <http://example.com> <http://example.com> .");
        exec(request, StatusCode::NoContent)
    }

    #[test]
    fn post_wrong_file() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/").unwrap());
        request.insert_header("Content-Type", "text/turtle");
        request.set_body("<http://example.com>");
        exec(request, StatusCode::BadRequest)
    }

    #[test]
    fn post_unsupported_file() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/").unwrap());
        request.insert_header("Content-Type", "text/foo");
        exec(request, StatusCode::UnsupportedMediaType)
    }

    #[test]
    fn get_query() {
        exec(
            Request::new(
                Method::Get,
                Url::parse(
                    "http://localhost/query?query=SELECT%20*%20WHERE%20{%20?s%20?p%20?o%20}",
                )
                .unwrap(),
            ),
            StatusCode::Ok,
        );
    }

    #[test]
    fn get_bad_query() {
        exec(
            Request::new(
                Method::Get,
                Url::parse("http://localhost/query?query=SELECT").unwrap(),
            ),
            StatusCode::BadRequest,
        );
    }

    #[test]
    fn get_without_query() {
        exec(
            Request::new(Method::Get, Url::parse("http://localhost/query").unwrap()),
            StatusCode::BadRequest,
        );
    }

    #[test]
    fn post_query() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/query").unwrap());
        request.insert_header("Content-Type", "application/sparql-query");
        request.set_body("SELECT * WHERE { ?s ?p ?o }");
        exec(request, StatusCode::Ok)
    }

    #[test]
    fn post_bad_query() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/query").unwrap());
        request.insert_header("Content-Type", "application/sparql-query");
        request.set_body("SELECT");
        exec(request, StatusCode::BadRequest)
    }

    #[test]
    fn post_unknown_query() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/query").unwrap());
        request.insert_header("Content-Type", "application/sparql-todo");
        request.set_body("SELECT");
        exec(request, StatusCode::UnsupportedMediaType)
    }

    #[test]
    fn post_federated_query() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/query").unwrap());
        request.insert_header("Content-Type", "application/sparql-query");
        request.set_body("SELECT * WHERE { SERVICE <https://query.wikidata.org/sparql> { <https://en.wikipedia.org/wiki/Paris> ?p ?o } }");
        exec(request, StatusCode::Ok)
    }

    #[test]
    fn post_update() {
        let mut request =
            Request::new(Method::Post, Url::parse("http://localhost/update").unwrap());
        request.insert_header("Content-Type", "application/sparql-update");
        request.set_body(
            "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
        );
        exec(request, StatusCode::NoContent)
    }

    #[test]
    fn post_bad_update() {
        let mut request =
            Request::new(Method::Post, Url::parse("http://localhost/update").unwrap());
        request.insert_header("Content-Type", "application/sparql-update");
        request.set_body("INSERT");
        exec(request, StatusCode::BadRequest)
    }

    fn exec(request: Request, expected_status: StatusCode) {
        let mut path = temp_dir();
        path.push("temp-oxigraph-server-test");
        let mut s = DefaultHasher::new();
        format!("{:?}", request).hash(&mut s);
        path.push(&s.finish().to_string());

        let store = Store::open(&path).unwrap();
        let (code, message) = match block_on(handle_request(request, store)) {
            Ok(r) => (r.status(), "".to_string()),
            Err(e) => (e.status(), e.to_string()),
        };
        assert_eq!(code, expected_status, "Error message: {}", message);
        remove_dir_all(&path).unwrap()
    }
}
