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
    let DatafusionQueryResults::Solutions(mut solutions) = SparqlEvaluator::new()
        .parse_query("SELECT ?x1 ?x2 ?x3 ?x4 WHERE { ?x1 <http://www.wikidata.org/prop/direct/P966> ?x2 .?x1 <http://www.wikidata.org/prop/direct/P3192> ?x3 .?x4 <http://www.wikidata.org/prop/direct/P434> ?x2 .?x4 <http://www.wikidata.org/prop/direct/P3265> ?x3 . } LIMIT 1000")?
        .datafusion(&store)
        .await?
    else {
        unreachable!();
    };
    while let Some(solution) = solutions.next().await {
        panic!("{:?}", solution?);
    }
    Ok(())
}
