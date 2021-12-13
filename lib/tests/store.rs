use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::model::*;
use oxigraph::store::Store;
use std::io::{Cursor, Result};
use std::process::Command;

const DATA: &str = r#"
@prefix schema: <http://schema.org/> .
@prefix wd: <http://www.wikidata.org/entity/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

wd:Q90 a schema:City ;
    schema:name "Paris"@fr , "la ville lumière"@fr ;
    schema:country wd:Q142 ;
    schema:population 2000000 ;
    schema:startDate "-300"^^xsd:gYear ;
    schema:url "https://www.paris.fr/"^^xsd:anyURI ;
    schema:postalCode "75001" .
"#;
const NUMBER_OF_TRIPLES: usize = 8;

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
fn test_load_graph() -> Result<()> {
    let store = Store::new()?;
    store.load_graph(
        Cursor::new(DATA),
        GraphFormat::Turtle,
        GraphNameRef::DefaultGraph,
        None,
    )?;
    for q in quads(GraphNameRef::DefaultGraph) {
        assert!(store.contains(q)?);
    }
    Ok(())
}

#[test]
fn test_load_dataset() -> Result<()> {
    let store = Store::new()?;
    store.load_dataset(Cursor::new(DATA), DatasetFormat::TriG, None)?;
    for q in quads(GraphNameRef::DefaultGraph) {
        assert!(store.contains(q)?);
    }
    Ok(())
}

#[test]
fn test_bulk_load_dataset() -> Result<()> {
    let store = Store::new().unwrap();
    store.bulk_load_dataset(Cursor::new(DATA), DatasetFormat::TriG, None)?;
    for q in quads(GraphNameRef::DefaultGraph) {
        assert!(store.contains(q)?);
    }
    Ok(())
}

#[test]
fn test_load_graph_generates_new_blank_nodes() -> Result<()> {
    let store = Store::new()?;
    for _ in 0..2 {
        store.load_graph(
            Cursor::new("_:a <http://example.com/p> <http://example.com/p> ."),
            GraphFormat::NTriples,
            GraphNameRef::DefaultGraph,
            None,
        )?;
    }
    assert_eq!(store.len()?, 2);
    Ok(())
}

#[test]
fn test_dump_graph() -> Result<()> {
    let store = Store::new()?;
    for q in quads(GraphNameRef::DefaultGraph) {
        store.insert(q)?;
    }

    let mut buffer = Vec::new();
    store.dump_graph(
        &mut buffer,
        GraphFormat::NTriples,
        GraphNameRef::DefaultGraph,
    )?;
    assert_eq!(
        buffer.into_iter().filter(|c| *c == b'\n').count(),
        NUMBER_OF_TRIPLES
    );
    Ok(())
}

#[test]
fn test_dump_dataset() -> Result<()> {
    let store = Store::new()?;
    for q in quads(GraphNameRef::DefaultGraph) {
        store.insert(q)?;
    }

    let mut buffer = Vec::new();
    store.dump_dataset(&mut buffer, DatasetFormat::NQuads)?;
    assert_eq!(
        buffer.into_iter().filter(|c| *c == b'\n').count(),
        NUMBER_OF_TRIPLES
    );
    Ok(())
}

#[test]
fn test_snapshot_isolation_iterator() -> Result<()> {
    let quad = QuadRef::new(
        NamedNodeRef::new_unchecked("http://example.com/s"),
        NamedNodeRef::new_unchecked("http://example.com/p"),
        NamedNodeRef::new_unchecked("http://example.com/o"),
        NamedNodeRef::new_unchecked("http://example.com/g"),
    );
    let store = Store::new()?;
    store.insert(quad)?;
    let iter = store.iter();
    store.remove(quad)?;
    assert_eq!(iter.collect::<Result<Vec<_>>>()?, vec![quad.into_owned()]);
    Ok(())
}

#[test]
fn test_bulk_load_on_existing_delete_overrides_the_delete() -> Result<()> {
    let quad = QuadRef::new(
        NamedNodeRef::new_unchecked("http://example.com/s"),
        NamedNodeRef::new_unchecked("http://example.com/p"),
        NamedNodeRef::new_unchecked("http://example.com/o"),
        NamedNodeRef::new_unchecked("http://example.com/g"),
    );
    let store = Store::new()?;
    store.remove(quad)?;
    store.bulk_extend([quad.into_owned()])?;
    assert_eq!(store.len()?, 1);
    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn test_backward_compatibility() -> Result<()> {
    // We run twice to check if data is properly saved and closed
    for _ in 0..2 {
        let store = Store::open("tests/rocksdb_bc_data")?;
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
            store.named_graphs().collect::<Result<Vec<_>>>()?
        );
    }
    reset_dir("tests/rocksdb_bc_data")?;
    Ok(())
}

fn reset_dir(dir: &str) -> Result<()> {
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
