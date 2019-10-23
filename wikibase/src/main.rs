use crate::loader::WikibaseLoader;
use clap::App;
use clap::Arg;
use clap::ArgMatches;
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

pub fn main() {
    let matches = App::new("Oxigraph SPARQL server")
        .arg(
            Arg::with_name("bind")
                .long("bind")
                .short("b")
                .help("Specify a server socket to bind using the format $(HOST):$(PORT)")
                .default_value("localhost:7878")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("file")
                .long("file")
                .short("f")
                .help("Directory in which persist the data. By default data are kept in memory.")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("mediawiki_api")
                .long("mediawiki_api")
                .help("URL of the MediaWiki API like https://www.wikidata.org/w/api.php.")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("mediawiki_base_url")
                .long("mediawiki_base_url")
                .help("Base URL of MediaWiki like https://www.wikidata.org/wiki/")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("namespaces")
                .long("namespaces")
                .help("Namespaces ids, to load in Blazegraph like \"0,120\"")
                .required(true)
                .takes_value(true),
        )
        .get_matches();

    let file = matches.value_of("file").map(|v| v.to_string());
    if let Some(file) = file {
        main_with_dataset(Arc::new(RocksDbRepository::open(file).unwrap()), &matches)
    } else {
        main_with_dataset(Arc::new(MemoryRepository::default()), &matches)
    }
}

fn main_with_dataset<R: Send + Sync + 'static>(repository: Arc<R>, matches: &ArgMatches)
where
    for<'a> &'a R: Repository,
{
    let addr = matches.value_of("bind").unwrap().to_owned();
    println!("Listening for requests at http://{}", &addr);

    let repo = repository.clone();
    let mediawiki_api = matches.value_of("mediawiki_api").unwrap().to_owned();
    let mediawiki_base_url = matches.value_of("mediawiki_base_url").unwrap().to_owned();
    let namespaces = matches
        .value_of("namespaces")
        .unwrap()
        .split(',')
        .map(|t| u32::from_str(t.trim()).unwrap())
        .collect::<Vec<_>>();
    thread::spawn(move || {
        let mut loader = WikibaseLoader::new(
            repo.as_ref(),
            &mediawiki_api,
            &mediawiki_base_url,
            &namespaces,
            Duration::new(10, 0),
        )
        .unwrap();
        loader.initial_loading().unwrap();
        loader.update_loop();
    });

    start_server(addr.to_string(), move |request| {
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
