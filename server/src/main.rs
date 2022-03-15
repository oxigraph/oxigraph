use clap::{Parser, Subcommand};
use flate2::read::MultiGzDecoder;
use oxhttp::model::{Body, HeaderName, HeaderValue, Request, Response, Status};
use oxhttp::Server;
use oxigraph::io::{DatasetFormat, DatasetSerializer, GraphFormat, GraphSerializer};
use oxigraph::model::{GraphName, GraphNameRef, IriParseError, NamedNode, NamedOrBlankNode};
use oxigraph::sparql::{Query, QueryResults, Update};
use oxigraph::store::{BulkLoader, Store};
use oxiri::Iri;
use rand::random;
use sparesults::{QueryResultsFormat, QueryResultsSerializer};
use std::cell::RefCell;
use std::cmp::min;
use std::fmt;
use std::fs::File;
use std::io::{self, BufReader, ErrorKind, Read, Write};
use std::path::PathBuf;
use std::rc::Rc;
use std::str::FromStr;
use std::thread::{spawn, JoinHandle};
use std::time::{Duration, Instant};
use url::form_urlencoded;

const MAX_SPARQL_BODY_SIZE: u64 = 1_048_576;
const HTTP_TIMEOUT: Duration = Duration::from_secs(60);
const HTML_ROOT_PAGE: &str = include_str!("../templates/query.html");
const LOGO: &str = include_str!("../logo.svg");

#[derive(Parser)]
#[clap(about, version)]
/// Oxigraph SPARQL server.
struct Args {
    /// Directory in which persist the data.
    #[clap(short, long, parse(from_os_str), global = true)]
    location: Option<PathBuf>,
    #[clap(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start Oxigraph HTTP server.
    Serve {
        /// Host and port to listen to.
        #[clap(short, long, default_value = "localhost:7878", global = true)]
        bind: String,
    },
    /// Load file(s) into the store.
    Load {
        /// file(s) to load.
        ///
        /// If multiple files are provided they are loaded in parallel.
        #[clap(short, long, global = true)]
        file: Vec<String>,
        /// Attempt to keep loading even if the data file is invalid.
        ///
        /// Only works with N-Triples and N-Quads for now.
        #[clap(long, global = true)]
        lenient: bool,
    },
}

pub fn main() -> std::io::Result<()> {
    let matches = Args::parse();
    let store = if let Some(path) = &matches.location {
        Store::open(path)
    } else {
        Store::new()
    }?;

    match matches.command {
        Command::Load { file, lenient } => {
            let handles = file
                .iter()
                .map(|file| {
                    let store = store.clone();
                    let file = file.to_string();
                    spawn(move || {
                        let f = file.clone();
                        let start = Instant::now();
                        let mut loader = store.bulk_loader().on_progress(move |size| {
                            let elapsed = start.elapsed();
                            eprintln!(
                                "{} triples loaded in {}s ({} t/s) from {}",
                                size,
                                elapsed.as_secs(),
                                size / elapsed.as_secs(),
                                f
                            )
                        });
                        if lenient {
                            loader = loader.on_parse_error(|e| {
                                eprintln!("Parsing error: {}", e);
                                Ok(())
                            })
                        }
                        if file.ends_with(".gz") {
                            bulk_load(
                                loader,
                                &file[..file.len() - 3],
                                MultiGzDecoder::new(File::open(&file)?),
                            )
                        } else {
                            bulk_load(loader, &file, File::open(&file)?)
                        }
                    })
                })
                .collect::<Vec<JoinHandle<io::Result<()>>>>();
            for handle in handles {
                handle.join().unwrap()?;
            }
            Ok(())
        }
        Command::Serve { bind } => {
            let mut server = Server::new(move |request| handle_request(request, store.clone()));
            server.set_global_timeout(HTTP_TIMEOUT);
            server
                .set_server_name(concat!("Oxigraph/", env!("CARGO_PKG_VERSION")))
                .unwrap();
            eprintln!("Listening for requests at http://{}", &bind);
            server.listen(bind)?;
            Ok(())
        }
    }
}

fn bulk_load(loader: BulkLoader, file: &str, reader: impl Read) -> io::Result<()> {
    let (_, extension) = file.rsplit_once('.').ok_or_else(|| io::Error::new(
        ErrorKind::InvalidInput,
        format!("The server is not able to guess the file format of {} because the file name as no extension", file)))?;
    let reader = BufReader::new(reader);
    if let Some(format) = DatasetFormat::from_extension(extension) {
        loader.load_dataset(reader, format, None)?;
        Ok(())
    } else if let Some(format) = GraphFormat::from_extension(extension) {
        loader.load_graph(reader, format, GraphNameRef::DefaultGraph, None)?;
        Ok(())
    } else {
        Err(io::Error::new(
            ErrorKind::InvalidInput,
            format!(
                "The server is not able to guess the file format from the extension {}",
                extension
            ),
        ))
    }
}

