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

use async_std::future::Future;
use async_std::io::Read;
use async_std::net::{TcpListener, TcpStream};
use async_std::prelude::*;
use async_std::task::{block_on, spawn};
use clap::{crate_version, App, Arg};
use http_types::content::ContentType;
use http_types::{
    bail_status, format_err_status, headers, Error, Method, Mime, Request, Response, Result,
    StatusCode,
};
use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::{GraphName, GraphNameRef, NamedNode, NamedOrBlankNode};
use oxigraph::sparql::algebra::GraphUpdateOperation;
use oxigraph::sparql::{Query, QueryResults, QueryResultsFormat, Update};
#[cfg(feature = "rocksdb")]
use oxigraph::RocksDbStore as Store;
#[cfg(all(feature = "sled", not(feature = "rocksdb")))]
use oxigraph::SledStore as Store;
use oxiri::Iri;
use rand::random;
use std::io::BufReader;
use std::str::FromStr;
use url::form_urlencoded;

const MAX_SPARQL_BODY_SIZE: u64 = 1_048_576;
const HTML_ROOT_PAGE: &str = include_str!("../templates/query.html");
const LOGO: &str = include_str!("../logo.svg");
const SERVER: &str = concat!("Oxigraph/", env!("CARGO_PKG_VERSION"));

#[async_std::main]
pub async fn main() -> Result<()> {
    let matches = App::new("Oxigraph SPARQL server")
        .version(crate_version!())
        .arg(
            Arg::with_name("bind")
                .short("b")
                .long("bind")
                .help("Host and port to listen to")
                .default_value("localhost:7878")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("file")
                .short("f")
                .long("file")
                .help("directory in which persist the data")
                .takes_value(true)
                .required(true),
        )
        .get_matches();
    let bind = matches.value_of("bind").unwrap();
    let file = matches.value_of_os("file").unwrap();

    let store = Store::open(file)?;
    println!("Listening for requests at http://{}", &bind);
    http_server(bind, move |request| handle_request(request, store.clone())).await
}

