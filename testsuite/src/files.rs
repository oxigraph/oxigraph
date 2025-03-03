use anyhow::{bail, Context, Result};
use json_ld::fs::Error;
use json_ld::{Iri, IriBuf, LoadError, Loader, LoadingResult, RemoteDocument};
use oxigraph::io::{RdfFormat, RdfParser};
use oxigraph::model::{Dataset, Graph};
use oxttl::n3::N3Quad;
use oxttl::N3Parser;
use std::fs::File;
use std::io;
use std::io::Read;
use std::path::Path;

pub fn read_file(url: &str) -> Result<impl Read> {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join(if url.starts_with("https://w3c.github.io/") {
            url.replace("https://w3c.github.io/", "")
        } else if url.starts_with("https://github.com/oxigraph/oxigraph/tests/") {
            url.replace(
                "https://github.com/oxigraph/oxigraph/tests/",
                "oxigraph-tests/",
            )
        } else if url.starts_with("http://drobilla.net/sw/serd/test/") {
            url.replace("http://drobilla.net/sw/serd/test/", "serd-tests/")
        } else {
            bail!("Not supported url for file: {url}")
        });
    File::open(&path).with_context(|| format!("Failed to read {}", path.display()))
}

pub fn read_file_to_string(url: &str) -> Result<String> {
    let mut buf = String::new();
    read_file(url)?.read_to_string(&mut buf)?;
    Ok(buf)
}

pub fn load_to_graph(
    url: &str,
    graph: &mut Graph,
    format: RdfFormat,
    base_iri: Option<&str>,
    ignore_errors: bool,
) -> Result<()> {
    let parser = RdfParser::from_format(format).with_base_iri(base_iri.unwrap_or(url))?;
    for t in parser.for_reader(read_file(url)?) {
        match t {
            Ok(t) => {
                graph.insert(&t.into());
            }
            Err(e) => {
                if !ignore_errors {
                    return Err(e.into());
                }
            }
        }
    }
    Ok(())
}

pub fn load_graph(url: &str, format: RdfFormat, ignore_errors: bool) -> Result<Graph> {
    let mut graph = Graph::new();
    load_to_graph(url, &mut graph, format, None, ignore_errors)?;
    Ok(graph)
}

pub fn load_to_dataset(
    url: &str,
    dataset: &mut Dataset,
    format: RdfFormat,
    ignore_errors: bool,
    unchecked: bool,
) -> Result<()> {
    let mut parser = RdfParser::from_format(format).with_base_iri(url)?;
    if unchecked {
        parser = parser.unchecked();
    }
    for q in parser.for_reader(read_file(url)?) {
        match q {
            Ok(q) => {
                dataset.insert(&q);
            }
            Err(e) => {
                if !ignore_errors {
                    return Err(e.into());
                }
            }
        }
    }
    Ok(())
}

pub fn load_dataset(
    url: &str,
    format: RdfFormat,
    ignore_errors: bool,
    unchecked: bool,
) -> Result<Dataset> {
    let mut dataset = Dataset::new();
    load_to_dataset(url, &mut dataset, format, ignore_errors, unchecked)?;
    Ok(dataset)
}

pub fn guess_rdf_format(url: &str) -> Result<RdfFormat> {
    url.rsplit_once('.')
        .and_then(|(_, extension)| RdfFormat::from_extension(extension))
        .with_context(|| format!("Serialization type not found for {url}"))
}

pub fn load_n3(url: &str, ignore_errors: bool) -> Result<Vec<N3Quad>> {
    let mut quads = Vec::new();
    for q in N3Parser::new()
        .with_base_iri(url)?
        .with_prefix("", format!("{url}#"))?
        .for_reader(read_file(url)?)
    {
        match q {
            Ok(q) => quads.push(q),
            Err(e) => {
                if !ignore_errors {
                    return Err(e.into());
                }
            }
        }
    }
    Ok(quads)
}

pub struct JsonLdLoader;

impl Loader for JsonLdLoader {
    async fn load(&self, url: &Iri) -> LoadingResult<IriBuf> {
        let mut file = read_file(url.as_str())
            .map_err(|e| LoadError::new(url.to_owned(), Error::IO(io::Error::other(e))))?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .map_err(|e| LoadError::new(url.to_owned(), Error::IO(e)))?;
        Ok(RemoteDocument::new(
            Some(url.to_owned()),
            Some("application/ld+json".parse().unwrap()),
            contents
                .parse()
                .map_err(|e| LoadError::new(url.to_owned(), Error::Parse(e)))?,
        ))
    }
}
