use oxigraph::sparql::dataset::HDTDatasetView;
use oxigraph::sparql::evaluate_hdt_query;
use oxigraph::sparql::QueryOptions;
use oxigraph::sparql::results::QueryResultsFormat;
use std::io;
use std::rc::Rc;

// Run with `cargo run --bin hdt_driver`.

// Based on oxigraph/lib/README.md, https://w3c.github.io/rdf-tests/,
// and https://www.w3.org/TR/sparql11-query/#WritingSimpleQueries.

fn main() {
    println!("Oxigraph/HDT - Driver for Testing");

    // Open the HDT file.
    let dataset = Rc::new(HDTDatasetView::new(vec![
        "oxhdt-sys/tests/resources/test.hdt".to_string(),
    ]));

    // Test
    println!();
    println!("Test");
    println!();

    let sparql_query = "SELECT ?o WHERE { <http://example.org/book/book1> ?p ?o }";

    let (results, _explain) = evaluate_hdt_query(
        Rc::clone(&dataset),
        sparql_query,
        QueryOptions::default(),
        false,
    )
    .expect("failed to evaluate SPARQL query");

    results
        .unwrap()
        .write(io::stdout(), QueryResultsFormat::Csv)
        .unwrap();

    // Test
    println!();
    println!("Test");
    println!();

    let sparql_query = "SELECT ?s WHERE { ?s <http://purl.org/dc/elements/1.1/title> ?o }";

    let (results, _explain) = evaluate_hdt_query(
        Rc::clone(&dataset),
        sparql_query,
        QueryOptions::default(),
        false,
    )
    .expect("failed to evaluate SPARQL query");

    results
        .unwrap()
        .write(io::stdout(), QueryResultsFormat::Csv)
        .unwrap();

    // Test
    println!();
    println!("Test");
    println!();

    let sparql_query = "SELECT ?s ?p ?o WHERE { <http://example.org/book/book1> ?p ?o . ?s ?p \"SPARQL Tutorial\" }";

    let (results, _explain) = evaluate_hdt_query(
        Rc::clone(&dataset),
        sparql_query,
        QueryOptions::default(),
        false,
    )
    .expect("failed to evaluate SPARQL query");

    results
        .unwrap()
        .write(io::stdout(), QueryResultsFormat::Csv)
        .unwrap();
}