fn handle_request(request: &mut Request, store: Store) -> Response {
    match (request.url().path(), request.method().as_ref()) {
        ("/", "HEAD") => Response::builder(Status::OK)
            .with_header(HeaderName::CONTENT_TYPE, "text_html")
            .unwrap()
            .build(),
        ("/", "GET") => Response::builder(Status::OK)
            .with_header(HeaderName::CONTENT_TYPE, "text_html")
            .unwrap()
            .with_body(HTML_ROOT_PAGE),
        ("/logo.svg", "HEAD") => Response::builder(Status::OK)
            .with_header(HeaderName::CONTENT_TYPE, "image/svg+xml")
            .unwrap()
            .build(),
        ("/logo.svg", "GET") => Response::builder(Status::OK)
            .with_header(HeaderName::CONTENT_TYPE, "image/svg+xml")
            .unwrap()
            .with_body(LOGO),
        ("/query", "GET") => {
            configure_and_evaluate_sparql_query(store, &[url_query(request)], None, request)
        }
        ("/query", "POST") => {
            if let Some(content_type) = content_type(request) {
                if content_type == "application/sparql-query" {
                    let mut buffer = String::new();
                    if let Err(e) = request
                        .body_mut()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_string(&mut buffer)
                    {
                        return bad_request(e);
                    }
                    configure_and_evaluate_sparql_query(
                        store,
                        &[url_query(request)],
                        Some(buffer),
                        request,
                    )
                } else if content_type == "application/x-www-form-urlencoded" {
                    let mut buffer = Vec::new();
                    if let Err(e) = request
                        .body_mut()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_end(&mut buffer)
                    {
                        return bad_request(e);
                    }
                    configure_and_evaluate_sparql_query(
                        store,
                        &[url_query(request), &buffer],
                        None,
                        request,
                    )
                } else {
                    unsupported_media_type(&content_type)
                }
            } else {
                bad_request("No Content-Type given")
            }
        }
        ("/update", "POST") => {
            if let Some(content_type) = content_type(request) {
                if content_type == "application/sparql-update" {
                    let mut buffer = String::new();
                    if let Err(e) = request
                        .body_mut()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_string(&mut buffer)
                    {
                        return bad_request(e);
                    }
                    configure_and_evaluate_sparql_update(
                        store,
                        &[url_query(request)],
                        Some(buffer),
                        request,
                    )
                } else if content_type == "application/x-www-form-urlencoded" {
                    let mut buffer = Vec::new();
                    if let Err(e) = request
                        .body_mut()
                        .take(MAX_SPARQL_BODY_SIZE)
                        .read_to_end(&mut buffer)
                    {
                        return bad_request(e);
                    }
                    configure_and_evaluate_sparql_update(
                        store,
                        &[url_query(request), &buffer],
                        None,
                        request,
                    )
                } else {
                    unsupported_media_type(&content_type)
                }
            } else {
                bad_request("No Content-Type given")
            }
        }
        (path, "GET") if path.starts_with("/store") => {
            if let Some(target) = match store_target(request) {
                Ok(target) => target,
                Err(error) => return error,
            } {
                if !match &target {
                    NamedGraphName::DefaultGraph => true,
                    NamedGraphName::NamedNode(target) => match store.contains_named_graph(target) {
                        Ok(r) => r,
                        Err(e) => return internal_server_error(e),
                    },
                } {
                    return error(
                        Status::NOT_FOUND,
                        format!("The graph {} does not exists", GraphName::from(target)),
                    );
                }
                let format = match graph_content_negotiation(request) {
                    Ok(format) => format,
                    Err(response) => return response,
                };
                let triples = store.quads_for_pattern(
                    None,
                    None,
                    None,
                    Some(GraphName::from(target).as_ref()),
                );
                ReadForWrite::build_response(
                    move |w| {
                        Ok((
                            GraphSerializer::from_format(format).triple_writer(w)?,
                            triples,
                        ))
                    },
                    |(mut writer, mut triples)| {
                        Ok(if let Some(t) = triples.next() {
                            writer.write(&t?.into())?;
                            Some((writer, triples))
                        } else {
                            writer.finish()?;
                            None
                        })
                    },
                    format.media_type(),
                )
            } else {
                let format = match dataset_content_negotiation(request) {
                    Ok(format) => format,
                    Err(response) => return response,
                };
                ReadForWrite::build_response(
                    move |w| {
                        Ok((
                            DatasetSerializer::from_format(format).quad_writer(w)?,
                            store.iter(),
                        ))
                    },
                    |(mut writer, mut quads)| {
                        Ok(if let Some(q) = quads.next() {
                            writer.write(&q?)?;
                            Some((writer, quads))
                        } else {
                            writer.finish()?;
                            None
                        })
                    },
                    format.media_type(),
                )
            }
        }
        (path, "PUT") if path.starts_with("/store") => {
            if let Some(content_type) = content_type(request) {
                if let Some(target) = match store_target(request) {
                    Ok(target) => target,
                    Err(error) => return error,
                } {
                    if let Some(format) = GraphFormat::from_media_type(&content_type) {
                        let new = !match &target {
                            NamedGraphName::NamedNode(target) => {
                                if match store.contains_named_graph(target) {
                                    Ok(r) => r,
                                    Err(e) => return internal_server_error(e),
                                } {
                                    if let Err(e) = store.clear_graph(target) {
                                        return internal_server_error(e);
                                    }
                                    true
                                } else {
                                    if let Err(e) = store.insert_named_graph(target) {
                                        return internal_server_error(e);
                                    }
                                    false
                                }
                            }
                            NamedGraphName::DefaultGraph => {
                                if let Err(e) = store.clear_graph(GraphNameRef::DefaultGraph) {
                                    return internal_server_error(e);
                                }
                                true
                            }
                        };
                        if let Err(e) = store.load_graph(
                            BufReader::new(request.body_mut()),
                            format,
                            GraphName::from(target).as_ref(),
                            None,
                        ) {
                            return bad_request(e);
                        }
                        Response::builder(if new {
                            Status::CREATED
                        } else {
                            Status::NO_CONTENT
                        })
                        .build()
                    } else {
                        unsupported_media_type(&content_type)
                    }
                } else if let Some(format) = DatasetFormat::from_media_type(&content_type) {
                    if let Err(e) = store.clear() {
                        return internal_server_error(e);
                    }
                    if let Err(e) =
                        store.load_dataset(BufReader::new(request.body_mut()), format, None)
                    {
                        return internal_server_error(e);
                    }
                    Response::builder(Status::NO_CONTENT).build()
                } else {
                    unsupported_media_type(&content_type)
                }
            } else {
                bad_request("No Content-Type given")
            }
        }
        (path, "DELETE") if path.starts_with("/store") => {
            if let Some(target) = match store_target(request) {
                Ok(target) => target,
                Err(error) => return error,
            } {
                match target {
                    NamedGraphName::DefaultGraph => {
                        if let Err(e) = store.clear_graph(GraphNameRef::DefaultGraph) {
                            return internal_server_error(e);
                        }
                    }
                    NamedGraphName::NamedNode(target) => {
                        if match store.contains_named_graph(&target) {
                            Ok(r) => r,
                            Err(e) => return internal_server_error(e),
                        } {
                            if let Err(e) = store.remove_named_graph(&target) {
                                return internal_server_error(e);
                            }
                        } else {
                            return error(
                                Status::NOT_FOUND,
                                format!("The graph {} does not exists", target),
                            );
                        }
                    }
                }
            } else if let Err(e) = store.clear() {
                return internal_server_error(e);
            }
            Response::builder(Status::NO_CONTENT).build()
        }
        (path, "POST") if path.starts_with("/store") => {
            if let Some(content_type) = content_type(request) {
                if let Some(target) = match store_target(request) {
                    Ok(target) => target,
                    Err(error) => return error,
                } {
                    if let Some(format) = GraphFormat::from_media_type(&content_type) {
                        let new = !match &target {
                            NamedGraphName::NamedNode(target) => {
                                match store.contains_named_graph(target) {
                                    Ok(r) => r,
                                    Err(e) => return internal_server_error(e),
                                }
                            }
                            NamedGraphName::DefaultGraph => true,
                        };
                        if let Err(e) = store.load_graph(
                            BufReader::new(request.body_mut()),
                            format,
                            GraphName::from(target).as_ref(),
                            None,
                        ) {
                            return bad_request(e);
                        }
                        Response::builder(if new {
                            Status::CREATED
                        } else {
                            Status::NO_CONTENT
                        })
                        .build()
                    } else {
                        unsupported_media_type(&content_type)
                    }
                } else if let Some(format) = DatasetFormat::from_media_type(&content_type) {
                    if let Err(e) =
                        store.load_dataset(BufReader::new(request.body_mut()), format, None)
                    {
                        return bad_request(e);
                    }
                    Response::builder(Status::NO_CONTENT).build()
                } else if let Some(format) = GraphFormat::from_media_type(&content_type) {
                    let graph =
                        match resolve_with_base(request, &format!("/store/{:x}", random::<u128>()))
                        {
                            Ok(graph) => graph,
                            Err(e) => return e,
                        };
                    if let Err(e) =
                        store.load_graph(BufReader::new(request.body_mut()), format, &graph, None)
                    {
                        return bad_request(e);
                    }
                    Response::builder(Status::CREATED)
                        .with_header(HeaderName::LOCATION, graph.into_string())
                        .unwrap()
                        .build()
                } else {
                    unsupported_media_type(&content_type)
                }
            } else {
                bad_request("No Content-Type given")
            }
        }
        (path, "HEAD") if path.starts_with("/store") => {
            if let Some(target) = match store_target(request) {
                Ok(target) => target,
                Err(error) => return error,
            } {
                if !match &target {
                    NamedGraphName::DefaultGraph => true,
                    NamedGraphName::NamedNode(target) => match store.contains_named_graph(target) {
                        Ok(r) => r,
                        Err(e) => return internal_server_error(e),
                    },
                } {
                    return error(
                        Status::NOT_FOUND,
                        format!("The graph {} does not exists", GraphName::from(target)),
                    );
                }
                Response::builder(Status::OK).build()
            } else {
                Response::builder(Status::OK).build()
            }
        }
        _ => error(
            Status::NOT_FOUND,
            format!(
                "{} {} is not supported by this server",
                request.method(),
                request.url().path()
            ),
        ),
    }
}