async fn handle_request(request: Request, store: Store) -> Result<Response> {
    Ok(match (request.url().path(), request.method()) {
        ("/", Method::Get) => {
            let mut response = Response::new(StatusCode::Ok);
            ContentType::new("text/html").apply(&mut response);
            response.set_body(HTML_ROOT_PAGE);
            response
        }
        ("/logo.svg", Method::Get) => {
            let mut response = Response::new(StatusCode::Ok);
            ContentType::new("image/svg+xml").apply(&mut response);
            response.set_body(LOGO);
            response
        }
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
                    configure_and_evaluate_sparql_update(
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
                    configure_and_evaluate_sparql_update(store, buffer, None, request)?
                } else {
                    bail_status!(415, "Not supported Content-Type given: {}", content_type);
                }
            } else {
                bail_status!(400, "No Content-Type given");
            }
        }
        (path, Method::Get) if path.starts_with("/store") => {
            //TODO: stream
            let mut body = Vec::default();
            let format = if let Some(target) = store_target(&request)? {
                if !match &target {
                    GraphName::DefaultGraph => true,
                    GraphName::NamedNode(target) => store.contains_named_graph(target)?,
                    GraphName::BlankNode(target) => store.contains_named_graph(target)?,
                } {
                    bail_status!(404, "The graph {} does not exists", target);
                }
                let format = graph_content_negotiation(request)?;
                store.dump_graph(&mut body, format, &target)?;
                format.media_type()
            } else {
                let format = dataset_content_negotiation(request)?;
                store.dump_dataset(&mut body, format)?;
                format.media_type()
            };
            let mut response = Response::from(body);
            ContentType::new(format).apply(&mut response);
            response
        }
        (path, Method::Put) if path.starts_with("/store") => {
            if let Some(content_type) = request.content_type() {
                if let Some(target) = store_target(&request)? {
                    if let Some(format) = GraphFormat::from_media_type(content_type.essence()) {
                        let new = !match &target {
                            GraphName::NamedNode(target) => {
                                if store.contains_named_graph(target)? {
                                    store.clear_graph(target)?;
                                    true
                                } else {
                                    store.insert_named_graph(target)?;
                                    false
                                }
                            }
                            GraphName::BlankNode(target) => {
                                if store.contains_named_graph(target)? {
                                    store.clear_graph(target)?;
                                    true
                                } else {
                                    store.insert_named_graph(target)?;
                                    false
                                }
                            }
                            GraphName::DefaultGraph => {
                                store.clear_graph(&target)?;
                                true
                            }
                        };
                        store
                            .load_graph(
                                BufReader::new(SyncAsyncReader::from(request)),
                                format,
                                &target,
                                None,
                            )
                            .map_err(bad_request)?;
                        Response::new(if new {
                            StatusCode::Created
                        } else {
                            StatusCode::NoContent
                        })
                    } else {
                        bail_status!(
                            415,
                            "No supported content Content-Type given: {}",
                            content_type
                        );
                    }
                } else if let Some(format) = DatasetFormat::from_media_type(content_type.essence())
                {
                    store.clear()?;
                    store
                        .load_dataset(BufReader::new(SyncAsyncReader::from(request)), format, None)
                        .map_err(bad_request)?;
                    Response::new(StatusCode::NoContent)
                } else {
                    bail_status!(
                        415,
                        "No supported content Content-Type given: {}",
                        content_type
                    );
                }
            } else {
                bail_status!(400, "No Content-Type given");
            }
        }
        (path, Method::Delete) if path.starts_with("/store") => {
            if let Some(target) = store_target(&request)? {
                match target {
                    GraphName::DefaultGraph => store.clear_graph(GraphNameRef::DefaultGraph)?,
                    GraphName::NamedNode(target) => {
                        if store.contains_named_graph(&target)? {
                            store.remove_named_graph(&target)?;
                        } else {
                            bail_status!(404, "The graph {} does not exists", target);
                        }
                    }
                    GraphName::BlankNode(target) => {
                        if store.contains_named_graph(&target)? {
                            store.remove_named_graph(&target)?;
                        } else {
                            bail_status!(404, "The graph {} does not exists", target);
                        }
                    }
                }
            } else {
                store.clear()?;
            }
            Response::new(StatusCode::NoContent)
        }
        (path, Method::Post) if path.starts_with("/store") => {
            if let Some(content_type) = request.content_type() {
                if let Some(target) = store_target(&request)? {
                    if let Some(format) = GraphFormat::from_media_type(content_type.essence()) {
                        let new = !match &target {
                            GraphName::NamedNode(target) => store.contains_named_graph(target)?,
                            GraphName::BlankNode(target) => store.contains_named_graph(target)?,
                            GraphName::DefaultGraph => true,
                        };
                        store
                            .load_graph(
                                BufReader::new(SyncAsyncReader::from(request)),
                                format,
                                &target,
                                None,
                            )
                            .map_err(bad_request)?;
                        Response::new(if new {
                            StatusCode::Created
                        } else {
                            StatusCode::NoContent
                        })
                    } else {
                        bail_status!(
                            415,
                            "No supported content Content-Type given: {}",
                            content_type
                        );
                    }
                } else if let Some(format) = DatasetFormat::from_media_type(content_type.essence())
                {
                    store
                        .load_dataset(BufReader::new(SyncAsyncReader::from(request)), format, None)
                        .map_err(bad_request)?;
                    Response::new(StatusCode::NoContent)
                } else if let Some(format) = GraphFormat::from_media_type(content_type.essence()) {
                    let graph =
                        resolve_with_base(&request, &format!("/store/{:x}", random::<u128>()))?;
                    store
                        .load_graph(
                            BufReader::new(SyncAsyncReader::from(request)),
                            format,
                            &graph,
                            None,
                        )
                        .map_err(bad_request)?;
                    let mut response = Response::new(StatusCode::Created);
                    response.insert_header(headers::LOCATION, graph.into_string());
                    response
                } else {
                    bail_status!(
                        415,
                        "No supported content Content-Type given: {}",
                        content_type
                    );
                }
            } else {
                bail_status!(400, "No Content-Type given")
            }
        }
        (path, Method::Head) if path.starts_with("/store") => {
            if let Some(target) = store_target(&request)? {
                if !match &target {
                    GraphName::DefaultGraph => true,
                    GraphName::NamedNode(target) => store.contains_named_graph(target)?,
                    GraphName::BlankNode(target) => store.contains_named_graph(target)?,
                } {
                    bail_status!(404, "The graph {} does not exists", target);
                }
                Response::new(StatusCode::Ok)
            } else {
                Response::new(StatusCode::Ok)
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

fn base_url(request: &Request) -> Result<String> {
    let mut url = request.url().clone();
    if let Some(host) = request.host() {
        url.set_host(Some(host)).map_err(bad_request)?;
    }
    url.set_query(None);
    url.set_fragment(None);
    Ok(url.into())
}

fn resolve_with_base(request: &Request, url: &str) -> Result<NamedNode> {
    Ok(NamedNode::new_unchecked(
        Iri::parse(base_url(request)?)
            .map_err(bad_request)?
            .resolve(url)
            .map_err(bad_request)?
            .into_inner(),
    ))
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
    let mut use_default_graph_as_union = false;
    for (k, v) in form_urlencoded::parse(&encoded) {
        match k.as_ref() {
            "query" => {
                if query.is_some() {
                    bail_status!(400, "Multiple query parameters provided")
                }
                query = Some(v.into_owned())
            }
            "default-graph-uri" => default_graph_uris.push(v.into_owned()),
            "union-default-graph" => use_default_graph_as_union = true,
            "named-graph-uri" => named_graph_uris.push(v.into_owned()),
            _ => (),
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
        bail_status!(400, "You should set the 'query' parameter")
    }
}

fn evaluate_sparql_query(
    store: Store,
    query: String,
    use_default_graph_as_union: bool,
    default_graph_uris: Vec<String>,
    named_graph_uris: Vec<String>,
    request: Request,
) -> Result<Response> {
    let mut query =
        Query::parse(&query, Some(base_url(&request)?.as_str())).map_err(bad_request)?;

    if use_default_graph_as_union {
        if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
            bail_status!(
                400,
                "default-graph-uri or named-graph-uri and union-default-graph should not be set at the same time"
            );
        }
        query.dataset_mut().set_default_graph_as_union()
    } else if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
        query.dataset_mut().set_default_graph(
            default_graph_uris
                .into_iter()
                .map(|e| Ok(NamedNode::new(e)?.into()))
                .collect::<Result<_>>()
                .map_err(bad_request)?,
        );
        query.dataset_mut().set_available_named_graphs(
            named_graph_uris
                .into_iter()
                .map(|e| Ok(NamedNode::new(e)?.into()))
                .collect::<Result<_>>()
                .map_err(bad_request)?,
        );
    }

    let results = store.query(query)?;
    //TODO: stream
    if let QueryResults::Graph(_) = results {
        let format = graph_content_negotiation(request)?;
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

fn configure_and_evaluate_sparql_update(
    store: Store,
    encoded: Vec<u8>,
    mut update: Option<String>,
    request: Request,
) -> Result<Response> {
    let mut use_default_graph_as_union = false;
    let mut default_graph_uris = Vec::new();
    let mut named_graph_uris = Vec::new();
    for (k, v) in form_urlencoded::parse(&encoded) {
        match k.as_ref() {
            "update" => {
                if update.is_some() {
                    bail_status!(400, "Multiple update parameters provided")
                }
                update = Some(v.into_owned())
            }
            "using-graph-uri" => default_graph_uris.push(v.into_owned()),
            "using-union-graph" => use_default_graph_as_union = true,
            "using-named-graph-uri" => named_graph_uris.push(v.into_owned()),
            _ => (),
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
        bail_status!(400, "You should set the 'update' parameter")
    }
}

fn evaluate_sparql_update(
    store: Store,
    update: String,
    use_default_graph_as_union: bool,
    default_graph_uris: Vec<String>,
    named_graph_uris: Vec<String>,
    request: Request,
) -> Result<Response> {
    let mut update =
        Update::parse(&update, Some(base_url(&request)?.as_str())).map_err(bad_request)?;

    if use_default_graph_as_union {
        if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
            bail_status!(
                400,
                "using-graph-uri or using-named-graph-uri and using-union-graph should not be set at the same time"
            );
        }
        for operation in &mut update.operations {
            if let GraphUpdateOperation::DeleteInsert { using, .. } = operation {
                if !using.is_default_dataset() {
                    bail_status!(
                        400,
                        "using-union-graph must not be used with a SPARQL UPDATE containing USING",
                    );
                }
                using.set_default_graph_as_union();
            }
        }
    } else if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
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
        for operation in &mut update.operations {
            if let GraphUpdateOperation::DeleteInsert { using, .. } = operation {
                if !using.is_default_dataset() {
                    bail_status!(
                        400,
                        "using-graph-uri and using-named-graph-uri must not be used with a SPARQL UPDATE containing USING",
                    );
                }
                using.set_default_graph(default_graph_uris.clone());
                using.set_available_named_graphs(named_graph_uris.clone());
            }
        }
    }
    store.update(update)?;
    Ok(Response::new(StatusCode::NoContent))
}

fn store_target(request: &Request) -> Result<Option<GraphName>> {
    if request.url().path() == "/store" {
        let mut graph = None;
        let mut default = false;
        for (k, v) in form_urlencoded::parse(request.url().query().unwrap_or("").as_bytes()) {
            match k.as_ref() {
                "graph" => graph = Some(v.into_owned()),
                "default" => default = true,
                _ => {
                    bail_status!(400, "Unexpected parameter: {}", k);
                }
            }
        }
        Ok(if let Some(graph) = graph {
            if default {
                bail_status!(
                    400,
                    "Both graph and default parameters should not be set at the same time",
                );
            } else {
                Some(resolve_with_base(request, &graph)?.into())
            }
        } else if default {
            Some(GraphName::DefaultGraph)
        } else {
            None
        })
    } else {
        Ok(Some(resolve_with_base(request, "")?.into()))
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

    let listener = TcpListener::bind(host).await?;
    let mut incoming = listener.incoming();
    while let Some(stream) = incoming.next().await {
        let stream = stream?;
        let handle = handle.clone();
        spawn(async {
            if let Err(error) = accept(stream, handle).await {
                eprintln!("{}", error);
            };
        });
    }
    Ok(())
}

fn graph_content_negotiation(request: Request) -> Result<GraphFormat> {
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

fn dataset_content_negotiation(request: Request) -> Result<DatasetFormat> {
    content_negotiation(
        request,
        &[
            DatasetFormat::NQuads.media_type(),
            DatasetFormat::TriG.media_type(),
        ],
        DatasetFormat::from_media_type,
    )
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
    use super::*;
    use crate::handle_request;
    use async_std::task::block_on;
    use http_types::Url;
    use tempfile::{tempdir, TempDir};

    #[test]
    fn get_ui() {
        ServerTest::new().test_status(
            Request::new(Method::Get, Url::parse("http://localhost/").unwrap()),
            StatusCode::Ok,
        )
    }

    #[test]
    fn post_dataset_file() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/store").unwrap());
        request.insert_header("Content-Type", "application/trig");
        request.set_body("<http://example.com> <http://example.com> <http://example.com> .");
        ServerTest::new().test_status(request, StatusCode::NoContent)
    }

    #[test]
    fn post_wrong_file() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/store").unwrap());
        request.insert_header("Content-Type", "application/trig");
        request.set_body("<http://example.com>");
        ServerTest::new().test_status(request, StatusCode::BadRequest)
    }

    #[test]
    fn post_unsupported_file() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/store").unwrap());
        request.insert_header("Content-Type", "text/foo");
        ServerTest::new().test_status(request, StatusCode::UnsupportedMediaType)
    }

    #[test]
    fn get_query() {
        ServerTest::new().test_status(
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
    fn get_query_accept_star() {
        let mut request = Request::new(
            Method::Get,
            Url::parse("http://localhost/query?query=SELECT%20*%20WHERE%20{%20?s%20?p%20?o%20}")
                .unwrap(),
        );
        request.insert_header("Accept", "*/*");
        ServerTest::new().test_status(request, StatusCode::Ok);
    }

    #[test]
    fn get_query_accept_good() {
        let mut request = Request::new(
            Method::Get,
            Url::parse("http://localhost/query?query=SELECT%20*%20WHERE%20{%20?s%20?p%20?o%20}")
                .unwrap(),
        );
        request.insert_header("Accept", "application/sparql-results+json;charset=utf-8");
        ServerTest::new().test_status(request, StatusCode::Ok);
    }

    #[test]
    fn get_query_accept_bad() {
        let mut request = Request::new(
            Method::Get,
            Url::parse("http://localhost/query?query=SELECT%20*%20WHERE%20{%20?s%20?p%20?o%20}")
                .unwrap(),
        );
        request.insert_header("Accept", "application/foo");
        ServerTest::new().test_status(request, StatusCode::NotAcceptable);
    }

    #[test]
    fn get_bad_query() {
        ServerTest::new().test_status(
            Request::new(
                Method::Get,
                Url::parse("http://localhost/query?query=SELECT").unwrap(),
            ),
            StatusCode::BadRequest,
        );
    }

    #[test]
    fn get_query_union_graph() {
        ServerTest::new().test_status(Request::new(
            Method::Get,
            Url::parse("http://localhost/query?query=SELECT%20*%20WHERE%20{%20?s%20?p%20?o%20}&union-default-graph")
                .unwrap(),
        ), StatusCode::Ok);
    }

    #[test]
    fn get_query_union_graph_and_default_graph() {
        ServerTest::new().test_status(Request::new(
            Method::Get,
            Url::parse("http://localhost/query?query=SELECT%20*%20WHERE%20{%20?s%20?p%20?o%20}&union-default-graph&default-graph-uri=http://example.com")
                .unwrap(),
        ), StatusCode::BadRequest);
    }

    #[test]
    fn get_without_query() {
        ServerTest::new().test_status(
            Request::new(Method::Get, Url::parse("http://localhost/query").unwrap()),
            StatusCode::BadRequest,
        );
    }

    #[test]
    fn post_query() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/query").unwrap());
        request.insert_header("Content-Type", "application/sparql-query");
        request.set_body("SELECT * WHERE { ?s ?p ?o }");
        ServerTest::new().test_status(request, StatusCode::Ok)
    }

    #[test]
    fn post_bad_query() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/query").unwrap());
        request.insert_header("Content-Type", "application/sparql-query");
        request.set_body("SELECT");
        ServerTest::new().test_status(request, StatusCode::BadRequest)
    }

    #[test]
    fn post_unknown_query() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/query").unwrap());
        request.insert_header("Content-Type", "application/sparql-todo");
        request.set_body("SELECT");
        ServerTest::new().test_status(request, StatusCode::UnsupportedMediaType)
    }

    #[test]
    fn post_federated_query() {
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/query").unwrap());
        request.insert_header("Content-Type", "application/sparql-query");
        request.set_body("SELECT * WHERE { SERVICE <https://query.wikidata.org/sparql> { <https://en.wikipedia.org/wiki/Paris> ?p ?o } }");
        ServerTest::new().test_status(request, StatusCode::Ok)
    }

    #[test]
    fn post_update() {
        let mut request =
            Request::new(Method::Post, Url::parse("http://localhost/update").unwrap());
        request.insert_header("Content-Type", "application/sparql-update");
        request.set_body(
            "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
        );
        ServerTest::new().test_status(request, StatusCode::NoContent)
    }

    #[test]
    fn post_bad_update() {
        let mut request =
            Request::new(Method::Post, Url::parse("http://localhost/update").unwrap());
        request.insert_header("Content-Type", "application/sparql-update");
        request.set_body("INSERT");
        ServerTest::new().test_status(request, StatusCode::BadRequest)
    }

    #[test]
    fn graph_store_url_normalization() {
        let server = ServerTest::new();

        // PUT
        let mut request = Request::new(
            Method::Put,
            Url::parse("http://localhost/store?graph=http://example.com").unwrap(),
        );
        request.insert_header("Content-Type", "text/turtle");
        request.set_body("<http://example.com> <http://example.com> <http://example.com> .");
        server.test_status(request, StatusCode::Created);

        // GET good URI
        server.test_status(
            Request::new(
                Method::Get,
                Url::parse("http://localhost/store?graph=http://example.com").unwrap(),
            ),
            StatusCode::Ok,
        );

        // GET bad URI
        server.test_status(
            Request::new(
                Method::Get,
                Url::parse("http://localhost/store?graph=http://example.com/").unwrap(),
            ),
            StatusCode::NotFound,
        );
    }

    #[test]
    fn graph_store_protocol() {
        // Tests from https://www.w3.org/2009/sparql/docs/tests/data-sparql11/http-rdf-update/

        let server = ServerTest::new();

        // PUT - Initial state
        let mut request = Request::new(
            Method::Put,
            Url::parse("http://localhost/store/person/1.ttl").unwrap(),
        );
        request.insert_header("Content-Type", "text/turtle; charset=utf-8");
        request.set_body(
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
        server.test_status(request, StatusCode::Created);

        // GET of PUT - Initial state
        let mut request = Request::new(
            Method::Get,
            Url::parse("http://localhost/store?graph=/store/person/1.ttl").unwrap(),
        );
        request.insert_header("Accept", "text/turtle");
        server.test_status(request, StatusCode::Ok);

        // HEAD on an existing graph
        server.test_status(
            Request::new(
                Method::Head,
                Url::parse("http://localhost/store/person/1.ttl").unwrap(),
            ),
            StatusCode::Ok,
        );

        // HEAD on a non-existing graph
        server.test_status(
            Request::new(
                Method::Head,
                Url::parse("http://localhost/store/person/4.ttl").unwrap(),
            ),
            StatusCode::NotFound,
        );

        // PUT - graph already in store
        let mut request = Request::new(
            Method::Put,
            Url::parse("http://localhost/store/person/1.ttl").unwrap(),
        );
        request.insert_header("Content-Type", "text/turtle; charset=utf-8");
        request.set_body(
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
        server.test_status(request, StatusCode::NoContent);

        // GET of PUT - graph already in store
        let mut request = Request::new(
            Method::Get,
            Url::parse("http://localhost/store/person/1.ttl").unwrap(),
        );
        request.insert_header("Accept", "text/turtle");
        server.test_status(request, StatusCode::Ok);

        // PUT - default graph
        let mut request = Request::new(
            Method::Put,
            Url::parse("http://localhost/store?default").unwrap(),
        );
        request.insert_header("Content-Type", "text/turtle; charset=utf-8");
        request.set_body(
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
        server.test_status(request, StatusCode::NoContent); // The default graph always exists in Oxigraph

        // GET of PUT - default graph
        let mut request = Request::new(
            Method::Get,
            Url::parse("http://localhost/store?default").unwrap(),
        );
        request.insert_header("Accept", "text/turtle");
        server.test_status(request, StatusCode::Ok);

        // PUT - mismatched payload
        let mut request = Request::new(
            Method::Put,
            Url::parse("http://localhost/store/person/1.ttl").unwrap(),
        );
        request.insert_header("Content-Type", "text/turtle; charset=utf-8");
        request.set_body("@prefix fo");
        server.test_status(request, StatusCode::BadRequest);

        // PUT - empty graph
        let mut request = Request::new(
            Method::Put,
            Url::parse("http://localhost/store/person/2.ttl").unwrap(),
        );
        request.insert_header("Content-Type", "text/turtle; charset=utf-8");
        server.test_status(request, StatusCode::Created);

        // GET of PUT - empty graph
        let mut request = Request::new(
            Method::Get,
            Url::parse("http://localhost/store/person/2.ttl").unwrap(),
        );
        request.insert_header("Accept", "text/turtle");
        server.test_status(request, StatusCode::Ok);

        // PUT - replace empty graph
        let mut request = Request::new(
            Method::Put,
            Url::parse("http://localhost/store/person/2.ttl").unwrap(),
        );
        request.insert_header("Content-Type", "text/turtle; charset=utf-8");
        request.set_body(
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
        server.test_status(request, StatusCode::NoContent);

        // GET of replacement for empty graph
        let mut request = Request::new(
            Method::Get,
            Url::parse("http://localhost/store/person/2.ttl").unwrap(),
        );
        request.insert_header("Accept", "text/turtle");
        server.test_status(request, StatusCode::Ok);

        // DELETE - existing graph
        server.test_status(
            Request::new(
                Method::Delete,
                Url::parse("http://localhost/store/person/2.ttl").unwrap(),
            ),
            StatusCode::NoContent,
        );

        // GET of DELETE - existing graph
        server.test_status(
            Request::new(
                Method::Get,
                Url::parse("http://localhost/store/person/2.ttl").unwrap(),
            ),
            StatusCode::NotFound,
        );

        // DELETE - non-existent graph
        server.test_status(
            Request::new(
                Method::Delete,
                Url::parse("http://localhost/store/person/2.ttl").unwrap(),
            ),
            StatusCode::NotFound,
        );

        // POST - existing graph
        let mut request = Request::new(
            Method::Put,
            Url::parse("http://localhost/store/person/1.ttl").unwrap(),
        );
        request.insert_header("Content-Type", "text/turtle; charset=utf-8");
        server.test_status(request, StatusCode::NoContent);

        // TODO: POST - multipart/form-data
        // TODO: GET of POST - multipart/form-data

        // POST - create new graph
        let mut request = Request::new(Method::Post, Url::parse("http://localhost/store").unwrap());
        request.insert_header("Content-Type", "text/turtle; charset=utf-8");
        request.set_body(
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
        assert_eq!(response.status(), StatusCode::Created);
        let location = response.header("Location").unwrap().as_str();

        // GET of POST - create new graph
        let mut request = Request::new(Method::Get, Url::parse(location).unwrap());
        request.insert_header("Accept", "text/turtle");
        server.test_status(request, StatusCode::Ok);

        // POST - empty graph to existing graph
        let mut request = Request::new(Method::Put, Url::parse(location).unwrap());
        request.insert_header("Content-Type", "text/turtle; charset=utf-8");
        server.test_status(request, StatusCode::NoContent);

        // GET of POST - after noop
        let mut request = Request::new(Method::Get, Url::parse(location).unwrap());
        request.insert_header("Accept", "text/turtle");
        server.test_status(request, StatusCode::Ok);
    }

    struct ServerTest {
        store: Store,
        _path: TempDir,
    }

    impl ServerTest {
        fn new() -> ServerTest {
            let path = tempdir().unwrap();
            let store = Store::open(path.path()).unwrap();
            ServerTest { _path: path, store }
        }

        fn exec(&self, request: Request) -> Response {
            match block_on(handle_request(request, self.store.clone())) {
                Ok(response) => response,
                Err(e) => {
                    let mut response = Response::new(e.status());
                    response.set_body(e.to_string());
                    response
                }
            }
        }

        fn test_status(&self, request: Request, expected_status: StatusCode) {
            let mut response = self.exec(request);
            assert_eq!(
                response.status(),
                expected_status,
                "Error message: {}",
                block_on(response.body_string()).unwrap()
            );
        }
    }
}
