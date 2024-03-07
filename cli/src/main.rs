#![allow(clippy::print_stderr, clippy::cast_precision_loss, clippy::use_debug)]
use anyhow::{bail, ensure, Context};
use clap::{Parser, Subcommand, ValueHint};
use flate2::read::MultiGzDecoder;
use oxhttp::model::{Body, HeaderName, HeaderValue, Method, Request, Response, Status};
use oxhttp::Server;
use oxigraph::io::{RdfFormat, RdfParser, RdfSerializer};
use oxigraph::model::{
    GraphName, GraphNameRef, IriParseError, NamedNode, NamedNodeRef, NamedOrBlankNode,
};
use oxigraph::sparql::results::{QueryResultsFormat, QueryResultsSerializer};
use oxigraph::sparql::{Query, QueryOptions, QueryResults, Update};
use oxigraph::store::{BulkLoader, LoaderError, Store};
use oxiri::Iri;
use rand::random;
use rayon_core::ThreadPoolBuilder;
use std::borrow::Cow;
use std::cell::RefCell;
use std::cmp::{max, min};
#[cfg(target_os = "linux")]
use std::env;
use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, stdin, stdout, BufWriter, Read, Write};
use std::net::ToSocketAddrs;
#[cfg(target_os = "linux")]
use std::os::unix::net::UnixDatagram;
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
#[command(about, version, name = "oxigraph")]
/// Oxigraph command line toolkit and SPARQL HTTP server.
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Start Oxigraph HTTP server in read-write mode.
    Serve {
        /// Directory in which the data should be persisted.
        ///
        /// If not present. An in-memory storage will be used.
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: Option<PathBuf>,
        /// Host and port to listen to.
        #[arg(short, long, default_value = "localhost:7878", value_hint = ValueHint::Hostname)]
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
        /// Directory in which Oxigraph data are persisted.
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
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
        #[arg(long, value_hint = ValueHint::DirPath)]
        primary_location: PathBuf,
        /// Directory to which the current secondary instance might write to.
        ///
        /// By default, temporary storage is used.
        #[arg(long, value_hint = ValueHint::DirPath)]
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
        /// Directory in which Oxigraph data are persisted.
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// Directory in which the backup will be written.
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        destination: PathBuf,
    },
    /// Load file(s) into the store.
    ///
    /// Feel free to enable the --lenient option if you know your input is valid to get better performances.
    Load {
        /// Directory in which Oxigraph data are persisted.
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// File(s) to load.
        ///
        /// If multiple files are provided they are loaded in parallel.
        ///
        /// If no file is given, stdin is read.
        #[arg(short, long, num_args = 0.., value_hint = ValueHint::FilePath)]
        file: Vec<PathBuf>,
        /// The format of the file(s) to load.
        ///
        /// Can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default the format is guessed from the loaded file extension.
        #[arg(long, required_unless_present = "file")]
        format: Option<String>,
        /// Base IRI of the file(s) to load.
        #[arg(long, value_hint = ValueHint::Url)]
        base: Option<String>,
        /// Attempt to keep loading even if the data file is invalid.
        ///
        /// This disables most of validation on RDF content.
        #[arg(long)]
        lenient: bool,
        /// Name of the graph to load the data to.
        ///
        /// By default the default graph is used.
        ///
        /// Only available when loading a graph file (N-Triples, Turtle...) and not a dataset file (N-Quads, TriG...).
        #[arg(long, value_hint = ValueHint::Url)]
        graph: Option<String>,
    },
    /// Dump the store content into a file.
    Dump {
        /// Directory in which Oxigraph data are persisted.
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// File to dump to.
        ///
        /// If no file is given, stdout is used.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
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
        #[arg(long, value_hint = ValueHint::Url)]
        graph: Option<String>,
    },
    /// Executes a SPARQL query against the store.
    Query {
        /// Directory in which Oxigraph data are persisted.
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// The SPARQL query to execute.
        ///
        /// If no query or query file are given, stdin is used.
        #[arg(short, long, conflicts_with = "query_file")]
        query: Option<String>,
        /// File in which the query is stored.
        ///
        /// If no query or query file are given, stdin is used.
        #[arg(long, conflicts_with = "query", value_hint = ValueHint::FilePath)]
        query_file: Option<PathBuf>,
        /// Base IRI of the query.
        #[arg(long, value_hint = ValueHint::Url)]
        query_base: Option<String>,
        /// File in which the query results will be stored.
        ///
        /// If no file is given, stdout is used.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
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
        #[arg(long, conflicts_with = "explain", value_hint = ValueHint::FilePath)]
        explain_file: Option<PathBuf>,
        /// Computes some evaluation statistics to print as part of the query explanations.
        ///
        /// Beware, computing the statistics adds some overhead to the evaluation runtime.
        #[arg(long)]
        stats: bool,
    },
    /// Executes a SPARQL update against the store.
    Update {
        /// Directory in which Oxigraph data are persisted.
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// The SPARQL update to execute.
        ///
        /// If no query or query file are given, stdin is used.
        #[arg(short, long, conflicts_with = "update_file")]
        update: Option<String>,
        /// File in which the update is stored.
        ///
        /// If no update or update file are given, stdin is used.
        #[arg(long, conflicts_with = "update", value_hint = ValueHint::FilePath)]
        update_file: Option<PathBuf>,
        /// Base IRI of the update.
        #[arg(long, value_hint = ValueHint::Url)]
        update_base: Option<String>,
    },
    /// Optimizes the database storage.
    ///
    /// Done by default in the background when serving requests.
    /// It is likely to not be useful in most of cases except if you provide a read-only SPARQL endpoint under heavy load.
    Optimize {
        /// Directory in which Oxigraph data are persisted.
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
    },
    /// Converts a RDF serialization from one format to an other.
    Convert {
        /// File to convert from.
        ///
        /// If no file is given, stdin is read.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        from_file: Option<PathBuf>,
        /// The format of the file(s) to convert from.
        ///
        /// Can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default the format is guessed from the input file extension.
        #[arg(long, required_unless_present = "from_file")]
        from_format: Option<String>,
        /// Base IRI of the file to read.
        #[arg(long, value_hint = ValueHint::Url)]
        from_base: Option<String>,
        /// File to convert to.
        ///
        /// If no file is given, stdout is written.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        to_file: Option<PathBuf>,
        /// The format of the file(s) to convert from.
        ///
        /// Can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default the format is guessed from the target file extension.
        #[arg(long, required_unless_present = "to_file")]
        to_format: Option<String>,
        /// Attempt to keep converting even if the data file is invalid.
        #[arg(long)]
        lenient: bool,
        /// Only load the given named graph from the input file.
        ///
        /// By default all graphs are loaded.
        #[arg(long, conflicts_with = "from_default_graph", value_hint = ValueHint::Url)]
        from_graph: Option<String>,
        /// Only load the default graph from the input file.
        #[arg(long, conflicts_with = "from_graph")]
        from_default_graph: bool,
        /// Name of the graph to map the default graph to.
        ///
        /// By default the default graph is used.
        #[arg(long, value_hint = ValueHint::Url)]
        to_graph: Option<String>,
    },
}