fn base_url(request: &Request) -> Result<String, Response> {
    let mut url = request.url().clone();
    if let Some(host) = request.url().host_str() {
        url.set_host(Some(host)).map_err(bad_request)?;
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.into())
}

fn resolve_with_base(request: &Request, url: &str) -> Result<NamedNode, Response> {
    Ok(NamedNode::new_unchecked(
        Iri::parse(base_url(request)?)
            .map_err(bad_request)?
            .resolve(url)
            .map_err(bad_request)?
            .into_inner(),
    ))
}

fn url_query(request: &Request) -> &[u8] {
    request.url().query().unwrap_or("").as_bytes()
}

fn configure_and_evaluate_sparql_query(
    store: Store,
    encoded: &[&[u8]],
    mut query: Option<String>,
    request: &Request,
) -> Response {
    let mut default_graph_uris = Vec::new();
    let mut named_graph_uris = Vec::new();
    let mut use_default_graph_as_union = false;
    for encoded in encoded {
        for (k, v) in form_urlencoded::parse(encoded) {
            match k.as_ref() {
                "query" => {
                    if query.is_some() {
                        return bad_request("Multiple query parameters provided");
                    }
                    query = Some(v.into_owned())
                }
                "default-graph-uri" => default_graph_uris.push(v.into_owned()),
                "union-default-graph" => use_default_graph_as_union = true,
                "named-graph-uri" => named_graph_uris.push(v.into_owned()),
                _ => (),
            }
        }
    }
    if let Some(query) = query {
        evaluate_sparql_query(
            store,
            query,
            use_default_graph_as_union,
            default_graph_uris,
            named_graph_uris,
            request,
        )
    } else {
        bad_request("You should set the 'query' parameter")
    }
}

