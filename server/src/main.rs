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
use async_std::io::{BufRead, Read};
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task::{block_on, spawn, spawn_blocking};
use http_types::headers::HeaderName;
use http_types::{headers, Body, Error, Method, Mime, Request, Response, Result, StatusCode};
use oxigraph::sparql::{PreparedQuery, QueryOptions, QueryResult, QueryResultSyntax};
use oxigraph::{DatasetSyntax, FileSyntax, GraphSyntax, RocksDbStore};
use std::str::FromStr;
use url::form_urlencoded;

const MAX_SPARQL_BODY_SIZE: u64 = 1_048_576;
const HTML_ROOT_PAGE: &str = include_str!("../templates/query.html");
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
    let store = RocksDbStore::open(args.file)?;

    println!("Listening for requests at http://{}", &args.bind);
    http_server(args.bind, move |request| {
        handle_request(request, store.clone())
    })
    .await
}

async fn handle_request(request: Request, store: RocksDbStore) -> Result<Response> {
    let mut response = match (request.url().path(), request.method()) {
        ("/", Method::Get) => {
            let mut response = Response::new(StatusCode::Ok);
            response.append_header(headers::CONTENT_TYPE, "text/html")?;
            response.set_body(HTML_ROOT_PAGE);
            response
        }
        ("/", Method::Post) => {
            if let Some(content_type) = request.content_type() {
                match if let Some(format) = GraphSyntax::from_mime_type(essence(&content_type)) {
                    spawn_blocking(move || {
                        store.load_graph(SyncAsyncBufReader::from(request), format, None, None)
                    })
                } else if let Some(format) = DatasetSyntax::from_mime_type(essence(&content_type)) {
                    spawn_blocking(move || {
                        store.load_dataset(SyncAsyncBufReader::from(request), format, None)
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
                        let mut error = Error::from(error);
                        error.set_status(StatusCode::BadRequest);
                        return Err(error);
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
                if essence(&content_type) == "application/sparql-query" {
                    let mut buffer = String::new();
                    let mut request = request;
                    request
                        .take_body()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_string(&mut buffer)
                        .await?;
                    evaluate_sparql_query(store, buffer, request).await?
                } else if essence(&content_type) == "application/x-www-form-urlencoded" {
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
                        format!("No supported Content-Type given: {}", content_type),
                    )
                }
            } else {
                simple_response(StatusCode::BadRequest, "No Content-Type given")
            }
        }
        _ => Response::new(StatusCode::NotFound),
    };
    response.append_header("Server", SERVER)?;
    Ok(response)
}

/// TODO: bad hack to overcome http_types limitations
fn essence(mime: &Mime) -> &str {
    mime.essence().split(';').next().unwrap_or("")
}

fn simple_response(status: StatusCode, body: impl Into<Body>) -> Response {
    let mut response = Response::new(status);
    response.set_body(body);
    response
}

async fn evaluate_urlencoded_sparql_query(
    store: RocksDbStore,
    encoded: Vec<u8>,
    request: Request,
) -> Result<Response> {
    if let Some((_, query)) = form_urlencoded::parse(&encoded).find(|(k, _)| k == "query") {
        evaluate_sparql_query(store, query.to_string(), request).await
    } else {
        Ok(simple_response(
            StatusCode::BadRequest,
            "You should set the 'query' parameter",
        ))
    }
}

async fn evaluate_sparql_query(
    store: RocksDbStore,
    query: String,
    request: Request,
) -> Result<Response> {
    spawn_blocking(move || {
        //TODO: stream
        let query = store
            .prepare_query(&query, QueryOptions::default())
            .map_err(|e| {
                let mut e = Error::from(e);
                e.set_status(StatusCode::BadRequest);
                e
            })?;
        let results = query.exec()?;
        if let QueryResult::Graph(_) = results {
            let format = content_negotiation(
                request,
                &[
                    GraphSyntax::NTriples.media_type(),
                    GraphSyntax::Turtle.media_type(),
                    GraphSyntax::RdfXml.media_type(),
                ],
            )?;

            let mut response = Response::from(results.write_graph(Vec::default(), format)?);
            response.insert_header(headers::CONTENT_TYPE, format.media_type())?;
            Ok(response)
        } else {
            let format = content_negotiation(
                request,
                &[
                    QueryResultSyntax::Xml.media_type(),
                    QueryResultSyntax::Json.media_type(),
                ],
            )?;
            let mut response = Response::from(results.write(Vec::default(), format)?);
            response.insert_header(headers::CONTENT_TYPE, format.media_type())?;
            Ok(response)
        }
    })
    .await
}

async fn http_server<
    F: Clone + Send + Sync + 'static + Fn(Request) -> Fut,
    Fut: Send + Future<Output = Result<Response>>,
>(
    host: String,
    handle: F,
) -> Result<()> {
    async fn accept<F: Fn(Request) -> Fut, Fut: Future<Output = Result<Response>>>(
        addr: String,
        stream: TcpStream,
        handle: F,
    ) -> Result<()> {
        async_h1::accept(&addr, stream, |request| async {
            Ok(match handle(request).await {
                Ok(result) => result,
                Err(error) => simple_response(error.status(), error.to_string()),
            })
        })
        .await
    }

    let listener = TcpListener::bind(&host).await?;
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?.clone(); //TODO: clone stream?
        let handle = handle.clone();
        let addr = format!("http://{}", host);
        spawn(async {
            if let Err(err) = accept(addr, stream, handle).await {
                eprintln!("{}", err);
            };
        });
    }
    Ok(())
}

fn content_negotiation<F: FileSyntax>(request: Request, supported: &[&str]) -> Result<F> {
    let header = request
        .header(&HeaderName::from_str("Accept").unwrap())
        .and_then(|h| h.last())
        .map(|h| h.as_str().trim())
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
                f32::from_str(q)?
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

    F::from_mime_type(essence(result))
        .ok_or_else(|| Error::from_str(StatusCode::InternalServerError, "Unknown mime type"))
}

struct SyncAsyncBufReader<R: Unpin> {
    inner: R,
}

impl<R: Unpin> From<R> for SyncAsyncBufReader<R> {
    fn from(inner: R) -> Self {
        Self { inner }
    }
}

impl<R: Read + Unpin> std::io::Read for SyncAsyncBufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        block_on(self.inner.read(buf))
    }

    //TODO: implement other methods
}