pub fn main() -> anyhow::Result<()> {
    let matches = Args::parse();
    match matches.command {
        Command::Serve {
            location,
            bind,
            cors,
        } => serve(
            if let Some(location) = location {
                Store::open(location)
            } else {
                Store::new()
            }?,
            &bind,
            false,
            cors,
        ),
        Command::ServeReadOnly {
            location,
            bind,
            cors,
        } => serve(Store::open_read_only(location)?, &bind, true, cors),
        Command::ServeSecondary {
            primary_location,
            secondary_location,
            bind,
            cors,
        } => serve(
            if let Some(secondary_location) = secondary_location {
                Store::open_persistent_secondary(primary_location, secondary_location)
            } else {
                Store::open_secondary(primary_location)
            }?,
            &bind,
            true,
            cors,
        ),
        Command::Backup {
            location,
            destination,
        } => {
            let store = Store::open_read_only(location)?;
            store.backup(destination)?;
            Ok(())
        }
        Command::Load {
            location,
            file,
            lenient,
            format,
            base,
            graph,
        } => {
            let store = Store::open(location)?;
            let format = if let Some(format) = format {
                Some(rdf_format_from_name(&format)?)
            } else {
                None
            };
            let graph = if let Some(iri) = &graph {
                Some(
                    NamedNode::new(iri)
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
                    format.context("The --format option must be set when loading from stdin")?,
                    base.as_deref(),
                    graph,
                    lenient,
                )
            } else {
                ThreadPoolBuilder::new()
                    .num_threads(max(1, available_parallelism()?.get() / 2))
                    .thread_name(|i| format!("Oxigraph bulk loader thread {i}"))
                    .build()?
                    .scope(|s| {
                        for file in file {
                            let store = store.clone();
                            let graph = graph.clone();
                            let base = base.clone();
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
                                            MultiGzDecoder::new(fp),
                                            format.unwrap_or_else(|| {
                                                rdf_format_from_path(&file.with_extension(""))
                                                    .unwrap()
                                            }),
                                            base.as_deref(),
                                            graph,
                                            lenient,
                                        )
                                    } else {
                                        bulk_load(
                                            &loader,
                                            fp,
                                            format.unwrap_or_else(|| {
                                                rdf_format_from_path(&file).unwrap()
                                            }),
                                            base.as_deref(),
                                            graph,
                                            lenient,
                                        )
                                    }
                                } {
                                    eprintln!(
                                        "Error while loading file {}: {}",
                                        file.display(),
                                        error
                                    )
                                    // TODO: hard fail
                                }
                            })
                        }
                    });
                store.flush()?;
                Ok(())
            }
        }
        Command::Dump {
            location,
            file,
            format,
            graph,
        } => {
            let store = Store::open_read_only(location)?;
            let format = if let Some(format) = format {
                rdf_format_from_name(&format)?
            } else if let Some(file) = &file {
                rdf_format_from_path(file)?
            } else {
                bail!("The --format option must be set when writing to stdout")
            };
            let graph = if let Some(graph) = &graph {
                Some(if graph.eq_ignore_ascii_case("default") {
                    GraphNameRef::DefaultGraph
                } else {
                    NamedNodeRef::new(graph)
                        .with_context(|| format!("The target graph name {graph} is invalid"))?
                        .into()
                })
            } else {
                None
            };
            if let Some(file) = file {
                close_file_writer(dump(
                    &store,
                    BufWriter::new(File::create(file)?),
                    format,
                    graph,
                )?)?;
            } else {
                dump(&store, stdout().lock(), format, graph)?.flush()?;
            }
            Ok(())
        }
        Command::Query {
            location,
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
                io::read_to_string(stdin().lock())?
            };
            let query = Query::parse(&query, query_base.as_deref())?;
            let store = Store::open_read_only(location)?;
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
                                QueryResultsFormat::from_extension(ext).with_context(|| {
                                    format!("The file extension '{ext}' is unknown")
                                })
                            })?
                        } else {
                            bail!("The --results-format option must be set when writing to stdout")
                        };
                        if let Some(results_file) = results_file {
                            let mut writer = QueryResultsSerializer::from_format(format)
                                .serialize_solutions_to_write(
                                    BufWriter::new(File::create(results_file)?),
                                    solutions.variables().to_vec(),
                                )?;
                            for solution in solutions {
                                writer.write(&solution?)?;
                            }
                            close_file_writer(writer.finish()?)?;
                        } else {
                            let mut writer = QueryResultsSerializer::from_format(format)
                                .serialize_solutions_to_write(
                                    stdout().lock(),
                                    solutions.variables().to_vec(),
                                )?;
                            for solution in solutions {
                                writer.write(&solution?)?;
                            }
                            writer.finish()?.flush()?;
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
                                QueryResultsFormat::from_extension(ext).with_context(|| {
                                    format!("The file extension '{ext}' is unknown")
                                })
                            })?
                        } else {
                            bail!("The --results-format option must be set when writing to stdout")
                        };
                        if let Some(results_file) = results_file {
                            close_file_writer(
                                QueryResultsSerializer::from_format(format)
                                    .serialize_boolean_to_write(
                                        BufWriter::new(File::create(results_file)?),
                                        result,
                                    )?,
                            )?;
                        } else {
                            QueryResultsSerializer::from_format(format)
                                .serialize_boolean_to_write(stdout().lock(), result)?
                                .flush()?;
                        }
                    }
                    QueryResults::Graph(triples) => {
                        let format = if let Some(name) = &results_format {
                            rdf_format_from_name(name)
                        } else if let Some(results_file) = &results_file {
                            rdf_format_from_path(results_file)
                        } else {
                            bail!("The --results-format option must be set when writing to stdout")
                        }?;
                        let serializer = RdfSerializer::from_format(format);
                        if let Some(results_file) = results_file {
                            let mut writer = serializer
                                .serialize_to_write(BufWriter::new(File::create(results_file)?));
                            for triple in triples {
                                writer.write_triple(triple?.as_ref())?;
                            }
                            close_file_writer(writer.finish()?)?;
                        } else {
                            let mut writer = serializer.serialize_to_write(stdout().lock());
                            for triple in triples {
                                writer.write_triple(triple?.as_ref())?;
                            }
                            writer.finish()?.flush()?;
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
                        explanation.write_in_json(&mut file)?;
                    },
                    Some("txt") => {
                        write!(file, "{explanation:?}")?;
                    },
                    _ => bail!("The given explanation file {} must have an extension that is .json or .txt", explain_file.display())
                }
                close_file_writer(file)?;
            } else if explain || stats {
                eprintln!("{explanation:#?}");
            }
            print_result
        }
        Command::Update {
            location,
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
                io::read_to_string(stdin().lock())?
            };
            let update = Update::parse(&update, update_base.as_deref())?;
            let store = Store::open(location)?;
            store.update(update)?;
            store.flush()?;
            Ok(())
        }
        Command::Optimize { location } => {
            let store = Store::open(location)?;
            store.optimize()?;
            Ok(())
        }
        Command::Convert {
            from_file,
            from_format,
            from_base,
            to_file,
            to_format,
            lenient,
            from_graph,
            from_default_graph,
            to_graph,
        } => {
            let from_format = if let Some(format) = from_format {
                rdf_format_from_name(&format)?
            } else if let Some(file) = &from_file {
                rdf_format_from_path(file)?
            } else {
                bail!("The --from-format option must be set when reading from stdin")
            };
            let mut parser = RdfParser::from_format(from_format);
            if let Some(base) = from_base {
                parser = parser
                    .with_base_iri(&base)
                    .with_context(|| format!("Invalid base IRI {base}"))?;
            }

            let to_format = if let Some(format) = to_format {
                rdf_format_from_name(&format)?
            } else if let Some(file) = &to_file {
                rdf_format_from_path(file)?
            } else {
                bail!("The --to-format option must be set when writing to stdout")
            };
            let serializer = RdfSerializer::from_format(to_format);

            let from_graph = if let Some(from_graph) = from_graph {
                Some(
                    NamedNode::new(&from_graph)
                        .with_context(|| format!("The source graph name {from_graph} is invalid"))?
                        .into(),
                )
            } else if from_default_graph {
                Some(GraphName::DefaultGraph)
            } else {
                None
            };
            let to_graph = if let Some(to_graph) = to_graph {
                NamedNode::new(&to_graph)
                    .with_context(|| format!("The target graph name {to_graph} is invalid"))?
                    .into()
            } else {
                GraphName::DefaultGraph
            };

            match (from_file, to_file) {
                (Some(from_file), Some(to_file)) => close_file_writer(do_convert(
                    parser,
                    File::open(from_file)?,
                    serializer,
                    BufWriter::new(File::create(to_file)?),
                    lenient,
                    &from_graph,
                    &to_graph,
                )?),
                (Some(from_file), None) => do_convert(
                    parser,
                    File::open(from_file)?,
                    serializer,
                    stdout().lock(),
                    lenient,
                    &from_graph,
                    &to_graph,
                )?
                .flush(),
                (None, Some(to_file)) => close_file_writer(do_convert(
                    parser,
                    stdin().lock(),
                    serializer,
                    BufWriter::new(File::create(to_file)?),
                    lenient,
                    &from_graph,
                    &to_graph,
                )?),
                (None, None) => do_convert(
                    parser,
                    stdin().lock(),
                    serializer,
                    stdout().lock(),
                    lenient,
                    &from_graph,
                    &to_graph,
                )?
                .flush(),
            }?;
            Ok(())
        }
    }
}

