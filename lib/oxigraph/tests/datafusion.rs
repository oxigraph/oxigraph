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
    let query = " SELECT * {  }  VALUES (   ?2   ?1   ?1  ) { ( UNDEF \"2020-01-01-14:00\"^^<http://www.w3.org/2001/XMLSchema#date> UNDEF ) (  <http://example.org/1>   <http://example.org/1>   <http://example.org/1>  ) (  <http://example.org/1>   <http://example.org/1>   <http://example.org/1>  ) }";
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