impl<R: BufRead + Unpin> std::io::BufRead for SyncAsyncBufReader<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        unimplemented!()
    }

    fn consume(&mut self, amt: usize) {
        unimplemented!()
    }

    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> std::io::Result<usize> {
        block_on(self.inner.read_until(byte, buf))
    }

    fn read_line(&mut self, buf: &mut String) -> std::io::Result<usize> {
        block_on(self.inner.read_line(buf))
    }
}

#[cfg(test)]
mod tests {
    use crate::handle_request;
    use async_std::task::block_on;
    use http_types::{Method, Request, StatusCode, Url};
    use oxigraph::RocksDbStore;
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
        request
            .insert_header("Content-Type", "text/turtle")
            .unwrap();
        request.set_body("<http://example.com> <http://example.com> <http://example.com> .");
        exec(request, StatusCode::NoContent)
    }

    #[test]
    fn post_wrong_file() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/").unwrap());
        request
            .insert_header("Content-Type", "text/turtle")
            .unwrap();
        request.set_body("<http://example.com>");
        exec(request, StatusCode::BadRequest)
    }

    #[test]
    fn post_unsupported_file() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/").unwrap());
        request.insert_header("Content-Type", "text/plain").unwrap();
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
        request
            .insert_header("Content-Type", "application/sparql-query")
            .unwrap();
        request.set_body("SELECT * WHERE { ?s ?p ?o }");
        exec(request, StatusCode::Ok)
    }

    #[test]
    fn post_bad_query() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/query").unwrap());
        request
            .insert_header("Content-Type", "application/sparql-query")
            .unwrap();
        request.set_body("SELECT");
        exec(request, StatusCode::BadRequest)
    }

    #[test]
    fn post_unknown_query() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/query").unwrap());
        request
            .insert_header("Content-Type", "application/sparql-todo")
            .unwrap();
        request.set_body("SELECT");
        exec(request, StatusCode::UnsupportedMediaType)
    }

    fn exec(request: Request, expected_status: StatusCode) {
        let mut path = temp_dir();
        path.push("temp-oxigraph-server-test");
        let mut s = DefaultHasher::new();
        format!("{:?}", request).hash(&mut s);
        path.push(&s.finish().to_string());

        let store = RocksDbStore::open(&path).unwrap();
        assert_eq!(
            match block_on(handle_request(request, store)) {
                Ok(r) => r.status(),
                Err(e) => e.status(),
            },
            expected_status
        );
        remove_dir_all(&path).unwrap()
    }
}
