#![cfg(feature = "datafusion")]
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use oxrdf::{BlankNodeRef, GraphNameRef, NamedNodeRef, QuadRef};

#[test]
fn test_datafusion() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;
    store.insert(QuadRef::new(
        NamedNodeRef::new("http://example.org/a")?,
        NamedNodeRef::new("http://example.org/p")?,
        BlankNodeRef::new("b")?,
        GraphNameRef::DefaultGraph,
    ))?;
    store.insert(QuadRef::new(
        BlankNodeRef::new("b")?,
        NamedNodeRef::new("http://example.org/p")?,
        NamedNodeRef::new("http://example.org/c")?,
        GraphNameRef::DefaultGraph,
    ))?;
    let query = "\
DESCRIBE <http://example.org/a>";
    let result = SparqlEvaluator::new()
        .parse_query(query)?
        .datafusion_explain(&store)?;
    println!("{}", result);
    match SparqlEvaluator::new()
        .parse_query(query)?
        .datafusion(&store)?
    {
        QueryResults::Solutions(results) => {
            for result in results {
                println!("{:?}", result?);
            }
        }
        QueryResults::Boolean(result) => println!("{result}"),
        QueryResults::Graph(results) => {
            for result in results {
                println!("{}", result?);
            }
        }
    }
    Ok(())
}
