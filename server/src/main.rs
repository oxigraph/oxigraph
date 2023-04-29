#![allow(clippy::print_stderr, clippy::cast_precision_loss, clippy::use_debug)]
use anyhow::{anyhow, bail, Context, Error};
use clap::{Parser, Subcommand};
use flate2::read::MultiGzDecoder;
use oxhttp::model::{Body, HeaderName, HeaderValue, Method, Request, Response, Status};
use oxhttp::Server;
use oxigraph::io::{DatasetFormat, DatasetSerializer, GraphFormat, GraphSerializer};
use oxigraph::model::{
    GraphName, GraphNameRef, IriParseError, NamedNode, NamedNodeRef, NamedOrBlankNode,
};
use oxigraph::sparql::{Query, QueryOptions, QueryResults, Update};
use oxigraph::store::{BulkLoader, LoaderError, Store};
use oxiri::Iri;
use rand::random;
use rayon_core::ThreadPoolBuilder;
use sparesults::{QueryResultsFormat, QueryResultsSerializer};
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::{max, min};
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, stdin, stdout, BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::thread::available_parallelism;
use std::time::{Duration, Instant};
use std::{fmt, fs, str};
use url::form_urlencoded;

const MAX_SPARQL_BODY_SIZE: u64 = 0x0010_0000;
const HTTP_TIMEOUT: Duration = Duration::from_secs(60);
const HTML_ROOT_PAGE: &str = include_str!("../templates/query.html");
const LOGO: &str = include_str!("../logo.svg");

#[derive(Parser)]
#[command(about, version)]
/// Oxigraph SPARQL server.
struct Args {
    /// Directory in which the data should be persisted.
    ///
    /// If not present. An in-memory storage will be used.
    #[arg(short, long, global = true)]
    location: Option<PathBuf>, //TODO: move into commands on next breaking release
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start Oxigraph HTTP server in read-write mode.
    Serve {
        /// Host and port to listen to.
        #[arg(short, long, default_value = "localhost:7878")]
        bind: String,
        /// Allows cross-origin requests
        #[arg(long)]
        cors: bool,
    },
    /// Start Oxigraph HTTP server in read-only mode.
    ///
    /// It allows to read the database while other processes are also reading it.
    /// Opening as read-only while having an other process writing the database is undefined behavior.
    /// Please use the serve-secondary command in this case.
    ServeReadOnly {
        /// Host and port to listen to.
        #[arg(short, long, default_value = "localhost:7878")]
        bind: String,
        /// Allows cross-origin requests
        #[arg(long)]
        cors: bool,
    },
    /// Start Oxigraph HTTP server in secondary mode.
    ///
    /// It allows to read the database while an other process is writing it.
    /// Changes done while this process is running will be replicated after a possible lag.
    ///
    /// Beware: RocksDB secondary mode does not support snapshots and transactions.
    /// Dirty reads might happen.
    ServeSecondary {
        /// Directory where the primary Oxigraph instance is writing to.
        #[arg(long, conflicts_with = "location")]
        primary_location: Option<PathBuf>,
        /// Directory to which the current secondary instance might write to.
        ///
        /// By default, temporary storage is used.
        #[arg(long)]
        secondary_location: Option<PathBuf>,
        /// Host and port to listen to.
        #[arg(short, long, default_value = "localhost:7878")]
        bind: String,
        /// Allows cross-origin requests
        #[arg(long)]
        cors: bool,
    },
    /// Creates database backup into a target directory.
    ///
    /// After its creation, the backup is usable a separated Oxigraph database
    /// and operates independently from the original database.
    ///
    /// If the target directory is in the same file system as the current database,
    /// the database content will not be fully copied
    /// but hard links will be used to point to the original database immutable snapshots.
    /// This allows cheap regular backups.
    ///
    /// If you want to move your data to another RDF storage system, you should use the dump operation instead.
    Backup {
        /// Directory in which the backup will be written.
        #[arg(short, long)]
        destination: PathBuf,
    },
    /// Load file(s) into the store.
    Load {
        /// File(s) to load.
        ///
        /// If multiple files are provided they are loaded in parallel.
        ///
        /// If no file is given, stdin is read.
        #[arg(short, long, num_args = 0..)]
        file: Vec<PathBuf>,
        /// The format of the file(s) to load.
        ///
        /// Can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default the format is guessed from the loaded file extension.
        #[arg(long, required_unless_present = "file")]
        format: Option<String>,
        /// Attempt to keep loading even if the data file is invalid.
        ///
        /// Only works with N-Triples and N-Quads for now.
        #[arg(long)]
        lenient: bool,
        /// Name of the graph to load the data to.
        ///
        /// By default the default graph is used.
        ///
        /// Only available when loading a graph file (N-Triples, Turtle...) and not a dataset file (N-Quads, TriG...).
        #[arg(long)]
        graph: Option<String>,
    },
    /// Dump the store content into a file.
    Dump {
        /// File to dump to.
        ///
        /// If no file is given, stdout is used.
        #[arg(short, long)]
        file: Option<PathBuf>,
        /// The format of the file(s) to dump.
        ///
        /// Can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default the format is guessed from the target file extension.
        #[arg(long, required_unless_present = "file")]
        format: Option<String>,
        /// Name of the graph to dump.
        ///
        /// By default all graphs are dumped if the output format supports datasets.
        #[arg(long)]
        graph: Option<String>,
    },
    /// Executes a SPARQL query against the store.
    Query {
        /// The SPARQL query to execute.
        ///
        /// If no query or query file are given, stdin is used.
        #[arg(short, long, conflicts_with = "query_file")]
        query: Option<String>,
        /// File in which the query is stored.
        ///
        /// If no query or query file are given, stdin is used.
        #[arg(long, conflicts_with = "query")]
        query_file: Option<PathBuf>,
        /// Base URI of the query.
        #[arg(long)]
        query_base: Option<String>,
        /// File in which the query results will be stored.
        ///
        /// If no file is given, stdout is used.
        #[arg(short, long)]
        results_file: Option<PathBuf>,
        /// The format of the results.
        ///
        /// Can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default the format is guessed from the results file extension.
        #[arg(long, required_unless_present = "results_file")]
        results_format: Option<String>,
        /// Prints to stderr a human-readable explanation of the query evaluation.
        ///
        /// Use the stats option to print also query evaluation statistics.
        #[arg(long, conflicts_with = "explain_file")]
        explain: bool,
        /// Write to the given file an explanation of the query evaluation.
        ///
        /// If the file extension is .json the JSON format is used, if .txt a human readable format is used.
        ///
        /// Use the stats option to print also query evaluation statistics.
        #[arg(long, conflicts_with = "explain")]
        explain_file: Option<PathBuf>,
        /// Computes some evaluation statistics to print as part of the query explanations.
        ///
        /// Beware, computing the statistics adds some overhead to the evaluation runtime.
        #[arg(long)]
        stats: bool,
    },
    /// Executes a SPARQL update against the store.
    Update {
        /// The SPARQL update to execute.
        ///
        /// If no query or query file are given, stdin is used.
        #[arg(short, long, conflicts_with = "update_file")]
        update: Option<String>,
        /// File in which the update is stored.
        ///
        /// If no update or update file are given, stdin is used.
        #[arg(long, conflicts_with = "update")]
        update_file: Option<PathBuf>,
        /// Base URI of the update.
        #[arg(long)]
        update_base: Option<String>,
    },
    /// Optimizes the database storage.
    ///
    /// Done by default in the background when serving requests.
    /// It is likely to not be useful in most of cases except if you provide a read-only SPARQL endpoint under heavy load.
    Optimize {},
}

