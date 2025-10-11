#![cfg(feature = "datafusion")]
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use oxrdf::{GraphNameRef, NamedNodeRef, QuadRef};

#[test]
fn test_datafusion() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;
    store.insert(QuadRef::new(
        NamedNodeRef::new("http://example.org/s")?,
        NamedNodeRef::new("http://example.org/p")?,
        NamedNodeRef::new("http://example.org/o")?,
        GraphNameRef::DefaultGraph,
    ))?;
    let query = "SELECT ?v WHERE { VALUES ?v { 1 2 3 } } ORDER BY ASC(?v) LIMIT 100";
    if let Some(result) = SparqlEvaluator::new()
        .parse_query(query)?
        .datafusion_explain(&store)?
    {
        println!("{}", result);
    }
    match SparqlEvaluator::new()
        .parse_query(query)?
        .datafusion(&store)?
    {
        Some(QueryResults::Solutions(results)) => {
            for result in results {
                println!("{:?}", result?);
            }
        }
        Some(QueryResults::Boolean(result)) => println!("{result}"),
        _ => (),
    }
    Ok(())
}
