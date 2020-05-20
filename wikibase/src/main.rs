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

use crate::loader::WikibaseLoader;
use argh::FromArgs;
use async_std::future::Future;
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::sync::Arc;
use async_std::task::{spawn, spawn_blocking};
use http_types::headers::HeaderName;
use http_types::{headers, Body, Error, Method, Mime, Request, Response, Result, StatusCode};
use oxigraph::sparql::{PreparedQuery, QueryOptions, QueryResult, QueryResultSyntax};
use oxigraph::{
    FileSyntax, GraphSyntax, MemoryRepository, Repository, RepositoryConnection, RocksDbRepository,
};
use std::str::FromStr;
use std::time::Duration;
use url::form_urlencoded;

mod loader;

const MAX_SPARQL_BODY_SIZE: u64 = 1_048_576;
const SERVER: &str = concat!("Oxigraph/", env!("CARGO_PKG_VERSION"));

#[derive(FromArgs)]
/// Oxigraph SPARQL server for Wikibase
struct Args {
    /// specify a server socket to bind using the format $(HOST):$(PORT)
    #[argh(option, short = 'b', default = "\"localhost:7878\".to_string()")]
    bind: String,

    /// directory in which persist the data. By default data are kept in memory
    #[argh(option, short = 'f')]
    file: Option<String>,

    #[argh(option)]
    /// base URL of the MediaWiki API like https://www.wikidata.org/w/api.php
    mediawiki_api: String,

    #[argh(option)]
    /// base URL of MediaWiki like https://www.wikidata.org/wiki/
    mediawiki_base_url: String,

    #[argh(option)]
    /// namespaces ids to load like "0,120"
    namespaces: Option<String>,

    #[argh(option)]
    /// slot to load like "mediainfo". Could not be use with namespaces
    slot: Option<String>,
}

#[async_std::main]
pub async fn main() -> Result<()> {
    let args: Args = argh::from_env();

    let file = args.file.clone();
    if let Some(file) = file {
        main_with_dataset(Arc::new(RocksDbRepository::open(file)?), args).await
    } else {
        main_with_dataset(Arc::new(MemoryRepository::default()), args).await
    }
}

async fn main_with_dataset<R: Send + Sync + 'static>(repository: Arc<R>, args: Args) -> Result<()>
where
    for<'a> &'a R: Repository,
{
    let repo = repository.clone();
    let mediawiki_api = args.mediawiki_api.clone();
    let mediawiki_base_url = args.mediawiki_base_url.clone();
    let namespaces = args
        .namespaces
        .as_deref()
        .unwrap_or("")
        .split(',')
        .flat_map(|t| {
            let t = t.trim();
            if t.is_empty() {
                None
            } else {
                Some(u32::from_str(t).unwrap())
            }
        })
        .collect::<Vec<_>>();
    let slot = args.slot.clone();
    spawn_blocking(move || {
        let mut loader = WikibaseLoader::new(
            repo.as_ref(),
            &mediawiki_api,
            &mediawiki_base_url,
            &namespaces,
            slot.as_deref(),
            Duration::new(10, 0),
        )
        .unwrap();
        loader.initial_loading().unwrap();
        loader.update_loop();
    });

    println!("Listening for requests at http://{}", &args.bind);

    http_server(args.bind, move |request| {
        handle_request(request, Arc::clone(&repository))
    })
    .await
}

async fn handle_request<R: Send + Sync + 'static>(
    request: Request,
    repository: Arc<R>,
) -> Result<Response>
where
    for<'a> &'a R: Repository,
{
    let mut response = match (request.url().path(), request.method()) {
        ("/query", Method::Get) => {
            evaluate_urlencoded_sparql_query(
                repository,
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
                    evaluate_sparql_query(repository, buffer, request).await?
                } else if essence(&content_type) == "application/x-www-form-urlencoded" {
                    let mut buffer = Vec::new();
                    let mut request = request;
                    request
                        .take_body()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_end(&mut buffer)
                        .await?;
                    evaluate_urlencoded_sparql_query(repository, buffer, request).await?
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

async fn evaluate_urlencoded_sparql_query<R: Send + Sync + 'static>(
    repository: Arc<R>,
    encoded: Vec<u8>,
    request: Request,
) -> Result<Response>
where
    for<'a> &'a R: Repository,
{
    if let Some((_, query)) = form_urlencoded::parse(&encoded).find(|(k, _)| k == "query") {
        evaluate_sparql_query(repository, query.to_string(), request).await
    } else {
        Ok(simple_response(
            StatusCode::BadRequest,
            "You should set the 'query' parameter",
        ))
    }
}

async fn evaluate_sparql_query<R: Send + Sync + 'static>(
    repository: Arc<R>,
    query: String,
    request: Request,
) -> Result<Response>
where
    for<'a> &'a R: Repository,
{
    spawn_blocking(move || {
        //TODO: stream
        let query = repository
            .connection()?
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
