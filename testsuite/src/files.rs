use anyhow::{anyhow, Result};
use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::GraphName;
use oxigraph::MemoryStore;
use std::fs::File;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;

pub fn read_file(url: &str) -> Result<impl BufRead> {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push(if url.starts_with("http://w3c.github.io/rdf-tests/") {
        Ok(url.replace("http://w3c.github.io/rdf-tests/", "rdf-tests/"))
    } else if url.starts_with("http://www.w3.org/2013/RDFXMLTests/") {
        Ok(url.replace("http://www.w3.org/2013/RDFXMLTests/", "rdf-tests/rdf-xml/"))
    } else if url.starts_with("http://www.w3.org/2001/sw/DataAccess/tests/data-r2/") {
        Ok(url.replace(
            "http://www.w3.org/2001/sw/DataAccess/tests/",
            "rdf-tests/sparql11/",
        ))
    } else if url.starts_with("http://www.w3.org/2009/sparql/docs/tests/data-sparql11/") {
        Ok(url.replace(
            "http://www.w3.org/2009/sparql/docs/tests/",
            "rdf-tests/sparql11/",
        ))
    } else if url.starts_with("https://github.com/oxigraph/oxigraph/tests/") {
        Ok(url.replace(
            "https://github.com/oxigraph/oxigraph/tests/",
            "oxigraph-tests/",
        ))
    } else {
        Err(anyhow!("Not supported url for file: {}", url))
    }?);
    Ok(BufReader::new(File::open(&path)?))
}

pub fn read_file_to_string(url: &str) -> Result<String> {
    let mut buf = String::new();
    read_file(url)?.read_to_string(&mut buf)?;
    Ok(buf)
}

pub fn load_to_store(url: &str, store: &MemoryStore, to_graph_name: &GraphName) -> Result<()> {
    if url.ends_with(".nt") {
        store.load_graph(
            read_file(url)?,
            GraphFormat::NTriples,
            to_graph_name,
            Some(url),
        )?
    } else if url.ends_with(".ttl") {
        store.load_graph(
            read_file(url)?,
            GraphFormat::Turtle,
            to_graph_name,
            Some(url),
        )?
    } else if url.ends_with(".rdf") {
        store.load_graph(
            read_file(url)?,
            GraphFormat::RdfXml,
            to_graph_name,
            Some(url),
        )?
    } else if url.ends_with(".nq") {
        store.load_dataset(read_file(url)?, DatasetFormat::NQuads, Some(url))?
    } else if url.ends_with(".trig") {
        store.load_dataset(read_file(url)?, DatasetFormat::TriG, Some(url))?
    } else {
        return Err(anyhow!("Serialization type not found for {}", url));
    }
    Ok(())
}

pub fn load_store(url: &str) -> Result<MemoryStore> {
    let store = MemoryStore::new();
    load_to_store(url, &store, &GraphName::DefaultGraph)?;
    Ok(store)
}