fn evaluate_sparql_query(
    store: Store,
    query: String,
    use_default_graph_as_union: bool,
    default_graph_uris: Vec<String>,
    named_graph_uris: Vec<String>,
    request: &Request,
) -> Response {
    let mut query = match Query::parse(
        &query,
        Some(&match base_url(request) {
            Ok(url) => url,
            Err(r) => return r,
        }),
    ) {
        Ok(query) => query,
        Err(e) => return bad_request(e),
    };

    if use_default_graph_as_union {
        if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
            return bad_request(
                "default-graph-uri or named-graph-uri and union-default-graph should not be set at the same time"
            );
        }
        query.dataset_mut().set_default_graph_as_union()
    } else if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
        query.dataset_mut().set_default_graph(
            match default_graph_uris
                .into_iter()
                .map(|e| Ok(NamedNode::new(e)?.into()))
                .collect::<Result<Vec<GraphName>, IriParseError>>()
            {
                Ok(default_graph_uris) => default_graph_uris,
                Err(e) => return bad_request(e),
            },
        );
        query.dataset_mut().set_available_named_graphs(
            match named_graph_uris
                .into_iter()
                .map(|e| Ok(NamedNode::new(e)?.into()))
                .collect::<Result<Vec<NamedOrBlankNode>, IriParseError>>()
            {
                Ok(named_graph_uris) => named_graph_uris,
                Err(e) => return bad_request(e),
            },
        );
    }

    let results = match store.query(query) {
        Ok(results) => results,
        Err(e) => return internal_server_error(e),
    };
    match results {
        QueryResults::Solutions(solutions) => {
            let format = match query_results_content_negotiation(request) {
                Ok(format) => format,
                Err(response) => return response,
            };
            ReadForWrite::build_response(
                move |w| {
                    Ok((
                        QueryResultsSerializer::from_format(format)
                            .solutions_writer(w, solutions.variables().to_vec())?,
                        solutions,
                    ))
                },
                |(mut writer, mut solutions)| {
                    Ok(if let Some(solution) = solutions.next() {
                        writer.write(&solution?)?;
                        Some((writer, solutions))
                    } else {
                        writer.finish()?;
                        None
                    })
                },
                format.media_type(),
            )
        }
        QueryResults::Boolean(result) => {
            let format = match query_results_content_negotiation(request) {
                Ok(format) => format,
                Err(response) => return response,
            };
            let mut body = Vec::new();
            if let Err(e) =
                QueryResultsSerializer::from_format(format).write_boolean_result(&mut body, result)
            {
                return internal_server_error(e);
            }
            Response::builder(Status::OK)
                .with_header(HeaderName::CONTENT_TYPE, format.media_type())
                .unwrap()
                .with_body(body)
        }
        QueryResults::Graph(triples) => {
            let format = match graph_content_negotiation(request) {
                Ok(format) => format,
                Err(response) => return response,
            };
            ReadForWrite::build_response(
                move |w| {
                    Ok((
                        GraphSerializer::from_format(format).triple_writer(w)?,
                        triples,
                    ))
                },
                |(mut writer, mut triples)| {
                    Ok(if let Some(t) = triples.next() {
                        writer.write(&t?)?;
                        Some((writer, triples))
                    } else {
                        writer.finish()?;
                        None
                    })
                },
                format.media_type(),
            )
        }
    }
}

fn configure_and_evaluate_sparql_update(
    store: Store,
    encoded: &[&[u8]],
    mut update: Option<String>,
    request: &Request,
) -> Response {
    let mut use_default_graph_as_union = false;
    let mut default_graph_uris = Vec::new();
    let mut named_graph_uris = Vec::new();
    for encoded in encoded {
        for (k, v) in form_urlencoded::parse(encoded) {
            match k.as_ref() {
                "update" => {
                    if update.is_some() {
                        return bad_request("Multiple update parameters provided");
                    }
                    update = Some(v.into_owned())
                }
                "using-graph-uri" => default_graph_uris.push(v.into_owned()),
                "using-union-graph" => use_default_graph_as_union = true,
                "using-named-graph-uri" => named_graph_uris.push(v.into_owned()),
                _ => (),
            }
        }
    }
    if let Some(update) = update {
        evaluate_sparql_update(
            store,
            update,
            use_default_graph_as_union,
            default_graph_uris,
            named_graph_uris,
            request,
        )
    } else {
        bad_request("You should set the 'update' parameter")
    }
}

fn evaluate_sparql_update(
    store: Store,
    update: String,
    use_default_graph_as_union: bool,
    default_graph_uris: Vec<String>,
    named_graph_uris: Vec<String>,
    request: &Request,
) -> Response {
    let mut update = match Update::parse(
        &update,
        Some(
            match base_url(request) {
                Ok(url) => url,
                Err(e) => return e,
            }
            .as_str(),
        ),
    ) {
        Ok(update) => update,
        Err(e) => return bad_request(e),
    };

    if use_default_graph_as_union {
        if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
            return bad_request(
                "using-graph-uri or using-named-graph-uri and using-union-graph should not be set at the same time"
            );
        }
        for using in update.using_datasets_mut() {
            if !using.is_default_dataset() {
                return bad_request(
                    "using-union-graph must not be used with a SPARQL UPDATE containing USING",
                );
            }
            using.set_default_graph_as_union();
        }
    } else if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
        let default_graph_uris = match default_graph_uris
            .into_iter()
            .map(|e| Ok(NamedNode::new(e)?.into()))
            .collect::<Result<Vec<GraphName>, IriParseError>>()
        {
            Ok(default_graph_uris) => default_graph_uris,
            Err(e) => return bad_request(e),
        };
        let named_graph_uris = match named_graph_uris
            .into_iter()
            .map(|e| Ok(NamedNode::new(e)?.into()))
            .collect::<Result<Vec<NamedOrBlankNode>, IriParseError>>()
        {
            Ok(named_graph_uris) => named_graph_uris,
            Err(e) => return bad_request(e),
        };
        for using in update.using_datasets_mut() {
            if !using.is_default_dataset() {
                return bad_request(
                        "using-graph-uri and using-named-graph-uri must not be used with a SPARQL UPDATE containing USING",
                    );
            }
            using.set_default_graph(default_graph_uris.clone());
            using.set_available_named_graphs(named_graph_uris.clone());
        }
    }
    if let Err(e) = store.update(update) {
        return internal_server_error(e);
    }
    Response::builder(Status::NO_CONTENT).build()
}

