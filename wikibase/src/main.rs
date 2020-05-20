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
use oxigraph::sparql::{PreparedQuery, QueryOptions, QueryResult, QueryResultSyntax};
use oxigraph::{
    FileSyntax, GraphSyntax, MemoryRepository, Repository, RepositoryConnection, RocksDbRepository,
};
use rouille::input::priority_header_preferred;
use rouille::url::form_urlencoded;
use rouille::{content_encoding, start_server, Request, Response};
use std::io::Read;
use std::str::FromStr;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

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

pub fn main() {
    let args: Args = argh::from_env();

    let file = args.file.clone();
    if let Some(file) = file {
        main_with_dataset(Arc::new(RocksDbRepository::open(file).unwrap()), args)
    } else {
        main_with_dataset(Arc::new(MemoryRepository::default()), args)
    }
}

fn main_with_dataset<R: Send + Sync + 'static>(repository: Arc<R>, args: Args)
where
    for<'a> &'a R: Repository,
{
    println!("Listening for requests at http://{}", &args.bind);

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
    thread::spawn(move || {
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

    start_server(args.bind, move |request| {
        content_encoding::apply(
            request,
            handle_request(request, repository.connection().unwrap()),
        )
        .with_unique_header("Server", SERVER)
    })
}

fn handle_request<R: RepositoryConnection>(request: &Request, connection: R) -> Response {
    match (request.url().as_str(), request.method()) {
        ("/query", "GET") => evaluate_urlencoded_sparql_query(
            connection,
            request.raw_query_string().as_bytes(),
            request,
        ),
        ("/query", "POST") => {
            if let Some(body) = request.data() {
                if let Some(content_type) = request.header("Content-Type") {
                    if content_type.starts_with("application/sparql-query") {
                        let mut buffer = String::default();
                        body.take(MAX_SPARQL_BODY_SIZE)
                            .read_to_string(&mut buffer)
                            .unwrap();
                        evaluate_sparql_query(connection, &buffer, request)
                    } else if content_type.starts_with("application/x-www-form-urlencoded") {
                        let mut buffer = Vec::default();
                        body.take(MAX_SPARQL_BODY_SIZE)
                            .read_to_end(&mut buffer)
                            .unwrap();
                        evaluate_urlencoded_sparql_query(connection, &buffer, request)
                    } else {
                        Response::text(format!(
                            "No supported content Content-Type given: {}",
                            content_type
                        ))
                        .with_status_code(415)
                    }
                } else {
                    Response::text("No Content-Type given").with_status_code(400)
                }
            } else {
                Response::text("No content given").with_status_code(400)
            }
        }
        _ => Response::empty_404(),
    }
}

fn evaluate_urlencoded_sparql_query<R: RepositoryConnection>(
    connection: R,
    encoded: &[u8],
    request: &Request,
) -> Response {
    if let Some((_, query)) = form_urlencoded::parse(encoded).find(|(k, _)| k == "query") {
        evaluate_sparql_query(connection, &query, request)
    } else {
        Response::text("You should set the 'query' parameter").with_status_code(400)
    }
}

fn evaluate_sparql_query<R: RepositoryConnection>(
    connection: R,
    query: &str,
    request: &Request,
) -> Response {
    //TODO: stream
    match connection.prepare_query(query, QueryOptions::default().with_default_graph_as_union()) {
        Ok(query) => {
            let results = query.exec().unwrap();
            if let QueryResult::Graph(_) = results {
                let supported_formats = [
                    GraphSyntax::NTriples.media_type(),
                    GraphSyntax::Turtle.media_type(),
                    GraphSyntax::RdfXml.media_type(),
                ];
                let format = if let Some(accept) = request.header("Accept") {
                    if let Some(media_type) =
                        priority_header_preferred(accept, supported_formats.iter().cloned())
                            .and_then(|p| GraphSyntax::from_mime_type(supported_formats[p]))
                    {
                        media_type
                    } else {
                        return Response::text(format!(
                            "No supported Accept given: {}. Supported format: {:?}",
                            accept, supported_formats
                        ))
                        .with_status_code(415);
                    }
                } else {
                    GraphSyntax::NTriples
                };

                Response::from_data(
                    format.media_type(),
                    results.write_graph(Vec::default(), format).unwrap(),
                )
            } else {
                let supported_formats = [
                    QueryResultSyntax::Xml.media_type(),
                    QueryResultSyntax::Json.media_type(),
                ];
                let format = if let Some(accept) = request.header("Accept") {
                    if let Some(media_type) =
                        priority_header_preferred(accept, supported_formats.iter().cloned())
                            .and_then(|p| QueryResultSyntax::from_mime_type(supported_formats[p]))
                    {
                        media_type
                    } else {
                        return Response::text(format!(
                            "No supported Accept given: {}. Supported format: {:?}",
                            accept, supported_formats
                        ))
                        .with_status_code(415);
                    }
                } else {
                    QueryResultSyntax::Json
                };

                Response::from_data(
                    format.media_type(),
                    results.write(Vec::default(), format).unwrap(),
                )
            }
        }
        Err(error) => Response::text(error.to_string()).with_status_code(400),
    }
}