pub fn main() -> anyhow::Result<()> {
    let matches = Args::parse();
    match matches.command {
        Command::Serve { bind, cors } => serve(
            if let Some(location) = matches.location {
                Store::open(location)
            } else {
                Store::new()
            }?,
            bind,
            false,
            cors,
        ),
        Command::ServeReadOnly { bind, cors } => serve(
            Store::open_read_only(
                matches
                    .location
                    .ok_or_else(|| anyhow!("The --location argument is required"))?,
            )?,
            bind,
            true,
            cors,
        ),
        Command::ServeSecondary {
            primary_location,
            secondary_location,
            bind,
            cors,
        } => {
            let primary_location = primary_location.or(matches.location).ok_or_else(|| {
                anyhow!("Either the --location or the --primary-location argument is required")
            })?;
            serve(
                if let Some(secondary_location) = secondary_location {
                    Store::open_persistent_secondary(primary_location, secondary_location)
                } else {
                    Store::open_secondary(primary_location)
                }?,
                bind,
                true,
                cors,
            )
        }
        Command::Backup { destination } => {
            let store = Store::open_read_only(
                matches
                    .location
                    .ok_or_else(|| anyhow!("The --location argument is required"))?,
            )?;
            store.backup(destination)?;
            Ok(())
        }
        Command::Load {
            file,
            lenient,
            format,
            graph,
        } => {
            let store = if let Some(location) = matches.location {
                Store::open(location)
            } else {
                eprintln!("Warning: opening an in-memory store. It will not be possible to read the written data.");
                Store::new()
            }?;
            let format = if let Some(format) = format {
                Some(GraphOrDatasetFormat::from_str(&format)?)
            } else {
                None
            };
            let graph = if let Some(iri) = &graph {
                Some(
                    NamedNodeRef::new(iri)
                        .with_context(|| format!("The target graph name {iri} is invalid"))?,
                )
            } else {
                None
            };
            #[allow(clippy::cast_precision_loss)]
            if file.is_empty() {
                // We read from stdin
                let start = Instant::now();
                let mut loader = store.bulk_loader().on_progress(move |size| {
                    let elapsed = start.elapsed();
                    eprintln!(
                        "{size} triples loaded in {}s ({} t/s)",
                        elapsed.as_secs(),
                        ((size as f64) / elapsed.as_secs_f64()).round()
                    )
                });
                if lenient {
                    loader = loader.on_parse_error(move |e| {
                        eprintln!("Parsing error: {e}");
                        Ok(())
                    })
                }
                bulk_load(
                    &loader,
                    stdin().lock(),
                    format.ok_or_else(|| {
                        anyhow!("The --format option must be set when loading from stdin")
                    })?,
                    None,
                    graph,
                )
            } else {
                ThreadPoolBuilder::new()
                    .num_threads(max(1, available_parallelism()?.get() / 2))
                    .thread_name(|i| format!("Oxigraph bulk loader thread {i}"))
                    .build()?
                    .scope(|s| {
                        for file in file {
                            let store = store.clone();
                            s.spawn(move |_| {
                                let f = file.clone();
                                let start = Instant::now();
                                let mut loader = store.bulk_loader().on_progress(move |size| {
                                    let elapsed = start.elapsed();
                                    eprintln!(
                                        "{} triples loaded in {}s ({} t/s) from {}",
                                        size,
                                        elapsed.as_secs(),
                                        ((size as f64) / elapsed.as_secs_f64()).round(),
                                        f.display()
                                    )
                                });
                                if lenient {
                                    let f = file.clone();
                                    loader = loader.on_parse_error(move |e| {
                                        eprintln!("Parsing error on file {}: {}", f.display(), e);
                                        Ok(())
                                    })
                                }
                                let fp = match File::open(&file) {
                                    Ok(fp) => fp,
                                    Err(error) => {
                                        eprintln!(
                                            "Error while opening file {}: {}",
                                            file.display(),
                                            error
                                        );
                                        return;
                                    }
                                };
                                if let Err(error) = {
                                    if file.extension().map_or(false, |e| e == OsStr::new("gz")) {
                                        bulk_load(
                                            &loader,
                                            BufReader::new(MultiGzDecoder::new(fp)),
                                            format.unwrap_or_else(|| {
                                                GraphOrDatasetFormat::from_path(
                                                    &file.with_extension(""),
                                                )
                                                .unwrap()
                                            }),
                                            None,
                                            graph,
                                        )
                                    } else {
                                        bulk_load(
                                            &loader,
                                            BufReader::new(fp),
                                            format.unwrap_or_else(|| {
                                                GraphOrDatasetFormat::from_path(&file).unwrap()
                                            }),
                                            None,
                                            graph,
                                        )
                                    }
                                } {
                                    eprintln!(
                                        "Error while loading file {}: {}",
                                        file.display(),
                                        error
                                    )
                                    //TODO: hard fail
                                }
                            })
                        }
                    });
                store.flush()?;
                Ok(())
            }
        }
        Command::Dump {
            file,
            format,
            graph,
        } => {
            let store = Store::open_read_only(
                matches
                    .location
                    .ok_or_else(|| anyhow!("The --location argument is required"))?,
            )?;
            let format = if let Some(format) = format {
                GraphOrDatasetFormat::from_str(&format)?
            } else if let Some(file) = &file {
                GraphOrDatasetFormat::from_path(file)?
            } else {
                bail!("The --format option must be set when writing to stdout")
            };
            let graph = if let Some(graph) = &graph {
                Some(if graph.eq_ignore_ascii_case("default") {
                    GraphName::DefaultGraph
                } else {
                    NamedNodeRef::new(graph)
                        .with_context(|| format!("The target graph name {graph} is invalid"))?
                        .into()
                })
            } else {
                None
            };
            if let Some(file) = file {
                dump(
                    &store,
                    BufWriter::new(File::create(&file).map_err(|e| {
                        anyhow!("Error while opening file {}: {e}", file.display())
                    })?),
                    format,
                    graph,
                )
            } else {
                dump(&store, stdout().lock(), format, graph)
            }
        }
        Command::Query {
            query,
            query_file,
            query_base,
            results_file,
            results_format,
            explain,
            explain_file,
            stats,
        } => {
            let query = if let Some(query) = query {
                query
            } else if let Some(query_file) = query_file {
                fs::read_to_string(&query_file).with_context(|| {
                    format!("Not able to read query file {}", query_file.display())
                })?
            } else {
                // TODO: use io::read_to_string
                let mut query = String::new();
                stdin().lock().read_to_string(&mut query)?;
                query
            };
            let query = Query::parse(&query, query_base.as_deref())?;
            let store = Store::open_read_only(
                matches
                    .location
                    .ok_or_else(|| anyhow!("The --location argument is required"))?,
            )?;
            let (results, explanation) =
                store.explain_query_opt(query, QueryOptions::default(), stats)?;
            let print_result = (|| {
                match results? {
                    QueryResults::Solutions(solutions) => {
                        let format = if let Some(name) = results_format {
                            if let Some(format) = QueryResultsFormat::from_extension(&name) {
                                format
                            } else if let Some(format) = QueryResultsFormat::from_media_type(&name)
                            {
                                format
                            } else {
                                bail!("The file format '{name}' is unknown")
                            }
                        } else if let Some(results_file) = &results_file {
                            format_from_path(results_file, |ext| {
                                QueryResultsFormat::from_extension(ext)
                                    .ok_or_else(|| anyhow!("The file extension '{ext}' is unknown"))
                            })?
                        } else {
                            bail!("The --results-format option must be set when writing to stdout")
                        };
                        if let Some(results_file) = results_file {
                            let mut writer = QueryResultsSerializer::from_format(format)
                                .solutions_writer(
                                    BufWriter::new(File::create(results_file)?),
                                    solutions.variables().to_vec(),
                                )?;
                            for solution in solutions {
                                writer.write(&solution?)?;
                            }
                            writer.finish()?;
                        } else {
                            let stdout = stdout(); // Not needed in Rust 1.61
                            let mut writer = QueryResultsSerializer::from_format(format)
                                .solutions_writer(stdout.lock(), solutions.variables().to_vec())?;
                            for solution in solutions {
                                writer.write(&solution?)?;
                            }
                            #[allow(clippy::let_underscore_must_use)]
                            let _ = writer.finish()?;
                        }
                    }
                    QueryResults::Boolean(result) => {
                        let format = if let Some(name) = results_format {
                            if let Some(format) = QueryResultsFormat::from_extension(&name) {
                                format
                            } else if let Some(format) = QueryResultsFormat::from_media_type(&name)
                            {
                                format
                            } else {
                                bail!("The file format '{name}' is unknown")
                            }
                        } else if let Some(results_file) = &results_file {
                            format_from_path(results_file, |ext| {
                                QueryResultsFormat::from_extension(ext)
                                    .ok_or_else(|| anyhow!("The file extension '{ext}' is unknown"))
                            })?
                        } else {
                            bail!("The --results-format option must be set when writing to stdout")
                        };
                        if let Some(results_file) = results_file {
                            QueryResultsSerializer::from_format(format).write_boolean_result(
                                BufWriter::new(File::create(results_file)?),
                                result,
                            )?;
                        } else {
                            #[allow(clippy::let_underscore_must_use)]
                            let _ = QueryResultsSerializer::from_format(format)
                                .write_boolean_result(stdout().lock(), result)?;
                        }
                    }
                    QueryResults::Graph(triples) => {
                        let format = if let Some(name) = results_format {
                            if let Some(format) = GraphFormat::from_extension(&name) {
                                format
                            } else if let Some(format) = GraphFormat::from_media_type(&name) {
                                format
                            } else {
                                bail!("The file format '{name}' is unknown")
                            }
                        } else if let Some(results_file) = &results_file {
                            format_from_path(results_file, |ext| {
                                GraphFormat::from_extension(ext)
                                    .ok_or_else(|| anyhow!("The file extension '{ext}' is unknown"))
                            })?
                        } else {
                            bail!("The --results-format option must be set when writing to stdout")
                        };
                        if let Some(results_file) = results_file {
                            let mut writer = GraphSerializer::from_format(format)
                                .triple_writer(BufWriter::new(File::create(results_file)?))?;
                            for triple in triples {
                                writer.write(triple?.as_ref())?;
                            }
                            writer.finish()?;
                        } else {
                            let stdout = stdout(); // Not needed in Rust 1.61
                            let mut writer = GraphSerializer::from_format(format)
                                .triple_writer(stdout.lock())?;
                            for triple in triples {
                                writer.write(triple?.as_ref())?;
                            }
                            writer.finish()?;
                        }
                    }
                }
                Ok(())
            })();
            if let Some(explain_file) = explain_file {
                let mut file = BufWriter::new(File::create(&explain_file)?);
                match explain_file
                    .extension()
                    .and_then(OsStr::to_str) {
                    Some("json") => {
                        explanation.write_in_json(file)?;
                    },
                    Some("txt") => {
                        write!(file, "{:?}", explanation)?;
                    },
                    _ => bail!("The given explanation file {} must have an extension that is .json or .txt", explain_file.display())
                }
            } else if explain || stats {
                eprintln!("{:#?}", explanation);
            }
            print_result
        }
        Command::Update {
            update,
            update_file,
            update_base,
        } => {
            let update = if let Some(update) = update {
                update
            } else if let Some(update_file) = update_file {
                fs::read_to_string(&update_file).with_context(|| {
                    format!("Not able to read update file {}", update_file.display())
                })?
            } else {
                // TODO: use io::read_to_string
                let mut update = String::new();
                stdin().lock().read_to_string(&mut update)?;
                update
            };
            let update = Update::parse(&update, update_base.as_deref())?;
            let store = Store::open(
                matches
                    .location
                    .ok_or_else(|| anyhow!("The --location argument is required"))?,
            )?;
            store.update(update)?;
            store.flush()?;
            Ok(())
        }
        Command::Optimize {} => {
            let store = Store::open(
                matches
                    .location
                    .ok_or_else(|| anyhow!("The --location argument is required"))?,
            )?;
            store.optimize()?;
            Ok(())
        }
    }
}