fn bulk_load(
    loader: &BulkLoader,
    read: impl Read,
    format: RdfFormat,
    base_iri: Option<&str>,
    to_graph_name: Option<NamedNode>,
    lenient: bool,
) -> anyhow::Result<()> {
    let mut parser = RdfParser::from_format(format);
    if let Some(to_graph_name) = to_graph_name {
        parser = parser.with_default_graph(to_graph_name);
    }
    if let Some(base_iri) = base_iri {
        parser = parser
            .with_base_iri(base_iri)
            .with_context(|| format!("Invalid base IRI {base_iri}"))?;
    }
    if lenient {
        parser = parser.unchecked();
    }
    loader.load_from_read(parser, read)?;
    Ok(())
}

fn dump<W: Write>(
    store: &Store,
    write: W,
    format: RdfFormat,
    from_graph_name: Option<GraphNameRef<'_>>,
) -> anyhow::Result<W> {
    ensure!(format.supports_datasets() || from_graph_name.is_some(), "The --graph option is required when writing a format not supporting datasets like NTriples, Turtle or RDF/XML");
    Ok(if let Some(from_graph_name) = from_graph_name {
        store.dump_graph_to_write(from_graph_name, format, write)
    } else {
        store.dump_to_write(format, write)
    }?)
}

fn do_convert<R: Read, W: Write>(
    parser: RdfParser,
    read: R,
    mut serializer: RdfSerializer,
    write: W,
    lenient: bool,
    from_graph: &Option<GraphName>,
    default_graph: &GraphName,
) -> anyhow::Result<W> {
    let mut parser = parser.parse_read(read);
    let first = parser.next(); // We read the first element to get prefixes
    for (prefix_name, prefix_iri) in parser.prefixes() {
        serializer = serializer
            .with_prefix(prefix_name, prefix_iri)
            .with_context(|| format!("Invalid IRI for prefix {prefix_name}: {prefix_iri}"))?;
    }
    let mut writer = serializer.serialize_to_write(write);
    for quad_result in first.into_iter().chain(parser) {
        match quad_result {
            Ok(mut quad) => {
                if let Some(from_graph) = from_graph {
                    if quad.graph_name == *from_graph {
                        quad.graph_name = GraphName::DefaultGraph;
                    } else {
                        continue;
                    }
                }
                if quad.graph_name.is_default_graph() {
                    quad.graph_name = default_graph.clone();
                }
                writer.write_quad(&quad)?;
            }
            Err(e) => {
                if lenient {
                    eprintln!("Parsing error: {e}");
                } else {
                    return Err(e.into());
                }
            }
        }
    }
    Ok(writer.finish()?)
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

fn rdf_format_from_path(path: &Path) -> anyhow::Result<RdfFormat> {
    format_from_path(path, |ext| {
        RdfFormat::from_extension(ext)
            .with_context(|| format!("The file extension '{ext}' is unknown"))
    })
}

fn rdf_format_from_name(name: &str) -> anyhow::Result<RdfFormat> {
    if let Some(t) = RdfFormat::from_extension(name) {
        return Ok(t);
    }
    if let Some(t) = RdfFormat::from_media_type(name) {
        return Ok(t);
    }
    bail!("The file format '{name}' is unknown")
}

fn serve(store: Store, bind: &str, read_only: bool, cors: bool) -> anyhow::Result<()> {
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
    }
    .with_global_timeout(HTTP_TIMEOUT)
    .with_server_name(concat!("Oxigraph/", env!("CARGO_PKG_VERSION")))?
    .with_max_concurrent_connections(available_parallelism()?.get() * 128);
    for socket in bind.to_socket_addrs()? {
        server = server.bind(socket);
    }
    let server = server.spawn()?;
    #[cfg(target_os = "linux")]
    systemd_notify_ready()?;
    eprintln!("Listening for requests at http://{bind}");
    server.join()?;
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
                let query = io::read_to_string(request.body_mut().take(MAX_SPARQL_BODY_SIZE))
                    .map_err(bad_request)?;
                configure_and_evaluate_sparql_query(
                    &store,
                    &[url_query(request)],
                    Some(query),
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
                let update = io::read_to_string(request.body_mut().take(MAX_SPARQL_BODY_SIZE))
                    .map_err(bad_request)?;
                configure_and_evaluate_sparql_update(
                    &store,
                    &[url_query(request)],
                    Some(update),
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
                let format = rdf_content_negotiation(request)?;

                let quads = store.quads_for_pattern(
                    None,
                    None,
                    None,
                    Some(GraphName::from(target).as_ref()),
                );
                ReadForWrite::build_response(
                    move |w| {
                        Ok((
                            RdfSerializer::from_format(format).serialize_to_write(w),
                            quads,
                        ))
                    },
                    |(mut writer, mut quads)| {
                        Ok(if let Some(q) = quads.next() {
                            writer.write_triple(&q?.into())?;
                            Some((writer, quads))
                        } else {
                            writer.finish()?;
                            None
                        })
                    },
                    format.media_type(),
                )
            } else {
                let format = rdf_content_negotiation(request)?;
                if !format.supports_datasets() {
                    return Err(bad_request(format!(
                        "It is not possible to serialize the full RDF dataset using {format} that does not support named graphs"
                    )));
                }
                ReadForWrite::build_response(
                    move |w| {
                        Ok((
                            RdfSerializer::from_format(format).serialize_to_write(w),
                            store.iter(),
                        ))
                    },
                    |(mut writer, mut quads)| {
                        Ok(if let Some(q) = quads.next() {
                            writer.write_quad(&q?)?;
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
                let format = RdfFormat::from_media_type(&content_type)
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
                web_load_graph(&store, request, format, &GraphName::from(target))?;
                Ok(Response::builder(if new {
                    Status::CREATED
                } else {
                    Status::NO_CONTENT
                })
                .build())
            } else {
                let format = RdfFormat::from_media_type(&content_type)
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
                let format = RdfFormat::from_media_type(&content_type)
                    .ok_or_else(|| unsupported_media_type(&content_type))?;
                let new = assert_that_graph_exists(&store, &target).is_ok();
                web_load_graph(&store, request, format, &GraphName::from(target))?;
                Ok(Response::builder(if new {
                    Status::CREATED
                } else {
                    Status::NO_CONTENT
                })
                .build())
            } else {
                let format = RdfFormat::from_media_type(&content_type)
                    .ok_or_else(|| unsupported_media_type(&content_type))?;
                if format.supports_datasets() {
                    web_load_dataset(&store, request, format)?;
                    Ok(Response::builder(Status::NO_CONTENT).build())
                } else {
                    let graph =
                        resolve_with_base(request, &format!("/store/{:x}", random::<u128>()))?;
                    web_load_graph(&store, request, format, &graph.clone().into())?;
                    Ok(Response::builder(Status::CREATED)
                        .with_header(HeaderName::LOCATION, graph.into_string())
                        .unwrap()
                        .build())
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
    Ok(Iri::parse(base_url(request))
        .map_err(bad_request)?
        .resolve(url)
        .map_err(bad_request)?
        .into())
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
                            .serialize_solutions_to_write(w, solutions.variables().to_vec())?,
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
                .serialize_boolean_to_write(&mut body, result)
                .map_err(internal_server_error)?;
            Ok(Response::builder(Status::OK)
                .with_header(HeaderName::CONTENT_TYPE, format.media_type())
                .unwrap()
                .with_body(body))
        }
        QueryResults::Graph(triples) => {
            let format = rdf_content_negotiation(request)?;
            ReadForWrite::build_response(
                move |w| {
                    Ok((
                        RdfSerializer::from_format(format).serialize_to_write(w),
                        triples,
                    ))
                },
                |(mut writer, mut triples)| {
                    Ok(if let Some(t) = triples.next() {
                        writer.write_triple(&t?)?;
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

fn rdf_content_negotiation(request: &Request) -> Result<RdfFormat, HttpError> {
    content_negotiation(
        request,
        RdfFormat::from_media_type,
        RdfFormat::NQuads,
        &[
            ("application", RdfFormat::NQuads),
            ("text", RdfFormat::NQuads),
        ],
        "application/n-quads or text/turtle",
    )
}

fn query_results_content_negotiation(request: &Request) -> Result<QueryResultsFormat, HttpError> {
    content_negotiation(
        request,
        QueryResultsFormat::from_media_type,
        QueryResultsFormat::Json,
        &[
            ("application", QueryResultsFormat::Json),
            ("text", QueryResultsFormat::Json),
        ],
        "application/sparql-results+json or text/tsv",
    )
}

fn content_negotiation<F: Copy>(
    request: &Request,
    parse: impl Fn(&str) -> Option<F>,
    default: F,
    default_by_base: &[(&str, F)],
    example: &str,
) -> Result<F, HttpError> {
    let default_value = HeaderValue::default();
    let header = request
        .header(&HeaderName::ACCEPT)
        .unwrap_or(&default_value)
        .to_str()
        .map_err(|_| bad_request("The Accept header should be a valid ASCII string"))?;

    if header.is_empty() {
        return Ok(default);
    }
    let mut result = None;
    let mut result_score = 0_f32;
    for mut possible in header.split(',') {
        let mut score = 1.;
        if let Some((possible_type, last_parameter)) = possible.rsplit_once(';') {
            if let Some((name, value)) = last_parameter.split_once('=') {
                if name.trim().eq_ignore_ascii_case("q") {
                    score = f32::from_str(value.trim()).map_err(|_| {
                        bad_request(format!("Invalid Accept media type score: {value}"))
                    })?;
                    possible = possible_type;
                }
            }
        }
        if score <= result_score {
            continue;
        }
        let (possible_base, possible_sub) = possible
            .split_once(';')
            .unwrap_or((possible, ""))
            .0
            .split_once('/')
            .ok_or_else(|| bad_request(format!("Invalid media type: '{possible}'")))?;
        let possible_base = possible_base.trim();
        let possible_sub = possible_sub.trim();

        let mut format = None;
        if possible_base == "*" && possible_sub == "*" {
            format = Some(default);
        } else if possible_sub == "*" {
            for (base, sub_format) in default_by_base {
                if *base == possible_base {
                    format = Some(*sub_format);
                }
            }
        } else {
            format = parse(possible);
        }
        if let Some(format) = format {
            result = Some(format);
            result_score = score;
        }
    }

    result.ok_or_else(|| {
        (
            Status::NOT_ACCEPTABLE,
            format!("The accept header does not provide any accepted format like {example}"),
        )
    })
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
    format: RdfFormat,
    to_graph_name: &GraphName,
) -> Result<(), HttpError> {
    let base_iri = if let GraphName::NamedNode(graph_name) = to_graph_name {
        Some(graph_name.as_str())
    } else {
        None
    };
    let mut parser = RdfParser::from_format(format)
        .without_named_graphs()
        .with_default_graph(to_graph_name.clone());
    if let Some(base_iri) = base_iri {
        parser = parser.with_base_iri(base_iri).map_err(bad_request)?;
    }
    if url_query_parameter(request, "no_transaction").is_some() {
        web_bulk_loader(store, request).load_from_read(parser, request.body_mut())
    } else {
        store.load_from_read(parser, request.body_mut())
    }
    .map_err(loader_to_http_error)
}

fn web_load_dataset(
    store: &Store,
    request: &mut Request,
    format: RdfFormat,
) -> Result<(), HttpError> {
    if url_query_parameter(request, "no_transaction").is_some() {
        web_bulk_loader(store, request).load_from_read(format, request.body_mut())
    } else {
        store.load_from_read(format, request.body_mut())
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
        LoaderError::InvalidBaseIri { .. } => bad_request(e),
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
            .map_err(internal_server_error)?
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

fn close_file_writer(writer: BufWriter<File>) -> io::Result<()> {
    let mut file = writer
        .into_inner()
        .map_err(io::IntoInnerError::into_error)?;
    file.flush()?;
    file.sync_all()
}

#[cfg(target_os = "linux")]
fn systemd_notify_ready() -> io::Result<()> {
    if let Some(path) = env::var_os("NOTIFY_SOCKET") {
        UnixDatagram::unbound()?.send_to(b"READY=1", path)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::panic_in_result_fn)]

    use super::*;
    use anyhow::Result;
    use assert_cmd::Command;
    use assert_fs::prelude::*;
    use assert_fs::{NamedTempFile, TempDir};
    use flate2::write::GzEncoder;
    use flate2::Compression;
    use predicates::prelude::*;
    use std::fs::remove_dir_all;
    use std::io::read_to_string;

    fn cli_command() -> Result<Command> {
        Ok(Command::from_std(
            escargot::CargoBuild::new()
                .bin("oxigraph")
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
        let input_file = NamedTempFile::new("input.ttl")?;
        input_file.write_str("<s> <http://example.com/p> <http://example.com/o> .")?;
        cli_command()?
            .arg("load")
            .arg("--location")
            .arg(store_dir.path())
            .arg("--file")
            .arg(input_file.path())
            .arg("--base")
            .arg("http://example.com/")
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
    fn cli_convert_file() -> Result<()> {
        let input_file = NamedTempFile::new("input.ttl")?;
        input_file.write_str("@prefix schema: <http://schema.org/> .\n<http://example.com#me> a schema:Person ;\n\tschema:name \"Foo Bar\"@en .\n")?;
        let output_file = NamedTempFile::new("output.rdf")?;
        cli_command()?
            .arg("convert")
            .arg("--from-file")
            .arg(input_file.path())
            .arg("--from-base")
            .arg("http://example.com/")
            .arg("--to-file")
            .arg(output_file.path())
            .assert()
            .success();
        output_file
            .assert("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<rdf:RDF xmlns:schema=\"http://schema.org/\" xmlns:rdf=\"http://www.w3.org/1999/02/22-rdf-syntax-ns#\">\n\t<schema:Person rdf:about=\"http://example.com#me\">\n\t\t<schema:name xml:lang=\"en\">Foo Bar</schema:name>\n\t</schema:Person>\n</rdf:RDF>");
        Ok(())
    }

    #[test]
    fn cli_convert_from_default_graph_to_named_graph() -> Result<()> {
        cli_command()?
            .arg("convert")
            .arg("--from-format")
            .arg("trig")
            .arg("--to-format")
            .arg("nq")
            .arg("--from-default-graph")
            .arg("--to-graph")
            .arg("http://example.com/t")
            .write_stdin("@base <http://example.com/> . <s> <p> <o> . <g> { <sg> <pg> <og> . }")
            .assert()
            .stdout("<http://example.com/s> <http://example.com/p> <http://example.com/o> <http://example.com/t> .\n")
            .success();
        Ok(())
    }

    #[test]
    fn cli_convert_from_named_graph() -> Result<()> {
        cli_command()?
            .arg("convert")
            .arg("--from-format")
            .arg("trig")
            .arg("--to-format")
            .arg("nq")
            .arg("--from-graph")
            .arg("http://example.com/g")
            .write_stdin("@base <http://example.com/> . <s> <p> <o> . <g> { <sg> <pg> <og> . }")
            .assert()
            .stdout("<http://example.com/sg> <http://example.com/pg> <http://example.com/og> .\n");
        Ok(())
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
            r#"{"head":{"vars":["s","p","o"]},"results":{"bindings":[]}}"#,
        )
    }

    #[test]
    fn get_query_accept_substar() -> Result<()> {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()?,
        )
        .with_header(HeaderName::ACCEPT, "text/*")?
        .build();
        ServerTest::new()?.test_body(
            request,
            r#"{"head":{"vars":["s","p","o"]},"results":{"bindings":[]}}"#,
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
            r#"{"head":{"vars":["s","p","o"]},"results":{"bindings":[]}}"#,
        )
    }

    #[test]
    fn get_query_accept_bad() -> Result<()> {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()?,
        )
        .with_header(HeaderName::ACCEPT, "application/foo")?
        .build();
        ServerTest::new()?.test_status(request, Status::NOT_ACCEPTABLE)
    }

    #[test]
    fn get_query_accept_explicit_priority() -> Result<()> {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()?,
        )
        .with_header(HeaderName::ACCEPT, "text/foo;q=0.5 , text/json ; q = 0.7")?
        .build();
        ServerTest::new()?.test_body(
            request,
            r#"{"head":{"vars":["s","p","o"]},"results":{"bindings":[]}}"#,
        )
    }

    #[test]
    fn get_query_accept_implicit_priority() -> Result<()> {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()?,
        )
        .with_header(HeaderName::ACCEPT, "text/json,text/foo")?
        .build();
        ServerTest::new()?.test_body(
            request,
            r#"{"head":{"vars":["s","p","o"]},"results":{"bindings":[]}}"#,
        )
    }
    #[test]
    fn get_query_accept_implicit_and_explicit_priority() -> Result<()> {
        let request = Request::builder(
            Method::GET,
            "http://localhost/query?query=SELECT%20?s%20?p%20?o%20WHERE%20{%20?s%20?p%20?o%20}"
                .parse()?,
        )
        .with_header(HeaderName::ACCEPT, "text/foo;q=0.9,text/csv")?
        .build();
        ServerTest::new()?.test_body(request, "s,p,o\r\n")
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
    fn graph_store_base_url() -> Result<()> {
        let server = ServerTest::new()?;

        // POST
        let request = Request::builder(
            Method::POST,
            "http://localhost/store?graph=http://example.com".parse()?,
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle")?
        .with_body("<> <http://example.com/p> <http://example.com/o1> .");
        server.test_status(request, Status::NO_CONTENT)?;

        // GET
        let request = Request::builder(
            Method::GET,
            "http://localhost/store?graph=http://example.com".parse()?,
        )
        .with_header(HeaderName::ACCEPT, "application/n-triples")?
        .build();
        server.test_body(
            request,
            "<http://example.com> <http://example.com/p> <http://example.com/o1> .\n",
        )?;

        // PUT
        let request = Request::builder(
            Method::PUT,
            "http://localhost/store?graph=http://example.com".parse()?,
        )
        .with_header(HeaderName::CONTENT_TYPE, "text/turtle")?
        .with_body("<> <http://example.com/p> <http://example.com/o2> .");
        server.test_status(request, Status::NO_CONTENT)?;

        // GET
        let request = Request::builder(
            Method::GET,
            "http://localhost/store?graph=http://example.com".parse()?,
        )
        .with_header(HeaderName::ACCEPT, "application/n-triples")?
        .build();
        server.test_body(
            request,
            "<http://example.com> <http://example.com/p> <http://example.com/o2> .\n",
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
                r#"
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

<http://$HOST$/$GRAPHSTORE$/person/1> a foaf:Person;
    foaf:businessCard [
        a v:VCard;
        v:fn "John Doe"
    ].
"#,
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
                r#"
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

<http://$HOST$/$GRAPHSTORE$/person/1> a foaf:Person;
    foaf:businessCard [
        a v:VCard;
        v:fn "Jane Doe"
    ].
"#,
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
                r#"
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

[]  a foaf:Person;
    foaf:businessCard [
        a v:VCard;
        v:given-name "Alice"
    ] .
"#,
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
                r#"
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

[]  a foaf:Person;
    foaf:businessCard [
        a v:VCard;
        v:given-name "Alice"
    ] .
"#,
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
                r#"
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix v: <http://www.w3.org/2006/vcard/ns#> .

[]  a foaf:Person;
    foaf:businessCard [
        a v:VCard;
        v:given-name "Alice"
    ] .
"#,
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
            let body = read_to_string(response.body_mut())?;
            assert_eq!(response.status(), expected_status, "Error message: {body}");
            Ok(())
        }

        fn test_body(&self, request: Request, expected_body: &str) -> Result<()> {
            let mut response = self.exec(request);
            let body = read_to_string(response.body_mut())?;
            assert_eq!(response.status(), Status::OK, "Error message: {body}");
            assert_eq!(&body, expected_body);
            Ok(())
        }
    }

    #[test]
    fn clap_debug() {
        use clap::CommandFactory;

        Args::command().debug_assert()
    }
}