fn store_target(request: &Request) -> Result<Option<NamedGraphName>, Response> {
    if request.url().path() == "/store" {
        let mut graph = None;
        let mut default = false;
        for (k, v) in form_urlencoded::parse(request.url().query().unwrap_or("").as_bytes()) {
            match k.as_ref() {
                "graph" => graph = Some(v.into_owned()),
                "default" => default = true,
                _ => {
                    return Err(bad_request(format!("Unexpected parameter: {}", k)));
                }
            }
        }
        if let Some(graph) = graph {
            if default {
                Err(bad_request(
                    "Both graph and default parameters should not be set at the same time",
                ))
            } else {
                Ok(Some(NamedGraphName::NamedNode(resolve_with_base(
                    request, &graph,
                )?)))
            }
        } else if default {
            Ok(Some(NamedGraphName::DefaultGraph))
        } else {
            Ok(None)
        }
    } else {
        Ok(Some(NamedGraphName::NamedNode(resolve_with_base(
            request, "",
        )?)))
    }
}

#[derive(Eq, PartialEq, Debug, Clone, Hash)]
enum NamedGraphName {
    NamedNode(NamedNode),
    DefaultGraph,
}

impl From<NamedGraphName> for GraphName {
    fn from(graph_name: NamedGraphName) -> Self {
        match graph_name {
            NamedGraphName::NamedNode(node) => node.into(),
            NamedGraphName::DefaultGraph => Self::DefaultGraph,
        }
    }
}

fn graph_content_negotiation(request: &Request) -> Result<GraphFormat, Response> {
    content_negotiation(
        request,
        &[
            GraphFormat::NTriples.media_type(),
            GraphFormat::Turtle.media_type(),
            GraphFormat::RdfXml.media_type(),
        ],
        GraphFormat::from_media_type,
    )
}

fn dataset_content_negotiation(request: &Request) -> Result<DatasetFormat, Response> {
    content_negotiation(
        request,
        &[
            DatasetFormat::NQuads.media_type(),
            DatasetFormat::TriG.media_type(),
        ],
        DatasetFormat::from_media_type,
    )
}

fn query_results_content_negotiation(request: &Request) -> Result<QueryResultsFormat, Response> {
    content_negotiation(
        request,
        &[
            QueryResultsFormat::Json.media_type(),
            QueryResultsFormat::Xml.media_type(),
            QueryResultsFormat::Csv.media_type(),
            QueryResultsFormat::Tsv.media_type(),
        ],
        QueryResultsFormat::from_media_type,
    )
}

fn content_negotiation<F>(
    request: &Request,
    supported: &[&str],
    parse: impl Fn(&str) -> Option<F>,
) -> Result<F, Response> {
    let default = HeaderValue::default();
    let header = request
        .header(&HeaderName::ACCEPT)
        .unwrap_or(&default)
        .to_str()
        .map_err(|_| bad_request("The Accept header should be a valid ASCII string"))?;

    if header.is_empty() {
        return parse(supported.first().unwrap())
            .ok_or_else(|| internal_server_error("Unknown media type"));
    }
    let mut result = None;
    let mut result_score = 0f32;

    for possible in header.split(',') {
        let (possible, parameters) = possible.split_once(';').unwrap_or((possible, ""));
        let (possible_base, possible_sub) = possible
            .split_once('/')
            .ok_or_else(|| bad_request(format!("Invalid media type: '{}'", possible)))?;
        let possible_base = possible_base.trim();
        let possible_sub = possible_sub.trim();

        let mut score = 1.;
        for parameter in parameters.split(';') {
            let parameter = parameter.trim();
            if let Some(s) = parameter.strip_prefix("q=") {
                score = f32::from_str(s.trim())
                    .map_err(|_| bad_request(format!("Invalid Accept media type score: {}", s)))?
            }
        }
        if score <= result_score {
            continue;
        }
        for candidate in supported {
            let (candidate_base, candidate_sub) = candidate
                .split_once(';')
                .map_or(*candidate, |(p, _)| p)
                .split_once('/')
                .ok_or_else(|| {
                    internal_server_error(format!("Invalid media type: '{}'", possible))
                })?;
            if (possible_base == candidate_base || possible_base == "*")
                && (possible_sub == candidate_sub || possible_sub == "*")
            {
                result = Some(candidate);
                result_score = score;
                break;
            }
        }
    }

    let result = result.ok_or_else(|| {
        error(
            Status::NOT_ACCEPTABLE,
            format!("The available Content-Types are {}", supported.join(", "),),
        )
    })?;

    parse(result).ok_or_else(|| error(Status::INTERNAL_SERVER_ERROR, "Unknown media type"))
}

fn content_type(request: &Request) -> Option<String> {
    let value = request.header(&HeaderName::CONTENT_TYPE)?.to_str().ok()?;
    Some(
        value
            .split_once(';')
            .map_or(value, |(b, _)| b)
            .trim()
            .to_ascii_lowercase(),
    )
}

fn error(status: Status, message: impl fmt::Display) -> Response {
    Response::builder(status)
        .with_header(HeaderName::CONTENT_TYPE, "text/plain; charset=utf-8")
        .unwrap()
        .with_body(message.to_string())
}

