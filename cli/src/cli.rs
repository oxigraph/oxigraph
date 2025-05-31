use clap::{Parser, Subcommand, ValueHint};
use std::path::PathBuf;

#[derive(Parser)]
#[command(about, version, name = "oxigraph")]
/// Oxigraph command line toolkit and SPARQL HTTP server
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Start Oxigraph HTTP server in read-write mode
    Serve {
        /// Directory in which the data should be persisted
        ///
        /// If not present, an in-memory storage will be used.
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: Option<PathBuf>,
        /// Host and port to listen to
        #[arg(short, long, default_value = "localhost:7878", value_hint = ValueHint::Hostname)]
        bind: String,
        /// Allows cross-origin requests
        #[arg(long)]
        cors: bool,
        /// If the SPARQL queries should look for triples in all the dataset graphs by default (i.e., without `GRAPH` operations)
        ///
        /// This is equivalent as setting the union-default-graph option in all SPARQL queries
        #[arg(long)]
        union_default_graph: bool,
    },
    /// Start Oxigraph HTTP server in read-only mode
    ///
    /// It allows reading the database while other processes are also reading it.
    /// Opening as read-only while having another process writing the database is undefined behavior.
    ServeReadOnly {
        /// Directory in which Oxigraph data are persisted
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// Host and port to listen to
        #[arg(short, long, default_value = "localhost:7878")]
        bind: String,
        /// Allow cross-origin requests
        #[arg(long)]
        cors: bool,
        /// If the SPARQL queries should look for triples in all the dataset graphs by default (i.e. without `GRAPH` operations)
        ///
        /// This is equivalent as setting the union-default-graph option in all SPARQL queries
        #[arg(long)]
        union_default_graph: bool,
    },
    /// Create a database backup into a target directory
    ///
    /// After its creation, the backup is usable a separated Oxigraph database
    /// and operates independently of the original database.
    ///
    /// If the target directory is in the same file system as the current database,
    /// the database content will not be fully copied,
    /// but hard links will be used to point to the original database immutable snapshots.
    /// This allows cheap regular backups.
    ///
    /// If you want to move your data to another RDF storage system, you should use the dump operation instead.
    Backup {
        /// Directory in which Oxigraph data are persisted
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// Directory in which the backup will be written
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        destination: PathBuf,
    },
    /// Load file(s) into the store
    ///
    /// Feel free to enable the --lenient option if you know your input is valid to get better performances.
    Load {
        /// Directory in which Oxigraph data are persisted
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// File(s) to load
        ///
        /// If multiple files are provided, they are loaded in parallel.
        ///
        /// If no file is given, stdin is used as if it were the input file content.
        /// In this case, the content format must be specified using the --format option.
        #[arg(short, long, num_args = 0.., value_hint = ValueHint::FilePath)]
        file: Vec<PathBuf>,
        /// The format of the file(s) to load
        ///
        /// It can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default, the format is guessed from the loaded file extension.
        #[arg(long, required_unless_present = "file")]
        format: Option<String>,
        /// Base IRI of the file(s) to load
        #[arg(long, value_hint = ValueHint::Url)]
        base: Option<String>,
        /// Attempt to keep loading even if the data file is invalid
        ///
        /// This disables most of the validation on RDF content.
        #[arg(long)]
        lenient: bool,
        /// Name of the graph to load the data to
        ///
        /// By default, the default graph is used.
        ///
        /// Only available when loading a graph file (N-Triples, Turtle...) and not a dataset file (N-Quads, TriG...).
        #[arg(long, value_hint = ValueHint::Url)]
        graph: Option<String>,
    },
    /// Dump the store content into a file
    Dump {
        /// Directory in which Oxigraph data are persisted
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// File to dump to
        ///
        /// If no file is given, stdout is used.
        /// In this case, the output format must be specified using the --format option.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        file: Option<PathBuf>,
        /// The format of the file(s) to dump
        ///
        /// It can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default, the format is guessed from the target file extension.
        #[arg(long, required_unless_present = "file")]
        format: Option<String>,
        /// Name of the graph to dump
        ///
        /// Use "default" to dump the default graph.
        ///
        /// By default, all graphs are dumped if the output format supports datasets.
        /// If the format does not support named graphs, then this parameter must be set.
        #[arg(long, value_hint = ValueHint::Url)]
        graph: Option<String>,
    },
    /// Execute a SPARQL query against the store
    Query {
        /// Directory in which Oxigraph data are persisted
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// The SPARQL query to execute
        ///
        /// If no query or query file are given, stdin is used.
        #[arg(short, long, conflicts_with = "query_file")]
        query: Option<String>,
        /// File in which the query is stored
        ///
        /// If no query or query file are given, stdin is used.
        #[arg(long, conflicts_with = "query", value_hint = ValueHint::FilePath)]
        query_file: Option<PathBuf>,
        /// Base IRI of the query
        #[arg(long, value_hint = ValueHint::Url)]
        query_base: Option<String>,
        /// File in which the query results will be stored
        ///
        /// If no file is given, stdout is used.
        /// In this case, the output format must be specified using the --format option.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        results_file: Option<PathBuf>,
        /// The results format
        ///
        /// It can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default, the format is guessed from the results file extension.
        #[arg(long, required_unless_present = "results_file")]
        results_format: Option<String>,
        /// Print to stderr a human-readable explanation of the query evaluation
        ///
        /// Use the --stats option to print also query evaluation statistics.
        #[arg(long, conflicts_with = "explain_file")]
        explain: bool,
        /// Write to the given file an explanation of the query evaluation
        ///
        /// If the file extension is ".json" the JSON format is used, if ".txt" a human-readable format is used.
        ///
        /// Use the --stats option to print also query evaluation statistics.
        #[arg(long, conflicts_with = "explain", value_hint = ValueHint::FilePath)]
        explain_file: Option<PathBuf>,
        /// Compute some evaluation statistics to print as part of the query explanations
        ///
        /// Beware, computing the statistics adds some overhead to the evaluation runtime.
        #[arg(long)]
        stats: bool,
        /// If the SPARQL queries should look for triples in all the dataset graphs by default (ie. without `GRAPH` operations)
        #[arg(long)]
        union_default_graph: bool,
    },
    /// Execute a SPARQL update against the store
    Update {
        /// Directory in which Oxigraph data are persisted
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
        /// The SPARQL update to execute
        ///
        /// If no query or query file are given, stdin is used.
        #[arg(short, long, conflicts_with = "update_file")]
        update: Option<String>,
        /// File in which the update is stored
        ///
        /// If no update or update file are given, stdin is used.
        #[arg(long, conflicts_with = "update", value_hint = ValueHint::FilePath)]
        update_file: Option<PathBuf>,
        /// Base IRI of the update
        #[arg(long, value_hint = ValueHint::Url)]
        update_base: Option<String>,
    },
    /// Optimize the database storage
    ///
    /// Done by default in the background when serving requests.
    /// It is likely to not be useful in most of the cases except if you provide a read-only SPARQL endpoint under heavy load.
    Optimize {
        /// Directory in which Oxigraph data are persisted
        #[arg(short, long, value_hint = ValueHint::DirPath)]
        location: PathBuf,
    },
    /// Convert an RDF file from one format to another
    Convert {
        /// File to convert from
        ///
        /// If no file is given, stdin is used as if it were the input file content.
        /// In this case, the content format must be specified using the --format option.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        from_file: Option<PathBuf>,
        /// The format of the file(s) to convert from
        ///
        /// It can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default, the format is guessed from the input file extension.
        #[arg(long, required_unless_present = "from_file")]
        from_format: Option<String>,
        /// Base IRI of the file to read
        #[arg(long, value_hint = ValueHint::Url)]
        from_base: Option<String>,
        /// File to convert to
        ///
        /// If no file is given, stdout is used.
        /// In this case, the output format must be specified using the --to-format option.
        #[arg(short, long, value_hint = ValueHint::FilePath)]
        to_file: Option<PathBuf>,
        /// The format of the file(s) to convert to
        ///
        /// It can be an extension like "nt" or a MIME type like "application/n-triples".
        ///
        /// By default, the format is guessed from the target file extension.
        #[arg(long, required_unless_present = "to_file")]
        to_format: Option<String>,
        /// Base IRI of the file to write
        #[arg(long, value_hint = ValueHint::Url)]
        to_base: Option<String>,
        /// Attempt to keep converting even if the data file is invalid
        ///
        /// This disables most of the validation on RDF content.
        #[arg(long)]
        lenient: bool,
        /// Only load the given named graph from the input file
        ///
        /// By default, all graphs are loaded.
        #[arg(long, conflicts_with = "from_default_graph", value_hint = ValueHint::Url)]
        from_graph: Option<String>,
        /// Only load the default graph from the input file
        #[arg(long, conflicts_with = "from_graph")]
        from_default_graph: bool,
        /// Name of the graph to map the default graph to
        ///
        /// By default, the default graph is used.
        #[arg(long, value_hint = ValueHint::Url)]
        to_graph: Option<String>,
    },
}
