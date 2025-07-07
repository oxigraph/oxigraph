#![cfg(feature = "datafusion")]

use futures::stream::StreamExt;
use oxigraph::sparql::{DatafusionQueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use oxrdf::{GraphNameRef, NamedNodeRef, QuadRef};

#[tokio::test]
async fn test_datafusion() -> Result<(), Box<dyn std::error::Error>> {
    let store = Store::new()?;
    store.insert(QuadRef::new(
        NamedNodeRef::new("http://example.org/s")?,
        NamedNodeRef::new("http://example.org/p")?,
        NamedNodeRef::new("http://example.org/o")?,
        GraphNameRef::DefaultGraph,
    ))?;
    match SparqlEvaluator::new()
        .parse_query("ASK  {  {  }  UNION  { SELECT  ?1  {  }  }  }")?
        .datafusion(&store)
        .await?
    {
        DatafusionQueryResults::Solutions(mut solutions) => {
            while let Some(solution) = solutions.next().await {
                panic!("{:?}", solution?);
            }
        }
        DatafusionQueryResults::Boolean(b) => assert!(b),
    }
    Ok(())
}
