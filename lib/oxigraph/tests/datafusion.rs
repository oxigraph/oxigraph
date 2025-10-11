#![cfg(feature = "datafusion")]
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use oxrdf::{GraphNameRef, NamedNodeRef, QuadRef};

#[test]
fn test_datafusion() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;
    store.insert(QuadRef::new(
        NamedNodeRef::new("http://example.org/a")?,
        NamedNodeRef::new("http://example.org/p")?,
        NamedNodeRef::new("http://example.org/b")?,
        GraphNameRef::DefaultGraph,
    ))?;
    store.insert(QuadRef::new(
        NamedNodeRef::new("http://example.org/b")?,
        NamedNodeRef::new("http://example.org/p")?,
        NamedNodeRef::new("http://example.org/c")?,
        GraphNameRef::DefaultGraph,
    ))?;
    let query = "\
SELECT ?X WHERE { ?X <http://example.org/p>* ?X . }";
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
