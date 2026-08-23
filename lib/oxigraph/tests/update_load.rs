#![cfg(test)]
#![cfg(all(feature = "http-client", not(target_family = "wasm")))]
#![allow(clippy::panic_in_result_fn)]

use oxigraph::model::{GraphName, NamedNode, Quad};
use oxigraph::sparql::SparqlEvaluator;
use oxigraph::store::Store;
use std::error::Error;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::thread;
use std::time::Duration;

#[test]
fn load_creates_empty_named_graph() -> Result<(), Box<dyn Error>> {
    check_load("", false, true)
}

#[test]
fn load_silent_does_not_insert_partial_document() -> Result<(), Box<dyn Error>> {
    check_load("<urn:s> <urn:p> <urn:o> .\n<urn:broken>", true, false)
}

#[test]
fn load_does_not_insert_partial_document_on_error() -> Result<(), Box<dyn Error>> {
    check_load("<urn:s> <urn:p> <urn:o> .\n<urn:broken>", false, false)
}

fn check_load(body: &'static str, silent: bool, valid: bool) -> Result<(), Box<dyn Error>> {
    // Exercise both the write-only store path and the readable transaction path.
    for in_transaction in [false, true] {
        let listener = TcpListener::bind(("127.0.0.1", 0))?;
        let address = listener.local_addr()?;
        let server = thread::spawn(move || -> std::io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let mut reader = BufReader::new(&stream);
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line)? == 0 || line == "\r\n" {
                    break;
                }
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/n-triples\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )
        });
        let store = Store::new()?;
        let existing = Quad::new(
            NamedNode::new("urn:existing")?,
            NamedNode::new("urn:p")?,
            NamedNode::new("urn:o")?,
            GraphName::DefaultGraph,
        );
        store.insert(&existing)?;
        let graph = NamedNode::new("urn:loaded")?;
        let update = SparqlEvaluator::new()
            .with_http_timeout(Duration::from_secs(5))
            .parse_update(&format!(
                "LOAD {} <http://{address}/> INTO GRAPH {graph}",
                if silent { "SILENT" } else { "" }
            ))?;
        let result = if in_transaction {
            let mut transaction = store.start_transaction()?;
            let result = update.on_transaction(&mut transaction).execute();
            // Even if LOAD fails, committing must not persist a partial document.
            transaction.commit()?;
            result
        } else {
            update.on_store(&store).execute()
        };
        server
            .join()
            .map_err(|_| std::io::Error::other("HTTP server panicked"))??;
        assert_eq!(result.is_ok(), valid || silent);
        assert_eq!(store.len()?, 1);
        assert!(store.contains(&existing)?);
        assert_eq!(store.contains_named_graph(&graph)?, valid);
    }
    Ok(())
}