fn bad_request(message: impl fmt::Display) -> Response {
    error(Status::BAD_REQUEST, message)
}

fn unsupported_media_type(content_type: &str) -> Response {
    error(
        Status::UNSUPPORTED_MEDIA_TYPE,
        format!("No supported content Content-Type given: {}", content_type),
    )
}

fn internal_server_error(message: impl fmt::Display) -> Response {
    eprintln!("Internal server error: {}", message);
    error(Status::INTERNAL_SERVER_ERROR, message)
}

/// Hacky tool to allow implementing read on top of a write loop
struct ReadForWrite<O, U: (Fn(O) -> std::io::Result<Option<O>>)> {
    buffer: Rc<RefCell<Vec<u8>>>,
    position: usize,
    add_more_data: U,
    state: Option<O>,
}

impl<O: 'static, U: (Fn(O) -> std::io::Result<Option<O>>) + 'static> ReadForWrite<O, U> {
    fn build_response(
        initial_state_builder: impl FnOnce(ReadForWriteWriter) -> std::io::Result<O>,
        add_more_data: U,
        content_type: &'static str,
    ) -> Response {
        let buffer = Rc::new(RefCell::new(Vec::new()));
        match initial_state_builder(ReadForWriteWriter {
            buffer: buffer.clone(),
        }) {
            Ok(state) => Response::builder(Status::OK)
                .with_header(HeaderName::CONTENT_TYPE, content_type)
                .unwrap()
                .with_body(Body::from_read(Self {
                    buffer,
                    position: 0,
                    add_more_data,
                    state: Some(state),
                })),
            Err(e) => internal_server_error(e),
        }
    }
}

impl<O, U: (Fn(O) -> std::io::Result<Option<O>>)> Read for ReadForWrite<O, U> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        while self.position == self.buffer.borrow().len() {
            // We read more data
            if let Some(state) = self.state.take() {
                self.buffer.borrow_mut().clear();
                self.position = 0;
                self.state = match (self.add_more_data)(state) {
                    Ok(state) => state,
                    Err(e) => {
                        eprintln!("Internal server error while steaming: {}", e);
                        self.buffer
                            .borrow_mut()
                            .write_all(e.to_string().as_bytes())?;
                        None
                    }
                }
            } else {
                return Ok(0); // End
            }
        }
        let buffer = self.buffer.borrow();
        let len = min(buffer.len() - self.position, buf.len());
        buf[..len].copy_from_slice(&buffer[self.position..self.position + len]);
        self.position += len;
        Ok(len)
    }
}

struct ReadForWriteWriter {
    buffer: Rc<RefCell<Vec<u8>>>,
}