fn bulk_load(
    loader: &BulkLoader,
    reader: impl BufRead,
    format: GraphOrDatasetFormat,
    base_iri: Option<&str>,
    to_graph_name: Option<NamedNodeRef<'_>>,
) -> anyhow::Result<()> {
    match format {
        GraphOrDatasetFormat::Graph(format) => loader.load_graph(
            reader,
            format,
            to_graph_name.map_or(GraphNameRef::DefaultGraph, GraphNameRef::from),
            base_iri,
        )?,
        GraphOrDatasetFormat::Dataset(format) => {
            if to_graph_name.is_some() {
                bail!("The --graph option is not allowed when loading a dataset format like NQuads or TriG");
            }
            loader.load_dataset(reader, format, base_iri)?
        }
    }
    Ok(())
}

fn dump(
    store: &Store,
    writer: impl Write,
    format: GraphOrDatasetFormat,
    to_graph_name: Option<GraphName>,
) -> anyhow::Result<()> {
    match format {
        GraphOrDatasetFormat::Graph(format) => store.dump_graph(
            writer,
            format,
            &to_graph_name.ok_or_else(|| anyhow!("The --graph option is required when writing a graph format like NTriples, Turtle or RDF/XML"))?,
        )?,
        GraphOrDatasetFormat::Dataset(format) => {
            if to_graph_name.is_some() {
                bail!("The --graph option is not allowed when writing a dataset format like NQuads or TriG");
            }
            store.dump_dataset(writer, format)?
        }
    }
    Ok(())
}

#[derive(Copy, Clone)]
enum GraphOrDatasetFormat {
    Graph(GraphFormat),
    Dataset(DatasetFormat),
}

impl GraphOrDatasetFormat {
    fn from_path(path: &Path) -> anyhow::Result<Self> {
        format_from_path(path, Self::from_extension)
    }

    fn from_extension(name: &str) -> anyhow::Result<Self> {
        Ok(match (GraphFormat::from_extension(name), DatasetFormat::from_extension(name)) {
            (Some(g), Some(d)) => bail!("The file extension '{name}' can be resolved to both '{}' and '{}', not sure what to pick", g.file_extension(), d.file_extension()),
            (Some(g), None) => Self::Graph(g),
            (None, Some(d)) => Self::Dataset(d),
            (None, None) =>
            bail!("The file extension '{name}' is unknown")
        })
    }

    fn from_media_type(name: &str) -> anyhow::Result<Self> {
        Ok(
            match (
                GraphFormat::from_media_type(name),
                DatasetFormat::from_media_type(name),
            ) {
                (Some(g), Some(d)) => bail!(
                "The media type '{name}' can be resolved to both '{}' and '{}', not sure what to pick",
                g.file_extension(),
                d.file_extension()
            ),
                (Some(g), None) => Self::Graph(g),
                (None, Some(d)) => Self::Dataset(d),
                (None, None) => bail!("The media type '{name}' is unknown"),
            },
        )
    }
}

fn format_from_path<T>(
    path: &Path,
    from_extension: impl FnOnce(&str) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    if let Some(ext) = path.extension().and_then(OsStr::to_str) {
        from_extension(ext).map_err(|e| {
            e.context(format!(
                "Not able to guess the file format from file name extension '{ext}'"
            ))
        })
    } else {
        bail!(
            "The path {} has no extension to guess a file format from",
            path.display()
        )
    }
}

impl FromStr for GraphOrDatasetFormat {
    type Err = Error;

    fn from_str(name: &str) -> anyhow::Result<Self> {
        if let Ok(t) = Self::from_extension(name) {
            return Ok(t);
        }
        if let Ok(t) = Self::from_media_type(name) {
            return Ok(t);
        }
        bail!("The file format '{name}' is unknown")
    }
}

fn serve(store: Store, bind: String, read_only: bool, cors: bool) -> anyhow::Result<()> {
    let mut server = if cors {
        Server::new(cors_middleware(move |request| {
            handle_request(request, store.clone(), read_only)
                .unwrap_or_else(|(status, message)| error(status, message))
        }))
    } else {
        Server::new(move |request| {
            handle_request(request, store.clone(), read_only)
                .unwrap_or_else(|(status, message)| error(status, message))
        })
    };
    server.set_global_timeout(HTTP_TIMEOUT);
    server.set_server_name(concat!("Oxigraph/", env!("CARGO_PKG_VERSION")))?;
    eprintln!("Listening for requests at http://{}", &bind);
    server.listen(bind)?;
    Ok(())
}

fn cors_middleware(
    on_request: impl Fn(&mut Request) -> Response + Send + Sync + 'static,
) -> impl Fn(&mut Request) -> Response + Send + Sync + 'static {
    let origin = HeaderName::from_str("Origin").unwrap();
    let access_control_allow_origin = HeaderName::from_str("Access-Control-Allow-Origin").unwrap();
    let access_control_request_method =
        HeaderName::from_str("Access-Control-Request-Method").unwrap();
    let access_control_allow_method = HeaderName::from_str("Access-Control-Allow-Methods").unwrap();
    let access_control_request_headers =
        HeaderName::from_str("Access-Control-Request-Headers").unwrap();
    let access_control_allow_headers =
        HeaderName::from_str("Access-Control-Allow-Headers").unwrap();
    let star = HeaderValue::from_str("*").unwrap();
    move |request| {
        if *request.method() == Method::OPTIONS {
            let mut response = Response::builder(Status::NO_CONTENT);
            if request.header(&origin).is_some() {
                response
                    .headers_mut()
                    .append(access_control_allow_origin.clone(), star.clone());
            }
            if let Some(method) = request.header(&access_control_request_method) {
                response
                    .headers_mut()
                    .append(access_control_allow_method.clone(), method.clone());
            }
            if let Some(headers) = request.header(&access_control_request_headers) {
                response
                    .headers_mut()
                    .append(access_control_allow_headers.clone(), headers.clone());
            }
            response.build()
        } else {
            let mut response = on_request(request);
            if request.header(&origin).is_some() {
                response
                    .headers_mut()
                    .append(access_control_allow_origin.clone(), star.clone());
            }
            response
        }
    }
}

type HttpError = (Status, String);

