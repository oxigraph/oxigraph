use clap::App;
use clap::Arg;
use clap::ArgMatches;
use rouille::input::priority_header_preferred;
use rouille::url::form_urlencoded;
use rouille::{start_server, Request, Response};
use rudf::sparql::QueryResult;
use rudf::sparql::{PreparedQuery, QueryResultSyntax};
use rudf::{
    DatasetSyntax, FileSyntax, GraphSyntax, MemoryRepository, Repository, RepositoryConnection,
    RocksDbRepository,
};
use std::io::{BufReader, Read};
use std::sync::Arc;

const MAX_SPARQL_BODY_SIZE: u64 = 1_048_576;
const HTML_ROOT_PAGE: &str = include_str!("../templates/query.html");

pub fn main() {
    let matches = App::new("Rudf SPARQL server")
        .arg(
            Arg::with_name("bind")
                .short("b")
                .long("bind")
                .help("Specify a server socket to bind using the format $(HOST):$(PORT)")
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
        main_with_dataset(Arc::new(RocksDbRepository::open(file).unwrap()), &matches)
    } else {
        main_with_dataset(Arc::new(MemoryRepository::default()), &matches)
    }
}

fn main_with_dataset<R: Send + Sync + 'static>(repository: Arc<R>, matches: &ArgMatches)
where
    for<'a> &'a R: Repository,
{
    let addr = matches
        .value_of("bind")
        .unwrap_or("127.0.0.1:7878")
        .to_owned();
    println!("Listening for requests at http://{}", &addr);

    start_server(addr.to_string(), move |request| {
        handle_request(request, repository.connection().unwrap(), &addr)
    })
}

fn handle_request<R: RepositoryConnection>(
    request: &Request,
    mut connection: R,
    host: &str,
) -> Response {
    match (request.url().as_str(), request.method()) {
        ("/", "GET") => {
            Response::html(HTML_ROOT_PAGE.replace("{{endpoint}}", &format!("//{}/query", host)))
        }
        ("/", "POST") => {
            if let Some(body) = request.data() {
                if let Some(content_type) = request.header("Content-Type") {
                    match if let Some(format) = GraphSyntax::from_mime_type(content_type) {
                        connection.load_graph(BufReader::new(body), format, None, None)
                    } else if let Some(format) = DatasetSyntax::from_mime_type(content_type) {
                        connection.load_dataset(BufReader::new(body), format, None)
                    } else {
                        return Response::text(format!(
                            "No supported content Content-Type given: {}",
                            content_type
                        ))
                        .with_status_code(415);
                    } {
                        Ok(()) => Response::empty_204(),
                        Err(error) => Response::text(error.to_string()).with_status_code(400),
                    }
                } else {
                    Response::text("No Content-Type given").with_status_code(400)
                }
            } else {
                Response::text("No content given").with_status_code(400)
            }
        }
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
    match connection.prepare_query(query, None) {
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

#[cfg(test)]
mod tests {
    use crate::handle_request;
    use rouille::Request;
    use rudf::{MemoryRepository, Repository};
    use std::io::Read;

    #[test]
    fn get_ui() {
        exec(Request::fake_http("GET", "/", vec![], vec![]))
    }

    #[test]
    fn get_query() {
        exec(Request::fake_http(
            "GET",
            "/query?query=SELECT+*+WHERE+{+?s+?p+?o+}",
            vec![(
                "Content-Type".to_string(),
                "application/sparql-query".to_string(),
            )],
            b"SELECT * WHERE { ?s ?p ?o }".to_vec(),
        ))
    }

    #[test]
    fn post_query() {
        exec(Request::fake_http(
            "POST",
            "/query",
            vec![(
                "Content-Type".to_string(),
                "application/sparql-query".to_string(),
            )],
            b"SELECT * WHERE { ?s ?p ?o }".to_vec(),
        ))
    }

    fn exec(request: Request) {
        let response = handle_request(
            &request,
            MemoryRepository::default().connection().unwrap(),
            "localhost",
        );
        let mut body = String::default();
        request
            .data()
            .map(|mut r| r.read_to_string(&mut body).unwrap());
        assert_eq!(response.status_code, 200, "{}", body);
    }
}
