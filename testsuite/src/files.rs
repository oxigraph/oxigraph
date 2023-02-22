use anyhow::{anyhow, bail, Result};
use oxigraph::io::{DatasetFormat, DatasetParser, GraphFormat, GraphParser};
use oxigraph::model::{Dataset, Graph};
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

pub fn read_file(url: &str) -> Result<impl BufRead> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(if url.starts_with("http://w3c.github.io/") {
        url.replace("http://w3c.github.io/", "")
    } else if url.starts_with("https://w3c.github.io/") {
        url.replace("https://w3c.github.io/", "")
    } else if url.starts_with("http://www.w3.org/2013/RDFXMLTests/") {
        url.replace("http://www.w3.org/2013/RDFXMLTests/", "rdf-tests/rdf-xml/")
    } else if url.starts_with("https://github.com/oxigraph/oxigraph/tests/") {
        url.replace(
            "https://github.com/oxigraph/oxigraph/tests/",
            "oxigraph-tests/",
        )
    } else {
        bail!("Not supported url for file: {url}")
    });
    Ok(BufReader::new(File::open(&path)?))
}

pub fn read_file_to_string(url: &str) -> Result<String> {
    let mut buf = String::new();
    read_file(url)?.read_to_string(&mut buf)?;
    Ok(buf)
}

pub fn load_to_graph(url: &str, graph: &mut Graph, format: GraphFormat) -> Result<()> {
    let parser = GraphParser::from_format(format).with_base_iri(url)?;
    for t in parser.read_triples(read_file(url)?)? {
        graph.insert(&t?);
    }
    Ok(())
}

pub fn load_graph(url: &str, format: GraphFormat) -> Result<Graph> {
    let mut graph = Graph::new();
    load_to_graph(url, &mut graph, format)?;
    Ok(graph)
}

pub fn guess_graph_format(url: &str) -> Result<GraphFormat> {
    url.rsplit_once('.')
        .and_then(|(_, extension)| GraphFormat::from_extension(extension))
        .ok_or_else(|| anyhow!("Serialization type not found for {url}"))
}

pub fn load_to_dataset(url: &str, dataset: &mut Dataset, format: DatasetFormat) -> Result<()> {
    let parser = DatasetParser::from_format(format).with_base_iri(url)?;
    for q in parser.read_quads(read_file(url)?)? {
        dataset.insert(&q?);
    }
    Ok(())
}

pub fn load_dataset(url: &str, format: DatasetFormat) -> Result<Dataset> {
    let mut dataset = Dataset::new();
    load_to_dataset(url, &mut dataset, format)?;
    Ok(dataset)
}

pub fn guess_dataset_format(url: &str) -> Result<DatasetFormat> {
    url.rsplit_once('.')
        .and_then(|(_, extension)| DatasetFormat::from_extension(extension))
        .ok_or_else(|| anyhow!("Serialization type not found for {url}"))
}