impl Write for ReadForWriteWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }

    fn write_all(&mut self, buf: &[u8]) -> std::io::Result<()> {
        self.buffer.borrow_mut().write_all(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxhttp::model::Method;

    #[test]
    fn get_ui() {
        ServerTest::new().test_status(
            Request::builder(Method::GET, "http://localhost/".parse().unwrap()).build(),
            Status::OK,
        )
    }

    #[test]
    fn post_dataset_file() {
        let request = Request::builder(Method::POST, "http://localhost/store".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "application/trig")
            .unwrap()
            .with_body("<http://example.com> <http://example.com> <http://example.com> .");
        ServerTest::new().test_status(request, Status::NO_CONTENT)
    }

    #[test]
    fn post_wrong_file() {
        let request = Request::builder(Method::POST, "http://localhost/store".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "application/trig")
            .unwrap()
            .with_body("<http://example.com>");
        ServerTest::new().test_status(request, Status::BAD_REQUEST)
    }

    #[test]
    fn post_unsupported_file() {
        let request = Request::builder(Method::POST, "http://localhost/store".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "text/foo")
            .unwrap()
            .build();
        ServerTest::new().test_status(request, Status::UNSUPPORTED_MEDIA_TYPE)
    }

    #[test]
    fn get_query() {
        let server = ServerTest::new();

        let request = Request::builder(Method::POST, "http://localhost/store".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "application/trig")
            .unwrap()
            .with_body("<http://example.com> <http://example.com> <http://example.com> .");
        server.test_status(request, Status::NO_CONTENT);

        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()
                .unwrap(),
        )
        .with_header(HeaderName::ACCEPT, "text/csv")
        .unwrap()
        .build();
        server.test_body(
            request,
            "s,p,o\r\nhttp://example.com,http://example.com,http://example.com",
        );
    }

    #[test]
    fn get_query_accept_star() {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()
                .unwrap(),
        )
        .with_header(HeaderName::ACCEPT, "*/*")
        .unwrap()
        .build();
        ServerTest::new().test_body(
            request,
            "{\"head\":{\"vars\":[\"s\",\"p\",\"o\"]},\"results\":{\"bindings\":[]}}",
        );
    }

    #[test]
    fn get_query_accept_good() {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()
                .unwrap(),
        )
        .with_header(
            HeaderName::ACCEPT,
            "application/sparql-results+json;charset=utf-8",
        )
        .unwrap()
        .build();
        ServerTest::new().test_body(
            request,
            "{\"head\":{\"vars\":[\"s\",\"p\",\"o\"]},\"results\":{\"bindings\":[]}}",
        );
    }

    #[test]
    fn get_query_accept_bad() {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20*%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()
                .unwrap(),
        )
        .with_header(HeaderName::ACCEPT, "application/foo")
        .unwrap()
        .build();
        ServerTest::new().test_status(request, Status::NOT_ACCEPTABLE);
    }

    #[test]
    fn get_bad_query() {
        ServerTest::new().test_status(
            Request::builder(
                Method::GET,
                "http://localhost/query?query=SELECT".parse().unwrap(),
            )
            .build(),
            Status::BAD_REQUEST,
        );
    }

    #[test]
    fn get_query_union_graph() {
        let server = ServerTest::new();

        let request = Request::builder(Method::PUT, "http://localhost/store/1".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle")
            .unwrap()
            .with_body("<http://example.com> <http://example.com> <http://example.com> .");
        server.test_status(request, Status::CREATED);

        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}&union-default-graph"
                .parse()
                .unwrap(),
        ).with_header(HeaderName::ACCEPT, "text/csv")
            .unwrap()
            .build();
        server.test_body(
            request,
            "s,p,o\r\nhttp://example.com,http://example.com,http://example.com",
        );
    }

    #[test]
    fn get_query_union_graph_in_url_and_urlencoded() {
        let server = ServerTest::new();

        let request = Request::builder(Method::PUT, "http://localhost/store/1".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle")
            .unwrap()
            .with_body("<http://example.com> <http://example.com> <http://example.com> .");
        server.test_status(request, Status::CREATED);

        let request = Request::builder(
            Method::POST,
            "http://localhost/query?union-default-graph"
                .parse()
                .unwrap(),
        )
        .with_header(
            HeaderName::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .unwrap()
        .with_header(HeaderName::ACCEPT, "text/csv")
        .unwrap()
        .with_body("query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}");
        server.test_body(
            request,
            "s,p,o\r\nhttp://example.com,http://example.com,http://example.com",
        );
    }

    #[test]
    fn get_query_union_graph_and_default_graph() {
        ServerTest::new().test_status(Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20*%20WHERE%20{%20?s%20?p%20?o%20}&union-default-graph&default-graph-uri=http://example.com".parse()
                .unwrap(),
        ).build(), Status::BAD_REQUEST);
    }

    #[test]
    fn get_without_query() {
        ServerTest::new().test_status(
            Request::builder(Method::GET, "http://localhost/query".parse().unwrap()).build(),
            Status::BAD_REQUEST,
        );
    }

    #[test]
    fn post_query() {
        let request = Request::builder(Method::POST, "http://localhost/query".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-query")
            .unwrap()
            .with_body("SELECT * WHERE { ?s ?p ?o }");
        ServerTest::new().test_status(request, Status::OK)
    }

    #[test]
    fn post_bad_query() {
        let request = Request::builder(Method::POST, "http://localhost/query".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-query")
            .unwrap()
            .with_body("SELECT");
        ServerTest::new().test_status(request, Status::BAD_REQUEST)
    }

    #[test]
    fn post_unknown_query() {
        let request = Request::builder(Method::POST, "http://localhost/query".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-todo")
            .unwrap()
            .with_body("SELECT");
        ServerTest::new().test_status(request, Status::UNSUPPORTED_MEDIA_TYPE)
    }

    #[test]
    fn post_federated_query() {
        let request = Request::builder(Method::POST, "http://localhost/query".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-query")
            .unwrap().with_body("SELECT * WHERE { SERVICE <https://query.wikidata.org/sparql> { <https://en.wikipedia.org/wiki/Paris> ?p ?o } }");
        ServerTest::new().test_status(request, Status::OK)
    }

    #[test]
    fn post_update() {
        let request = Request::builder(Method::POST, "http://localhost/update".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-update")
            .unwrap()
            .with_body(
                "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
            );
        ServerTest::new().test_status(request, Status::NO_CONTENT)
    }

    #[test]
    fn post_bad_update() {
        let request = Request::builder(Method::POST, "http://localhost/update".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-update")
            .unwrap()
            .with_body("INSERT");
        ServerTest::new().test_status(request, Status::BAD_REQUEST)
    }

    #[test]
    fn graph_store_url_normalization() {
        let server = ServerTest::new();

        // PUT
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store?graph=http://example.com"
                .parse()
                .unwrap(),
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle")
        .unwrap()
        .with_body("<http://example.com> <http://example.com> <http://example.com> .");
        server.test_status(request, Status::CREATED);

        // GET good URI
        server.test_status(
            Request::builder(
                Method::GET,
                "http://localhost/store?graph=http://example.com"
                    .parse()
                    .unwrap(),
            )
            .build(),
            Status::OK,
        );

        // GET bad URI
        server.test_status(
            Request::builder(
                Method::GET,
                "http://localhost/store?graph=http://example.com/"
                    .parse()
                    .unwrap(),
            )
            .build(),
            Status::NOT_FOUND,
        );
    }

    #[test]
    fn graph_store_protocol() {
        // Tests from https://www.w3.org/2009/sparql/docs/tests/data-sparql11/http-rdf-update/

        let server = ServerTest::new();

        // PUT - Initial state
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store/person/1.ttl".parse().unwrap(),
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")
        .unwrap()
        .with_body(
            "
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

<http://$HOST$/$GRAPHSTORE$/person/1> a foaf:Person;
    foaf:businessCard [
        a v:VCard;
        v:fn \"John Doe\"
    ].
",
        );
        server.test_status(request, Status::CREATED);

        // GET of PUT - Initial state
        let request = Request::builder(
            Method::GET,
            "http://localhost/store?graph=/store/person/1.ttl"
                .parse()
                .unwrap(),
        )
        .with_header(HeaderName::ACCEPT, "text/turtle")
        .unwrap()
        .build();
        server.test_status(request, Status::OK);

        // HEAD on an existing graph
        server.test_status(
            Request::builder(
                Method::HEAD,
                "http://localhost/store/person/1.ttl".parse().unwrap(),
            )
            .build(),
            Status::OK,
        );

        // HEAD on a non-existing graph
        server.test_status(
            Request::builder(
                Method::HEAD,
                "http://localhost/store/person/4.ttl".parse().unwrap(),
            )
            .build(),
            Status::NOT_FOUND,
        );

        // PUT - graph already in store
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store/person/1.ttl".parse().unwrap(),
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")
        .unwrap()
        .with_body(
            "
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

<http://$HOST$/$GRAPHSTORE$/person/1> a foaf:Person;
    foaf:businessCard [
        a v:VCard;
        v:fn \"Jane Doe\"
    ].
",
        );
        server.test_status(request, Status::NO_CONTENT);

        // GET of PUT - graph already in store
        let request = Request::builder(
            Method::GET,
            "http://localhost/store/person/1.ttl".parse().unwrap(),
        )
        .with_header(HeaderName::ACCEPT, "text/turtle")
        .unwrap()
        .build();
        server.test_status(request, Status::OK);

        // PUT - default graph
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store?default".parse().unwrap(),
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")
        .unwrap()
        .with_body(
            "
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

[]  a foaf:Person;
    foaf:businessCard [
        a v:VCard;
        v:given-name \"Alice\"
    ] .
",
        );
        server.test_status(request, Status::NO_CONTENT); // The default graph always exists in Oxigraph

        // GET of PUT - default graph
        let request = Request::builder(
            Method::GET,
            "http://localhost/store?default".parse().unwrap(),
        )
        .with_header(HeaderName::ACCEPT, "text/turtle")
        .unwrap()
        .build();
        server.test_status(request, Status::OK);

        // PUT - mismatched payload
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store/person/1.ttl".parse().unwrap(),
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")
        .unwrap()
        .with_body("@prefix fo");
        server.test_status(request, Status::BAD_REQUEST);

        // PUT - empty graph
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store/person/2.ttl".parse().unwrap(),
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")
        .unwrap()
        .build();
        server.test_status(request, Status::CREATED);

        // GET of PUT - empty graph
        let request = Request::builder(
            Method::GET,
            "http://localhost/store/person/2.ttl".parse().unwrap(),
        )
        .with_header(HeaderName::ACCEPT, "text/turtle")
        .unwrap()
        .build();
        server.test_status(request, Status::OK);

        // PUT - replace empty graph
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store/person/2.ttl".parse().unwrap(),
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")
        .unwrap()
        .with_body(
            "
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

[]  a foaf:Person;
    foaf:businessCard [
        a v:VCard;
        v:given-name \"Alice\"
    ] .
",
        );
        server.test_status(request, Status::NO_CONTENT);

        // GET of replacement for empty graph
        let request = Request::builder(
            Method::GET,
            "http://localhost/store/person/2.ttl".parse().unwrap(),
        )
        .with_header(HeaderName::ACCEPT, "text/turtle")
        .unwrap()
        .build();
        server.test_status(request, Status::OK);

        // DELETE - existing graph
        server.test_status(
            Request::builder(
                Method::DELETE,
                "http://localhost/store/person/2.ttl".parse().unwrap(),
            )
            .build(),
            Status::NO_CONTENT,
        );

        // GET of DELETE - existing graph
        server.test_status(
            Request::builder(
                Method::GET,
                "http://localhost/store/person/2.ttl".parse().unwrap(),
            )
            .build(),
            Status::NOT_FOUND,
        );

        // DELETE - non-existent graph
        server.test_status(
            Request::builder(
                Method::DELETE,
                "http://localhost/store/person/2.ttl".parse().unwrap(),
            )
            .build(),
            Status::NOT_FOUND,
        );

        // POST - existing graph
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store/person/1.ttl".parse().unwrap(),
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")
        .unwrap()
        .build();
        server.test_status(request, Status::NO_CONTENT);

        // TODO: POST - multipart/form-data
        // TODO: GET of POST - multipart/form-data

        // POST - create new graph
        let request = Request::builder(Method::POST, "http://localhost/store".parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")
            .unwrap()
            .with_body(
                "
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

[]  a foaf:Person;
    foaf:businessCard [
        a v:VCard;
        v:given-name \"Alice\"
    ] .
",
            );
        let response = server.exec(request);
        assert_eq!(response.status(), Status::CREATED);
        let location = response
            .header(&HeaderName::LOCATION)
            .unwrap()
            .to_str()
            .unwrap();

        // GET of POST - create new graph
        let request = Request::builder(Method::GET, location.parse().unwrap())
            .with_header(HeaderName::ACCEPT, "text/turtle")
            .unwrap()
            .build();
        server.test_status(request, Status::OK);

        // POST - empty graph to existing graph
        let request = Request::builder(Method::PUT, location.parse().unwrap())
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")
            .unwrap()
            .build();
        server.test_status(request, Status::NO_CONTENT);

        // GET of POST - after noop
        let request = Request::builder(Method::GET, location.parse().unwrap())
            .with_header(HeaderName::ACCEPT, "text/turtle")
            .unwrap()
            .build();
        server.test_status(request, Status::OK);
    }

    struct ServerTest {
        store: Store,
    }

    impl ServerTest {
        fn new() -> Self {
            Self {
                store: Store::new().unwrap(),
            }
        }

        fn exec(&self, mut request: Request) -> Response {
            handle_request(&mut request, self.store.clone())
        }

        fn test_status(&self, request: Request, expected_status: Status) {
            let mut response = self.exec(request);
            let mut buf = String::new();
            response.body_mut().read_to_string(&mut buf).unwrap();
            assert_eq!(response.status(), expected_status, "Error message: {}", buf);
        }

        fn test_body(&self, request: Request, expected_body: &str) {
            let mut response = self.exec(request);
            let mut buf = String::new();
            response.body_mut().read_to_string(&mut buf).unwrap();
            assert_eq!(response.status(), Status::OK, "Error message: {}", buf);
            assert_eq!(&buf, expected_body);
        }
    }

    #[test]
    fn clap_debug() {
        use clap::IntoApp;

        Args::command().debug_assert()
    }
}
