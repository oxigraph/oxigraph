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
use async_std::task::{spawn, spawn_blocking};
use http_types::{headers, Body, Error, Method, Mime, Request, Response, Result, StatusCode};
use oxigraph::io::GraphFormat;
use oxigraph::sparql::{Query, QueryOptions, QueryResults, QueryResultsFormat};
use oxigraph::RocksDbStore;
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

    /// directory in which persist the data
    #[argh(option, short = 'f')]
    file: String,

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

    let store = RocksDbStore::open(args.file)?;
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
    let repo = store.clone();
    spawn_blocking(move || {
        let mut loader = WikibaseLoader::new(
            repo,
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

    http_server(&args.bind, move |request| {
        handle_request(request, store.clone())
    })
    .await
}

async fn handle_request(request: Request, store: RocksDbStore) -> Result<Response> {
    let mut response = match (request.url().path(), request.method()) {
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
                    evaluate_sparql_query(store, buffer, request).await?
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
                        format!("No supported Content-Type given: {}", content_type),
                    )
                }
            } else {
                simple_response(StatusCode::BadRequest, "No Content-Type given")
            }
        }
        _ => Response::new(StatusCode::NotFound),
    };
    response.append_header("Server", SERVER);
    Ok(response)
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
        let mut query = Query::parse(&query, None).map_err(|e| {
            let mut e = Error::from(e);
            e.set_status(StatusCode::BadRequest);
            e
        })?;
        if query.dataset().is_default_dataset() {
            query.dataset_mut().set_default_graph_as_union();
        }
        let options = QueryOptions::default().with_simple_service_handler();
        let results = store.query(query, options)?;
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

    let listener = TcpListener::bind(&host).await?;
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