fn handle_request(
    request: &mut Request,
    store: Store,
    read_only: bool,
) -> Result<Response, HttpError> {
    match (request.url().path(), request.method().as_ref()) {
        ("/", "HEAD") => Ok(Response::builder(Status::OK)
            .with_header(HeaderName::CONTENT_TYPE, "text_html")
            .unwrap()
            .build()),
        ("/", "GET") => Ok(Response::builder(Status::OK)
            .with_header(HeaderName::CONTENT_TYPE, "text_html")
            .unwrap()
            .with_body(HTML_ROOT_PAGE)),
        ("/logo.svg", "HEAD") => Ok(Response::builder(Status::OK)
            .with_header(HeaderName::CONTENT_TYPE, "image/svg+xml")
            .unwrap()
            .build()),
        ("/logo.svg", "GET") => Ok(Response::builder(Status::OK)
            .with_header(HeaderName::CONTENT_TYPE, "image/svg+xml")
            .unwrap()
            .with_body(LOGO)),
        ("/query", "GET") => {
            configure_and_evaluate_sparql_query(&store, &[url_query(request)], None, request)
        }
        ("/query", "POST") => {
            let content_type =
                content_type(request).ok_or_else(|| bad_request("No Content-Type given"))?;
            if content_type == "application/sparql-query" {
                let mut buffer = String::new();
                request
                    .body_mut()
                    .take(MAX_SPARQL_BODY_SIZE)
                    .read_to_string(&mut buffer)
                    .map_err(bad_request)?;
                configure_and_evaluate_sparql_query(
                    &store,
                    &[url_query(request)],
                    Some(buffer),
                    request,
                )
            } else if content_type == "application/x-www-form-urlencoded" {
                let mut buffer = Vec::new();
                request
                    .body_mut()
                    .take(MAX_SPARQL_BODY_SIZE)
                    .read_to_end(&mut buffer)
                    .map_err(bad_request)?;
                configure_and_evaluate_sparql_query(
                    &store,
                    &[url_query(request), &buffer],
                    None,
                    request,
                )
            } else {
                Err(unsupported_media_type(&content_type))
            }
        }
        ("/update", "POST") => {
            if read_only {
                return Err(the_server_is_read_only());
            }
            let content_type =
                content_type(request).ok_or_else(|| bad_request("No Content-Type given"))?;
            if content_type == "application/sparql-update" {
                let mut buffer = String::new();
                request
                    .body_mut()
                    .take(MAX_SPARQL_BODY_SIZE)
                    .read_to_string(&mut buffer)
                    .map_err(bad_request)?;
                configure_and_evaluate_sparql_update(
                    &store,
                    &[url_query(request)],
                    Some(buffer),
                    request,
                )
            } else if content_type == "application/x-www-form-urlencoded" {
                let mut buffer = Vec::new();
                request
                    .body_mut()
                    .take(MAX_SPARQL_BODY_SIZE)
                    .read_to_end(&mut buffer)
                    .map_err(bad_request)?;
                configure_and_evaluate_sparql_update(
                    &store,
                    &[url_query(request), &buffer],
                    None,
                    request,
                )
            } else {
                Err(unsupported_media_type(&content_type))
            }
        }
        (path, "GET") if path.starts_with("/store") => {
            if let Some(target) = store_target(request)? {
                assert_that_graph_exists(&store, &target)?;
                let format = graph_content_negotiation(request)?;
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
                let format = dataset_content_negotiation(request)?;
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
            if read_only {
                return Err(the_server_is_read_only());
            }
            let content_type =
                content_type(request).ok_or_else(|| bad_request("No Content-Type given"))?;
            if let Some(target) = store_target(request)? {
                let format = GraphFormat::from_media_type(&content_type)
                    .ok_or_else(|| unsupported_media_type(&content_type))?;
                let new = !match &target {
                    NamedGraphName::NamedNode(target) => {
                        if store
                            .contains_named_graph(target)
                            .map_err(internal_server_error)?
                        {
                            store.clear_graph(target).map_err(internal_server_error)?;
                            true
                        } else {
                            store
                                .insert_named_graph(target)
                                .map_err(internal_server_error)?;
                            false
                        }
                    }
                    NamedGraphName::DefaultGraph => {
                        store
                            .clear_graph(GraphNameRef::DefaultGraph)
                            .map_err(internal_server_error)?;
                        true
                    }
                };
                web_load_graph(&store, request, format, GraphName::from(target).as_ref())?;
                Ok(Response::builder(if new {
                    Status::CREATED
                } else {
                    Status::NO_CONTENT
                })
                .build())
            } else {
                let format = DatasetFormat::from_media_type(&content_type)
                    .ok_or_else(|| unsupported_media_type(&content_type))?;
                store.clear().map_err(internal_server_error)?;
                web_load_dataset(&store, request, format)?;
                Ok(Response::builder(Status::NO_CONTENT).build())
            }
        }
        (path, "DELETE") if path.starts_with("/store") => {
            if read_only {
                return Err(the_server_is_read_only());
            }
            if let Some(target) = store_target(request)? {
                match target {
                    NamedGraphName::DefaultGraph => store
                        .clear_graph(GraphNameRef::DefaultGraph)
                        .map_err(internal_server_error)?,
                    NamedGraphName::NamedNode(target) => {
                        if store
                            .contains_named_graph(&target)
                            .map_err(internal_server_error)?
                        {
                            store
                                .remove_named_graph(&target)
                                .map_err(internal_server_error)?;
                        } else {
                            return Err((
                                Status::NOT_FOUND,
                                format!("The graph {target} does not exists"),
                            ));
                        }
                    }
                }
            } else {
                store.clear().map_err(internal_server_error)?;
            }
            Ok(Response::builder(Status::NO_CONTENT).build())
        }
        (path, "POST") if path.starts_with("/store") => {
            if read_only {
                return Err(the_server_is_read_only());
            }
            let content_type =
                content_type(request).ok_or_else(|| bad_request("No Content-Type given"))?;
            if let Some(target) = store_target(request)? {
                let format = GraphFormat::from_media_type(&content_type)
                    .ok_or_else(|| unsupported_media_type(&content_type))?;
                let new = assert_that_graph_exists(&store, &target).is_ok();
                web_load_graph(&store, request, format, GraphName::from(target).as_ref())?;
                Ok(Response::builder(if new {
                    Status::CREATED
                } else {
                    Status::NO_CONTENT
                })
                .build())
            } else {
                match GraphOrDatasetFormat::from_media_type(&content_type)
                    .map_err(|_| unsupported_media_type(&content_type))?
                {
                    GraphOrDatasetFormat::Graph(format) => {
                        let graph =
                            resolve_with_base(request, &format!("/store/{:x}", random::<u128>()))?;
                        web_load_graph(&store, request, format, graph.as_ref().into())?;
                        Ok(Response::builder(Status::CREATED)
                            .with_header(HeaderName::LOCATION, graph.into_string())
                            .unwrap()
                            .build())
                    }
                    GraphOrDatasetFormat::Dataset(format) => {
                        web_load_dataset(&store, request, format)?;
                        Ok(Response::builder(Status::NO_CONTENT).build())
                    }
                }
            }
        }
        (path, "HEAD") if path.starts_with("/store") => {
            if let Some(target) = store_target(request)? {
                assert_that_graph_exists(&store, &target)?;
            }
            Ok(Response::builder(Status::OK).build())
        }
        _ => Err((
            Status::NOT_FOUND,
            format!(
                "{} {} is not supported by this server",
                request.method(),
                request.url().path()
            ),
        )),
    }
}

fn base_url(request: &Request) -> String {
    let mut url = request.url().clone();
    url.set_query(None);
    url.set_fragment(None);
    url.into()
}

fn resolve_with_base(request: &Request, url: &str) -> Result<NamedNode, HttpError> {
    Ok(NamedNode::new_unchecked(
        Iri::parse(base_url(request))
            .map_err(bad_request)?
            .resolve(url)
            .map_err(bad_request)?
            .into_inner(),
    ))
}

fn url_query(request: &Request) -> &[u8] {
    request.url().query().unwrap_or("").as_bytes()
}

fn url_query_parameter<'a>(request: &'a Request, param: &str) -> Option<Cow<'a, str>> {
    request
        .url()
        .query_pairs()
        .find(|(k, _)| k == param)
        .map(|(_, v)| v)
}

