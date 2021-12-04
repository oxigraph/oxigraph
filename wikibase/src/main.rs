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
use async_std::future::Future;
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task::spawn;
use clap::{crate_version, App, Arg};
use http_types::content::ContentType;
use http_types::{
    bail_status, format_err_status, headers, Error, Method, Mime, Request, Response, Result,
    StatusCode,
};
use oxigraph::io::GraphFormat;
use oxigraph::model::{GraphName, NamedNode, NamedOrBlankNode};
use oxigraph::sparql::{Query, QueryResults, QueryResultsFormat};
use oxigraph::store::Store;
use std::str::FromStr;
use std::time::Duration;
use url::form_urlencoded;

mod loader;

const MAX_SPARQL_BODY_SIZE: u64 = 1_048_576;
const SERVER: &str = concat!("Oxigraph/", env!("CARGO_PKG_VERSION"));

#[async_std::main]
pub async fn main() -> Result<()> {
    let matches = App::new("Oxigraph SPARQL server for Wikibase")
        .version(crate_version!())
        .arg(
            Arg::with_name("bind")
                .short("b")
                .long("bind")
                .help("Sets a custom config file")
                .default_value("localhost:7878")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("file")
                .short("f")
                .long("file")
                .help("Directory in which persist the data")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("mediawiki_api")
                .long("mediawiki_api")
                .help("Base URL of the MediaWiki API like https://www.wikidata.org/w/api.php")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("mediawiki_base_url")
                .long("mediawiki_base_url")
                .help("Base URL of MediaWiki like https://www.wikidata.org/wiki/")
                .takes_value(true)
                .required(true),
        )
        .arg(
            Arg::with_name("namespaces")
                .long("namespaces")
                .help("Namespaces ids to load like '0,120'")
                .default_value("")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("slot")
                .long("slot")
                .help("Slot to load like 'mediainfo'. Could not be use with namespaces")
                .takes_value(true),
        )
        .get_matches();
    let bind = matches.value_of("bind").unwrap();
    let file = matches.value_of("file");
    let mediawiki_api = matches.value_of("mediawiki_api").unwrap();
    let mediawiki_base_url = matches.value_of("mediawiki_base_url").unwrap();
    let namespaces = matches
        .value_of("namespaces")
        .unwrap()
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
    let slot = matches.value_of("slot");

    let store = if let Some(file) = file {
        Store::open(file)
    } else {
        Store::new()
    }?;
    let repo = store.clone();
    let mut loader = WikibaseLoader::new(
        repo,
        mediawiki_api,
        mediawiki_base_url,
        &namespaces,
        slot,
        Duration::new(10, 0),
    )
    .unwrap();
    spawn(async move {
        loader.initial_loading().unwrap();
        loader.update_loop();
    });

    println!("Listening for requests at http://{}", &bind);

    http_server(bind, move |request| handle_request(request, store.clone())).await
}

async fn handle_request(request: Request, store: Store) -> Result<Response> {
    Ok(match (request.url().path(), request.method()) {
        ("/query", Method::Get) => {
            configure_and_evaluate_sparql_query(store, url_query(&request), None, request)?
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
                    configure_and_evaluate_sparql_query(
                        store,
                        url_query(&request),
                        Some(buffer),
                        request,
                    )?
                } else if content_type.essence() == "application/x-www-form-urlencoded" {
                    let mut buffer = Vec::new();
                    let mut request = request;
                    request
                        .take_body()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_end(&mut buffer)
                        .await?;
                    configure_and_evaluate_sparql_query(store, buffer, None, request)?
                } else {
                    bail_status!(415, "Not supported Content-Type given: {}", content_type);
                }
            } else {
                bail_status!(400, "No Content-Type given");
            }
        }
        _ => {
            bail_status!(
                404,
                "{} {} is not supported by this server",
                request.method(),
                request.url().path()
            );
        }
    })
}

fn url_query(request: &Request) -> Vec<u8> {
    request.url().query().unwrap_or("").as_bytes().to_vec()
}

fn configure_and_evaluate_sparql_query(
    store: Store,
    encoded: Vec<u8>,
    mut query: Option<String>,
    request: Request,
) -> Result<Response> {
    let mut default_graph_uris = Vec::new();
    let mut named_graph_uris = Vec::new();
    for (k, v) in form_urlencoded::parse(&encoded) {
        match k.as_ref() {
            "query" => {
                if query.is_some() {
                    bail_status!(400, "Multiple query parameters provided")
                }
                query = Some(v.into_owned())
            }
            "default-graph-uri" => default_graph_uris.push(v.into_owned()),
            "named-graph-uri" => named_graph_uris.push(v.into_owned()),
            _ => (),
        }
    }
    if let Some(query) = query {
        evaluate_sparql_query(store, query, default_graph_uris, named_graph_uris, request)
    } else {
        bail_status!(400, "You should set the 'query' parameter")
    }
}

fn evaluate_sparql_query(
    store: Store,
    query: String,
    default_graph_uris: Vec<String>,
    named_graph_uris: Vec<String>,
    request: Request,
) -> Result<Response> {
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
        ContentType::new(format.media_type()).apply(&mut response);
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
        ContentType::new(format.media_type()).apply(&mut response);
        Ok(response)
    }
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
            let mut response = match handle(request).await {
                Ok(result) => result,
                Err(error) => {
                    if error.status().is_server_error() {
                        eprintln!("{}", error);
                    }
                    let mut response = Response::new(error.status());
                    response.set_body(error.to_string());
                    response
                }
            };
            response.append_header(headers::SERVER, SERVER);
            Ok(response)
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

fn bad_request(e: impl Into<Error>) -> Error {
    let mut e = e.into();
    e.set_status(StatusCode::BadRequest);
    e
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
    let supported_mime: Vec<Mime> = supported
        .iter()
        .map(|h| Mime::from_str(h).unwrap())
        .collect();

    if header.is_empty() {
        return parse(supported.first().unwrap())
            .ok_or_else(|| Error::from_str(StatusCode::InternalServerError, "Unknown mime type"));
    }
    let mut result = None;
    let mut result_score = 0f32;

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
        for candidate in &supported_mime {
            if (possible.basetype() == candidate.basetype() || possible.basetype() == "*")
                && (possible.subtype() == candidate.subtype() || possible.subtype() == "*")
            {
                result = Some(candidate);
                result_score = score;
                break;
            }
        }
    }

    let result = result.ok_or_else(|| {
        format_err_status!(
            406,
            "The available Content-Types are {}",
            supported.join(", ")
        )
    })?;

    parse(result.essence())
        .ok_or_else(|| Error::from_str(StatusCode::InternalServerError, "Unknown mime type"))
}
