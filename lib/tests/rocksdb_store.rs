use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::model::*;
use oxigraph::RocksDbStore;
use std::io;
use std::process::Command;

fn quads(graph_name: impl Into<GraphNameRef<'static>>) -> Vec<QuadRef<'static>> {
    let graph_name = graph_name.into();
    let paris = NamedNodeRef::new_unchecked("http://www.wikidata.org/entity/Q90");
    let france = NamedNodeRef::new_unchecked("http://www.wikidata.org/entity/Q142");
    let city = NamedNodeRef::new_unchecked("http://schema.org/City");
    let name = NamedNodeRef::new_unchecked("http://schema.org/name");
    let country = NamedNodeRef::new_unchecked("http://schema.org/country");
    let population = NamedNodeRef::new_unchecked("http://schema.org/population");
    let start_date = NamedNodeRef::new_unchecked("http://schema.org/startDate");
    let url = NamedNodeRef::new_unchecked("http://schema.org/url");
    let postal_code = NamedNodeRef::new_unchecked("http://schema.org/postalCode");
    vec![
        QuadRef::new(paris, rdf::TYPE, city, graph_name),
        QuadRef::new(
            paris,
            name,
            LiteralRef::new_language_tagged_literal_unchecked("Paris", "fr"),
            graph_name,
        ),
        QuadRef::new(
            paris,
            name,
            LiteralRef::new_language_tagged_literal_unchecked("la ville lumière", "fr"),
            graph_name,
        ),
        QuadRef::new(paris, country, france, graph_name),
        QuadRef::new(
            paris,
            population,
            LiteralRef::new_typed_literal("2000000", xsd::INTEGER),
            graph_name,
        ),
        QuadRef::new(
            paris,
            start_date,
            LiteralRef::new_typed_literal("-300", xsd::G_YEAR),
            graph_name,
        ),
        QuadRef::new(
            paris,
            url,
            LiteralRef::new_typed_literal("https://www.paris.fr/", xsd::ANY_URI),
            graph_name,
        ),
        QuadRef::new(
            paris,
            postal_code,
            LiteralRef::new_simple_literal("75001"),
            graph_name,
        ),
    ]
}

#[test]
fn test_backward_compatibility() -> io::Result<()> {
    {
        let store = RocksDbStore::open("tests/rockdb_bc_data")?;
        for q in quads(GraphNameRef::DefaultGraph) {
            assert!(store.contains(q)?);
        }
        let graph_name =
            NamedNodeRef::new_unchecked("http://www.wikidata.org/wiki/Special:EntityData/Q90");
        for q in quads(graph_name) {
            assert!(store.contains(q)?);
        }
        assert!(store.contains_named_graph(graph_name)?);
        assert_eq!(
            vec![NamedOrBlankNode::from(graph_name)],
            store.named_graphs().collect::<io::Result<Vec<_>>>()?
        );
    };
    reset_dir("tests/rockdb_bc_data")?;
    Ok(())
}

fn reset_dir(dir: &str) -> io::Result<()> {
    assert!(Command::new("git")
        .args(&["clean", "-fX", dir])
        .status()?
        .success());
    assert!(Command::new("git")
        .args(&["checkout", "HEAD", "--", dir])
        .status()?
        .success());
    Ok(())
}