fn configure_and_evaluate_sparql_query(
    store: &Store,
    encoded: &[&[u8]],
    mut query: Option<String>,
    request: &Request,
) -> Result<Response, HttpError> {
    let mut default_graph_uris = Vec::new();
    let mut named_graph_uris = Vec::new();
    let mut use_default_graph_as_union = false;
    for encoded in encoded {
        for (k, v) in form_urlencoded::parse(encoded) {
            match k.as_ref() {
                "query" => {
                    if query.is_some() {
                        return Err(bad_request("Multiple query parameters provided"));
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
    let query = query.ok_or_else(|| bad_request("You should set the 'query' parameter"))?;
    evaluate_sparql_query(
        store,
        &query,
        use_default_graph_as_union,
        default_graph_uris,
        named_graph_uris,
        request,
    )
}

fn evaluate_sparql_query(
    store: &Store,
    query: &str,
    use_default_graph_as_union: bool,
    default_graph_uris: Vec<String>,
    named_graph_uris: Vec<String>,
    request: &Request,
) -> Result<Response, HttpError> {
    let mut query = Query::parse(query, Some(&base_url(request))).map_err(bad_request)?;

    if use_default_graph_as_union {
        if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
            return Err(bad_request(
                "default-graph-uri or named-graph-uri and union-default-graph should not be set at the same time"
            ));
        }
        query.dataset_mut().set_default_graph_as_union()
    } else if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
        query.dataset_mut().set_default_graph(
            default_graph_uris
                .into_iter()
                .map(|e| Ok(NamedNode::new(e)?.into()))
                .collect::<Result<Vec<GraphName>, IriParseError>>()
                .map_err(bad_request)?,
        );
        query.dataset_mut().set_available_named_graphs(
            named_graph_uris
                .into_iter()
                .map(|e| Ok(NamedNode::new(e)?.into()))
                .collect::<Result<Vec<NamedOrBlankNode>, IriParseError>>()
                .map_err(bad_request)?,
        );
    }

    let results = store.query(query).map_err(internal_server_error)?;
    match results {
        QueryResults::Solutions(solutions) => {
            let format = query_results_content_negotiation(request)?;
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
            let format = query_results_content_negotiation(request)?;
            let mut body = Vec::new();
            QueryResultsSerializer::from_format(format)
                .write_boolean_result(&mut body, result)
                .map_err(internal_server_error)?;
            Ok(Response::builder(Status::OK)
                .with_header(HeaderName::CONTENT_TYPE, format.media_type())
                .unwrap()
                .with_body(body))
        }
        QueryResults::Graph(triples) => {
            let format = graph_content_negotiation(request)?;
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
    store: &Store,
    encoded: &[&[u8]],
    mut update: Option<String>,
    request: &Request,
) -> Result<Response, HttpError> {
    let mut use_default_graph_as_union = false;
    let mut default_graph_uris = Vec::new();
    let mut named_graph_uris = Vec::new();
    for encoded in encoded {
        for (k, v) in form_urlencoded::parse(encoded) {
            match k.as_ref() {
                "update" => {
                    if update.is_some() {
                        return Err(bad_request("Multiple update parameters provided"));
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
    let update = update.ok_or_else(|| bad_request("You should set the 'update' parameter"))?;
    evaluate_sparql_update(
        store,
        &update,
        use_default_graph_as_union,
        default_graph_uris,
        named_graph_uris,
        request,
    )
}

fn evaluate_sparql_update(
    store: &Store,
    update: &str,
    use_default_graph_as_union: bool,
    default_graph_uris: Vec<String>,
    named_graph_uris: Vec<String>,
    request: &Request,
) -> Result<Response, HttpError> {
    let mut update =
        Update::parse(update, Some(base_url(request).as_str())).map_err(bad_request)?;

    if use_default_graph_as_union {
        if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
            return Err(bad_request(
                "using-graph-uri or using-named-graph-uri and using-union-graph should not be set at the same time"
            ));
        }
        for using in update.using_datasets_mut() {
            if !using.is_default_dataset() {
                return Err(bad_request(
                    "using-union-graph must not be used with a SPARQL UPDATE containing USING",
                ));
            }
            using.set_default_graph_as_union();
        }
    } else if !default_graph_uris.is_empty() || !named_graph_uris.is_empty() {
        let default_graph_uris = default_graph_uris
            .into_iter()
            .map(|e| Ok(NamedNode::new(e)?.into()))
            .collect::<Result<Vec<GraphName>, IriParseError>>()
            .map_err(bad_request)?;
        let named_graph_uris = named_graph_uris
            .into_iter()
            .map(|e| Ok(NamedNode::new(e)?.into()))
            .collect::<Result<Vec<NamedOrBlankNode>, IriParseError>>()
            .map_err(bad_request)?;
        for using in update.using_datasets_mut() {
            if !using.is_default_dataset() {
                return Err(bad_request(
                        "using-graph-uri and using-named-graph-uri must not be used with a SPARQL UPDATE containing USING",
                    ));
            }
            using.set_default_graph(default_graph_uris.clone());
            using.set_available_named_graphs(named_graph_uris.clone());
        }
    }
    store.update(update).map_err(internal_server_error)?;
    Ok(Response::builder(Status::NO_CONTENT).build())
}

fn store_target(request: &Request) -> Result<Option<NamedGraphName>, HttpError> {
    if request.url().path() == "/store" {
        let mut graph = None;
        let mut default = false;
        for (k, v) in request.url().query_pairs() {
            match k.as_ref() {
                "graph" => graph = Some(v.into_owned()),
                "default" => default = true,
                _ => continue,
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

fn assert_that_graph_exists(store: &Store, target: &NamedGraphName) -> Result<(), HttpError> {
    if match target {
        NamedGraphName::DefaultGraph => true,
        NamedGraphName::NamedNode(target) => store
            .contains_named_graph(target)
            .map_err(internal_server_error)?,
    } {
        Ok(())
    } else {
        Err((
            Status::NOT_FOUND,
            format!(
                "The graph {} does not exists",
                GraphName::from(target.clone())
            ),
        ))
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

fn graph_content_negotiation(request: &Request) -> Result<GraphFormat, HttpError> {
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

fn dataset_content_negotiation(request: &Request) -> Result<DatasetFormat, HttpError> {
    content_negotiation(
        request,
        &[
            DatasetFormat::NQuads.media_type(),
            DatasetFormat::TriG.media_type(),
        ],
        DatasetFormat::from_media_type,
    )
}

fn query_results_content_negotiation(request: &Request) -> Result<QueryResultsFormat, HttpError> {
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
) -> Result<F, HttpError> {
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
    let mut result_score = 0_f32;

    for possible in header.split(',') {
        let (possible, parameters) = possible.split_once(';').unwrap_or((possible, ""));
        let (possible_base, possible_sub) = possible
            .split_once('/')
            .ok_or_else(|| bad_request(format!("Invalid media type: '{possible}'")))?;
        let possible_base = possible_base.trim();
        let possible_sub = possible_sub.trim();

        let mut score = 1.;
        for parameter in parameters.split(';') {
            let parameter = parameter.trim();
            if let Some(s) = parameter.strip_prefix("q=") {
                score = f32::from_str(s.trim())
                    .map_err(|_| bad_request(format!("Invalid Accept media type score: {s}")))?
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
                    internal_server_error(format!("Invalid media type: '{possible}'"))
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
        (
            Status::NOT_ACCEPTABLE,
            format!("The available Content-Types are {}", supported.join(", "),),
        )
    })?;

    parse(result).ok_or_else(|| internal_server_error("Unknown media type"))
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

fn web_load_graph(
    store: &Store,
    request: &mut Request,
    format: GraphFormat,
    to_graph_name: GraphNameRef<'_>,
) -> Result<(), HttpError> {
    if url_query_parameter(request, "no_transaction").is_some() {
        web_bulk_loader(store, request).load_graph(
            BufReader::new(request.body_mut()),
            format,
            to_graph_name,
            None,
        )
    } else {
        store.load_graph(
            BufReader::new(request.body_mut()),
            format,
            to_graph_name,
            None,
        )
    }
    .map_err(loader_to_http_error)
}

fn web_load_dataset(
    store: &Store,
    request: &mut Request,
    format: DatasetFormat,
) -> Result<(), HttpError> {
    if url_query_parameter(request, "no_transaction").is_some() {
        web_bulk_loader(store, request).load_dataset(
            BufReader::new(request.body_mut()),
            format,
            None,
        )
    } else {
        store.load_dataset(BufReader::new(request.body_mut()), format, None)
    }
    .map_err(loader_to_http_error)
}

fn web_bulk_loader(store: &Store, request: &Request) -> BulkLoader {
    let start = Instant::now();
    let mut loader = store.bulk_loader().on_progress(move |size| {
        let elapsed = start.elapsed();
        eprintln!(
            "{} triples loaded in {}s ({} t/s)",
            size,
            elapsed.as_secs(),
            ((size as f64) / elapsed.as_secs_f64()).round()
        )
    });
    if url_query_parameter(request, "lenient").is_some() {
        loader = loader.on_parse_error(move |e| {
            eprintln!("Parsing error: {e}");
            Ok(())
        })
    }
    loader
}

fn error(status: Status, message: impl fmt::Display) -> Response {
    Response::builder(status)
        .with_header(HeaderName::CONTENT_TYPE, "text/plain; charset=utf-8")
        .unwrap()
        .with_body(message.to_string())
}

fn bad_request(message: impl fmt::Display) -> HttpError {
    (Status::BAD_REQUEST, message.to_string())
}

fn the_server_is_read_only() -> HttpError {
    (Status::FORBIDDEN, "The server is read-only".into())
}

fn unsupported_media_type(content_type: &str) -> HttpError {
    (
        Status::UNSUPPORTED_MEDIA_TYPE,
        format!("No supported content Content-Type given: {content_type}"),
    )
}

fn internal_server_error(message: impl fmt::Display) -> HttpError {
    eprintln!("Internal server error: {message}");
    (Status::INTERNAL_SERVER_ERROR, message.to_string())
}

fn loader_to_http_error(e: LoaderError) -> HttpError {
    match e {
        LoaderError::Parsing(e) => bad_request(e),
        LoaderError::Storage(e) => internal_server_error(e),
    }
}

/// Hacky tool to allow implementing read on top of a write loop
struct ReadForWrite<O, U: (Fn(O) -> io::Result<Option<O>>)> {
    buffer: Rc<RefCell<Vec<u8>>>,
    position: usize,
    add_more_data: U,
    state: Option<O>,
}

impl<O: 'static, U: (Fn(O) -> io::Result<Option<O>>) + 'static> ReadForWrite<O, U> {
    fn build_response(
        initial_state_builder: impl FnOnce(ReadForWriteWriter) -> io::Result<O>,
        add_more_data: U,
        content_type: &'static str,
    ) -> Result<Response, HttpError> {
        let buffer = Rc::new(RefCell::new(Vec::new()));
        let state = initial_state_builder(ReadForWriteWriter {
            buffer: Rc::clone(&buffer),
        })
        .map_err(internal_server_error)?;
        Ok(Response::builder(Status::OK)
            .with_header(HeaderName::CONTENT_TYPE, content_type)
            .unwrap()
            .with_body(Body::from_read(Self {
                buffer,
                position: 0,
                add_more_data,
                state: Some(state),
            })))
    }
}

impl<O, U: (Fn(O) -> io::Result<Option<O>>)> Read for ReadForWrite<O, U> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        while self.position == self.buffer.borrow().len() {
            // We read more data
            if let Some(state) = self.state.take() {
                self.buffer.borrow_mut().clear();
                self.position = 0;
                self.state = match (self.add_more_data)(state) {
                    Ok(state) => state,
                    Err(e) => {
                        eprintln!("Internal server error while streaming results: {e}");
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
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.buffer.borrow_mut().write_all(buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use assert_cmd::Command;
    use assert_fs::{prelude::*, NamedTempFile, TempDir};
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use oxhttp::model::Method;
    use predicates::prelude::*;
    use std::fs::remove_dir_all;

    fn cli_command() -> Result<Command> {
        Ok(Command::from_std(
            escargot::CargoBuild::new()
                .bin(env!("CARGO_PKG_NAME"))
                .manifest_path(format!("{}/Cargo.toml", env!("CARGO_MANIFEST_DIR")))
                .run()?
                .command(),
        ))
    }

    fn initialized_cli_store(data: &'static str) -> Result<TempDir> {
        let store_dir = TempDir::new()?;
        cli_command()?
            .arg("load")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--format")
            .arg("trig")
            .write_stdin(data)
            .assert()
            .success();
        Ok(store_dir)
    }

    fn assert_cli_state(store_dir: &TempDir, data: &'static str) -> Result<()> {
        cli_command()?
            .arg("dump")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--format")
            .arg("nq")
            .assert()
            .stdout(data)
            .success();
        Ok(())
    }

    #[test]
    fn cli_help() -> Result<()> {
        cli_command()?
            .assert()
            .failure()
            .stdout("")
            .stderr(predicate::str::starts_with("Oxigraph"));
        Ok(())
    }

    #[test]
    fn cli_load_optimize_and_dump_graph() -> Result<()> {
        let store_dir = TempDir::new()?;
        let input_file = NamedTempFile::new("input.nt")?;
        input_file
            .write_str("<http://example.com/s> <http://example.com/p> <http://example.com/o> .")?;
        cli_command()?
            .arg("--location")
            .arg(store_dir.path())
            .arg("load")
            .arg("--file")
            .arg(input_file.path())
            .assert()
            .success();

        cli_command()?
            .arg("optimize")
            .arg("--location")
            .arg(store_dir.path())
            .assert()
            .success();

        let output_file = NamedTempFile::new("output.nt")?;
        cli_command()?
            .arg("dump")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--file")
            .arg(output_file.path())
            .arg("--graph")
            .arg("default")
            .assert()
            .success();
        output_file
            .assert("<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n");
        Ok(())
    }

    #[test]
    fn cli_load_and_dump_dataset() -> Result<()> {
        let store_dir = TempDir::new()?;
        let input_file = NamedTempFile::new("input.nq")?;
        input_file
            .write_str("<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .")?;
        cli_command()?
            .arg("load")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--file")
            .arg(input_file.path())
            .assert()
            .success();

        let output_file = NamedTempFile::new("output.nq")?;
        cli_command()?
            .arg("dump")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--file")
            .arg(output_file.path())
            .assert()
            .success();
        output_file
            .assert("<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/g> .\n");
        Ok(())
    }

    #[test]
    fn cli_load_gzip_dataset() -> Result<()> {
        let store_dir = TempDir::new()?;
        let file = NamedTempFile::new("sample.nq.gz")?;
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder
            .write_all(b"<http://example.com/s> <http://example.com/p> <http://example.com/o> .")?;
        file.write_binary(&encoder.finish()?)?;
        cli_command()?
            .arg("load")
            .arg("-l")
            .arg(store_dir.path())
            .arg("-f")
            .arg(file.path())
            .assert()
            .success();

        cli_command()?
            .arg("dump")
            .arg("-l")
            .arg(store_dir.path())
            .arg("--format")
            .arg("nq")
            .assert()
            .success()
            .stdout("<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n");
        Ok(())
    }

    #[test]
    fn cli_load_and_dump_named_graph() -> Result<()> {
        let store_dir = TempDir::new()?;
        let input_file = NamedTempFile::new("input.nt")?;
        input_file.write_str(
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n",
        )?;
        cli_command()?
            .arg("load")
            .arg("-l")
            .arg(store_dir.path())
            .arg("-f")
            .arg(input_file.path())
            .arg("--graph")
            .arg("http://example.com/g")
            .assert()
            .success();

        let output_file = NamedTempFile::new("output.nt")?;
        cli_command()?
            .arg("dump")
            .arg("-l")
            .arg(store_dir.path())
            .arg("-f")
            .arg(output_file.path())
            .arg("--graph")
            .arg("http://example.com/g")
            .assert()
            .success();
        output_file
            .assert("<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n");
        Ok(())
    }

    #[test]
    fn cli_load_and_dump_with_format() -> Result<()> {
        let store_dir = TempDir::new()?;
        let input_file = NamedTempFile::new("input")?;
        input_file
            .write_str("<http://example.com/s> <http://example.com/p> <http://example.com/o> .")?;
        cli_command()?
            .arg("load")
            .arg("-l")
            .arg(store_dir.path())
            .arg("-f")
            .arg(input_file.path())
            .arg("--format")
            .arg("nt")
            .assert()
            .success();

        let output_file = NamedTempFile::new("output")?;
        cli_command()?
            .arg("dump")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--file")
            .arg(output_file.path())
            .arg("--graph")
            .arg("default")
            .arg("--format")
            .arg("nt")
            .assert()
            .success();
        output_file
            .assert("<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n");
        Ok(())
    }

    #[test]
    fn cli_load_from_stdin_and_dump_to_stdout() -> Result<()> {
        let store_dir = TempDir::new()?;
        cli_command()?
            .arg("load")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--format")
            .arg("nq")
            .write_stdin("<http://example.com/s> <http://example.com/p> <http://example.com/o> .")
            .assert()
            .success();

        cli_command()?
            .arg("dump")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--format")
            .arg("nq")
            .assert()
            .success()
            .stdout("<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n");
        Ok(())
    }

    #[test]
    fn cli_backup() -> Result<()> {
        let store_dir = initialized_cli_store(
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .",
        )?;

        let backup_dir = TempDir::new()?;
        remove_dir_all(backup_dir.path())?; // The directory should not exist yet
        cli_command()?
            .arg("backup")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--destination")
            .arg(backup_dir.path())
            .assert()
            .success();

        assert_cli_state(
            &store_dir,
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n",
        )
    }

    #[test]
    fn cli_ask_query_inline() -> Result<()> {
        let store_dir = initialized_cli_store(
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .",
        )?;
        cli_command()?
            .arg("query")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--query")
            .arg("ASK { <s> <p> <o> }")
            .arg("--query-base")
            .arg("http://example.com/")
            .arg("--results-format")
            .arg("csv")
            .assert()
            .stdout("true")
            .success();
        Ok(())
    }

    #[test]
    fn cli_construct_query_stdin() -> Result<()> {
        let store_dir = initialized_cli_store(
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .",
        )?;
        cli_command()?
            .arg("query")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--query-base")
            .arg("http://example.com/")
            .arg("--results-format")
            .arg("nt")
            .write_stdin("CONSTRUCT { ?s ?p ?o } WHERE { ?s ?p ?o }")
            .assert()
            .stdout("<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n")
            .success();
        Ok(())
    }

    #[test]
    fn cli_select_query_file() -> Result<()> {
        let store_dir = initialized_cli_store(
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .",
        )?;
        let input_file = NamedTempFile::new("input.rq")?;
        input_file.write_str("SELECT ?s WHERE { ?s ?p ?o }")?;
        let output_file = NamedTempFile::new("output.tsv")?;
        cli_command()?
            .arg("query")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--query-file")
            .arg(input_file.path())
            .arg("--results-file")
            .arg(output_file.path())
            .assert()
            .success();
        output_file.assert("?s\n<http://example.com/s>\n");
        Ok(())
    }

    #[test]
    fn cli_ask_update_inline() -> Result<()> {
        let store_dir = TempDir::new()?;
        cli_command()?
            .arg("update")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--update")
            .arg("INSERT DATA { <s> <p> <o> }")
            .arg("--update-base")
            .arg("http://example.com/")
            .assert()
            .success();
        assert_cli_state(
            &store_dir,
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n",
        )
    }

    #[test]
    fn cli_construct_update_stdin() -> Result<()> {
        let store_dir = TempDir::new()?;
        cli_command()?
            .arg("update")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--update-base")
            .arg("http://example.com/")
            .write_stdin("INSERT DATA { <s> <p> <o> }")
            .assert()
            .success();
        assert_cli_state(
            &store_dir,
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n",
        )
    }

    #[test]
    fn cli_update_file() -> Result<()> {
        let store_dir = TempDir::new()?;
        let input_file = NamedTempFile::new("input.rq")?;
        input_file.write_str(
            "INSERT DATA { <http://example.com/s> <http://example.com/p> <http://example.com/o> }",
        )?;
        cli_command()?
            .arg("update")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--update-file")
            .arg(input_file.path())
            .assert()
            .success();
        assert_cli_state(
            &store_dir,
            "<http://example.com/s> <http://example.com/p> <http://example.com/o> .\n",
        )
    }

    #[test]
    fn get_ui() -> Result<()> {
        ServerTest::new()?.test_status(
            Request::builder(Method::GET, "http://localhost/".parse()?).build(),
            Status::OK,
        )
    }

    #[test]
    fn post_dataset_file() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/store".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/trig")?
            .with_body("<http://example.com> <http://example.com> <http://example.com> .");
        ServerTest::new()?.test_status(request, Status::NO_CONTENT)
    }

    #[test]
    fn post_wrong_file() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/store".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/trig")?
            .with_body("<http://example.com>");
        ServerTest::new()?.test_status(request, Status::BAD_REQUEST)
    }

    #[test]
    fn post_unsupported_file() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/store".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/foo")?
            .build();
        ServerTest::new()?.test_status(request, Status::UNSUPPORTED_MEDIA_TYPE)
    }

    #[test]
    fn get_query() -> Result<()> {
        let server = ServerTest::new()?;

        let request = Request::builder(Method::POST, "http://localhost/store".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/trig")?
            .with_body("<http://example.com> <http://example.com> <http://example.com> .");
        server.test_status(request, Status::NO_CONTENT)?;

        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()?,
        )
        .with_header(HeaderName::ACCEPT, "text/csv")?
        .build();
        server.test_body(
            request,
            "s,p,o\r\nhttp://example.com,http://example.com,http://example.com\r\n",
        )
    }

    #[test]
    fn get_query_accept_star() -> Result<()> {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()?,
        )
        .with_header(HeaderName::ACCEPT, "*/*")?
        .build();
        ServerTest::new()?.test_body(
            request,
            "{\"head\":{\"vars\":[\"s\",\"p\",\"o\"]},\"results\":{\"bindings\":[]}}",
        )
    }

    #[test]
    fn get_query_accept_good() -> Result<()> {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()?,
        )
        .with_header(
            HeaderName::ACCEPT,
            "application/sparql-results+json;charset=utf-8",
        )?
        .build();
        ServerTest::new()?.test_body(
            request,
            "{\"head\":{\"vars\":[\"s\",\"p\",\"o\"]},\"results\":{\"bindings\":[]}}",
        )
    }

    #[test]
    fn get_query_accept_bad() -> Result<()> {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20*%20WHERE%20{%20?s%20?p%20?o%20}".parse()?,
        )
        .with_header(HeaderName::ACCEPT, "application/foo")?
        .build();
        ServerTest::new()?.test_status(request, Status::NOT_ACCEPTABLE)
    }

    #[test]
    fn get_bad_query() -> Result<()> {
        ServerTest::new()?.test_status(
            Request::builder(Method::GET, "http://localhost/query?query=SELECT".parse()?).build(),
            Status::BAD_REQUEST,
        )
    }

    #[test]
    fn get_query_union_graph() -> Result<()> {
        let server = ServerTest::new()?;

        let request = Request::builder(Method::PUT, "http://localhost/store/1".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle")?
            .with_body("<http://example.com> <http://example.com> <http://example.com> .");
        server.test_status(request, Status::CREATED)?;

        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}&union-default-graph"
                .parse()
                ?,
        ).with_header(HeaderName::ACCEPT, "text/csv")
            ?
            .build();
        server.test_body(
            request,
            "s,p,o\r\nhttp://example.com,http://example.com,http://example.com\r\n",
        )
    }

    #[test]
    fn get_query_union_graph_in_url_and_urlencoded() -> Result<()> {
        let server = ServerTest::new()?;

        let request = Request::builder(Method::PUT, "http://localhost/store/1".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle")?
            .with_body("<http://example.com> <http://example.com> <http://example.com> .");
        server.test_status(request, Status::CREATED)?;

        let request = Request::builder(
            Method::POST,
            "http://localhost/query?union-default-graph".parse()?,
        )
        .with_header(
            HeaderName::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )?
        .with_header(HeaderName::ACCEPT, "text/csv")?
        .with_body("query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}");
        server.test_body(
            request,
            "s,p,o\r\nhttp://example.com,http://example.com,http://example.com\r\n",
        )
    }

    #[test]
    fn get_query_union_graph_and_default_graph() -> Result<()> {
        ServerTest::new()?.test_status(Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20*%20WHERE%20{%20?s%20?p%20?o%20}&union-default-graph&default-graph-uri=http://example.com".parse()
                ?,
        ).build(), Status::BAD_REQUEST)
    }

    #[test]
    fn get_without_query() -> Result<()> {
        ServerTest::new()?.test_status(
            Request::builder(Method::GET, "http://localhost/query".parse()?).build(),
            Status::BAD_REQUEST,
        )
    }

    #[test]
    fn post_query() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/query".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-query")?
            .with_body("SELECT * WHERE { ?s ?p ?o }");
        ServerTest::new()?.test_status(request, Status::OK)
    }

    #[test]
    fn post_bad_query() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/query".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-query")?
            .with_body("SELECT");
        ServerTest::new()?.test_status(request, Status::BAD_REQUEST)
    }

    #[test]
    fn post_unknown_query() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/query".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-todo")?
            .with_body("SELECT");
        ServerTest::new()?.test_status(request, Status::UNSUPPORTED_MEDIA_TYPE)
    }

    #[test]
    fn post_federated_query_wikidata() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/query".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-query")
            ?.with_body("SELECT * WHERE { SERVICE <https://query.wikidata.org/sparql> { <https://en.wikipedia.org/wiki/Paris> ?p ?o } }");
        ServerTest::new()?.test_status(request, Status::OK)
    }

    #[test]
    fn post_federated_query_dbpedia() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/query".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-query")
            ?.with_body("SELECT * WHERE { SERVICE <https://dbpedia.org/sparql> { <http://dbpedia.org/resource/Paris> ?p ?o } }");
        ServerTest::new()?.test_status(request, Status::OK)
    }

    #[test]
    fn post_update() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/update".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-update")?
            .with_body(
                "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
            );
        ServerTest::new()?.test_status(request, Status::NO_CONTENT)
    }

    #[test]
    fn post_bad_update() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/update".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-update")?
            .with_body("INSERT");
        ServerTest::new()?.test_status(request, Status::BAD_REQUEST)
    }

    #[test]
    fn post_update_read_only() -> Result<()> {
        let request = Request::builder(Method::POST, "http://localhost/update".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "application/sparql-update")?
            .with_body(
                "INSERT DATA { <http://example.com> <http://example.com> <http://example.com> }",
            );
        ServerTest::check_status(
            ServerTest::new()?.exec_read_only(request),
            Status::FORBIDDEN,
        )
    }

    #[test]
    fn graph_store_url_normalization() -> Result<()> {
        let server = ServerTest::new()?;

        // PUT
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store?graph=http://example.com".parse()?,
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle")?
        .with_body("<http://example.com> <http://example.com> <http://example.com> .");
        server.test_status(request, Status::CREATED)?;

        // GET good URI
        server.test_status(
            Request::builder(
                Method::GET,
                "http://localhost/store?graph=http://example.com".parse()?,
            )
            .build(),
            Status::OK,
        )?;

        // GET bad URI
        server.test_status(
            Request::builder(
                Method::GET,
                "http://localhost/store?graph=http://example.com/".parse()?,
            )
            .build(),
            Status::NOT_FOUND,
        )
    }

    #[test]
    fn graph_store_protocol() -> Result<()> {
        // Tests from https://www.w3.org/2009/sparql/docs/tests/data-sparql11/http-rdf-update/

        let server = ServerTest::new()?;

        // PUT - Initial state
        let request = Request::builder(Method::PUT, "http://localhost/store/person/1.ttl".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
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
        server.test_status(request, Status::CREATED)?;

        // GET of PUT - Initial state
        let request = Request::builder(
            Method::GET,
            "http://localhost/store?graph=/store/person/1.ttl".parse()?,
        )
        .with_header(HeaderName::ACCEPT, "text/turtle")?
        .build();
        server.test_status(request, Status::OK)?;

        // HEAD on an existing graph
        server.test_status(
            Request::builder(Method::HEAD, "http://localhost/store/person/1.ttl".parse()?).build(),
            Status::OK,
        )?;

        // HEAD on a non-existing graph
        server.test_status(
            Request::builder(Method::HEAD, "http://localhost/store/person/4.ttl".parse()?).build(),
            Status::NOT_FOUND,
        )?;

        // PUT - graph already in store
        let request = Request::builder(Method::PUT, "http://localhost/store/person/1.ttl".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
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
        server.test_status(request, Status::NO_CONTENT)?;

        // GET of PUT - graph already in store
        let request = Request::builder(Method::GET, "http://localhost/store/person/1.ttl".parse()?)
            .with_header(HeaderName::ACCEPT, "text/turtle")?
            .build();
        server.test_status(request, Status::OK)?;

        // PUT - default graph
        let request = Request::builder(Method::PUT, "http://localhost/store?default".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
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
        server.test_status(request, Status::NO_CONTENT)?; // The default graph always exists in Oxigraph

        // GET of PUT - default graph
        let request = Request::builder(Method::GET, "http://localhost/store?default".parse()?)
            .with_header(HeaderName::ACCEPT, "text/turtle")?
            .build();
        server.test_status(request, Status::OK)?;

        // PUT - mismatched payload
        let request = Request::builder(Method::PUT, "http://localhost/store/person/1.ttl".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
            .with_body("@prefix foo");
        server.test_status(request, Status::BAD_REQUEST)?;

        // PUT - empty graph
        let request = Request::builder(Method::PUT, "http://localhost/store/person/2.ttl".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
            .build();
        server.test_status(request, Status::CREATED)?;

        // GET of PUT - empty graph
        let request = Request::builder(Method::GET, "http://localhost/store/person/2.ttl".parse()?)
            .with_header(HeaderName::ACCEPT, "text/turtle")?
            .build();
        server.test_status(request, Status::OK)?;

        // PUT - replace empty graph
        let request = Request::builder(Method::PUT, "http://localhost/store/person/2.ttl".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
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
        server.test_status(request, Status::NO_CONTENT)?;

        // GET of replacement for empty graph
        let request = Request::builder(Method::GET, "http://localhost/store/person/2.ttl".parse()?)
            .with_header(HeaderName::ACCEPT, "text/turtle")?
            .build();
        server.test_status(request, Status::OK)?;

        // DELETE - existing graph
        server.test_status(
            Request::builder(
                Method::DELETE,
                "http://localhost/store/person/2.ttl".parse()?,
            )
            .build(),
            Status::NO_CONTENT,
        )?;

        // GET of DELETE - existing graph
        server.test_status(
            Request::builder(Method::GET, "http://localhost/store/person/2.ttl".parse()?).build(),
            Status::NOT_FOUND,
        )?;

        // DELETE - non-existent graph
        server.test_status(
            Request::builder(
                Method::DELETE,
                "http://localhost/store/person/2.ttl".parse()?,
            )
            .build(),
            Status::NOT_FOUND,
        )?;

        // POST - existing graph
        let request = Request::builder(Method::PUT, "http://localhost/store/person/1.ttl".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
            .build();
        server.test_status(request, Status::NO_CONTENT)?;

        // TODO: POST - multipart/form-data
        // TODO: GET of POST - multipart/form-data

        // POST - create new graph
        let request = Request::builder(Method::POST, "http://localhost/store".parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
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
        let location = response.header(&HeaderName::LOCATION).unwrap().to_str()?;

        // GET of POST - create new graph
        let request = Request::builder(Method::GET, location.parse()?)
            .with_header(HeaderName::ACCEPT, "text/turtle")?
            .build();
        server.test_status(request, Status::OK)?;

        // POST - empty graph to existing graph
        let request = Request::builder(Method::PUT, location.parse()?)
            .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
            .build();
        server.test_status(request, Status::NO_CONTENT)?;

        // GET of POST - after noop
        let request = Request::builder(Method::GET, location.parse()?)
            .with_header(HeaderName::ACCEPT, "text/turtle")?
            .build();
        server.test_status(request, Status::OK)
    }

    #[test]
    fn graph_store_lenient_bulk() -> Result<()> {
        let server = ServerTest::new()?;
        let invalid_data = "
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

<http://$HOST$/$GRAPHSTORE$/person/1> a foaf:Person . foo";

        // POST
        let request = Request::builder(
            Method::POST,
            "http://localhost/store/person/1.ttl?no_transaction&lenient".parse()?,
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
        .with_body(invalid_data);
        server.test_status(request, Status::NO_CONTENT)?;

        // GET of POST
        let request = Request::builder(
            Method::GET,
            "http://localhost/store?graph=/store/person/1.ttl".parse()?,
        )
        .with_header(HeaderName::ACCEPT, "text/turtle")?
        .build();
        server.test_status(request, Status::OK)?;

        // POST dataset
        let request = Request::builder(
            Method::POST,
            "http://localhost/store?lenient&no_transaction".parse()?,
        )
        .with_header(HeaderName::CONTENT_TYPE, "application/trig; charset=utf-8")?
        .with_body(invalid_data);
        server.test_status(request, Status::NO_CONTENT)?;

        // GET of POST dataset
        let request = Request::builder(Method::GET, "http://localhost/store?default".parse()?)
            .with_header(HeaderName::ACCEPT, "text/turtle")?
            .build();
        server.test_status(request, Status::OK)?;

        // PUT
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store/person/1.ttl?lenient&no_transaction".parse()?,
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle; charset=utf-8")?
        .with_body(invalid_data);
        server.test_status(request, Status::NO_CONTENT)?;

        // GET of PUT - Initial state
        let request = Request::builder(
            Method::GET,
            "http://localhost/store?graph=/store/person/1.ttl".parse()?,
        )
        .with_header(HeaderName::ACCEPT, "text/turtle")?
        .build();
        server.test_status(request, Status::OK)?;

        // PUT dataset
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store?lenient&no_transaction".parse()?,
        )
        .with_header(HeaderName::CONTENT_TYPE, "application/trig; charset=utf-8")?
        .with_body(invalid_data);
        server.test_status(request, Status::NO_CONTENT)?;

        // GET of PUT dataset
        let request = Request::builder(Method::GET, "http://localhost/store?default".parse()?)
            .with_header(HeaderName::ACCEPT, "text/turtle")?
            .build();
        server.test_status(request, Status::OK)?;

        // GET of PUT dataset - replacement
        let request = Request::builder(
            Method::GET,
            "http://localhost/store?graph=/store/person/1.ttl".parse()?,
        )
        .with_header(HeaderName::ACCEPT, "text/turtle")?
        .build();
        server.test_status(request, Status::NOT_FOUND)
    }

    struct ServerTest {
        store: Store,
    }

    impl ServerTest {
        fn new() -> Result<Self> {
            Ok(Self {
                store: Store::new()?,
            })
        }

        fn exec(&self, mut request: Request) -> Response {
            handle_request(&mut request, self.store.clone(), false)
                .unwrap_or_else(|(status, message)| error(status, message))
        }

        fn exec_read_only(&self, mut request: Request) -> Response {
            handle_request(&mut request, self.store.clone(), true)
                .unwrap_or_else(|(status, message)| error(status, message))
        }

        fn test_status(&self, request: Request, expected_status: Status) -> Result<()> {
            Self::check_status(self.exec(request), expected_status)
        }

        fn check_status(mut response: Response, expected_status: Status) -> Result<()> {
            let mut buf = String::new();
            response.body_mut().read_to_string(&mut buf)?;
            assert_eq!(response.status(), expected_status, "Error message: {buf}");
            Ok(())
        }

        fn test_body(&self, request: Request, expected_body: &str) -> Result<()> {
            let mut response = self.exec(request);
            let mut buf = String::new();
            response.body_mut().read_to_string(&mut buf)?;
            assert_eq!(response.status(), Status::OK, "Error message: {buf}");
            assert_eq!(&buf, expected_body);
            Ok(())
        }
    }

    #[test]
    fn clap_debug() {
        use clap::CommandFactory;

        Args::command().debug_assert()
    }
}
