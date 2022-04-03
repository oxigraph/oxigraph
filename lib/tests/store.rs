use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::model::*;
use oxigraph::store::Store;
use rand::random;
use std::env::temp_dir;
use std::error::Error;
use std::fs::{create_dir, remove_dir_all, File};
use std::io::{Cursor, Write};
use std::iter::once;
use std::path::PathBuf;
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
const GRAPH_DATA: &str = r#"
@prefix schema: <http://schema.org/> .
@prefix wd: <http://www.wikidata.org/entity/> .
@prefix xsd: <http://www.w3.org/2001/XMLSchema#> .

GRAPH <http://www.wikidata.org/wiki/Special:EntityData/Q90> {
    wd:Q90 a schema:City ;
        schema:name "Paris"@fr , "la ville lumière"@fr ;
        schema:country wd:Q142 ;
        schema:population 2000000 ;
        schema:startDate "-300"^^xsd:gYear ;
        schema:url "https://www.paris.fr/"^^xsd:anyURI ;
        schema:postalCode "75001" .
}
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
fn test_load_graph() -> Result<(), Box<dyn Error>> {
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
    store.validate()?;
    Ok(())
}

#[test]
fn test_bulk_load_graph() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    store.bulk_loader().load_graph(
        Cursor::new(DATA),
        GraphFormat::Turtle,
        GraphNameRef::DefaultGraph,
        None,
    )?;
    for q in quads(GraphNameRef::DefaultGraph) {
        assert!(store.contains(q)?);
    }
    store.validate()?;
    Ok(())
}

#[test]
fn test_bulk_load_graph_lenient() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    store.bulk_loader().on_parse_error(|_| Ok(())).load_graph(
        Cursor::new(b"<http://example.com> <http://example.com> <http://example.com##> .\n<http://example.com> <http://example.com> <http://example.com> ."),
        GraphFormat::NTriples,
        GraphNameRef::DefaultGraph,
        None,
    )?;
    assert_eq!(store.len()?, 1);
    assert!(store.contains(QuadRef::new(
        NamedNodeRef::new_unchecked("http://example.com"),
        NamedNodeRef::new_unchecked("http://example.com"),
        NamedNodeRef::new_unchecked("http://example.com"),
        GraphNameRef::DefaultGraph
    ))?);
    store.validate()?;
    Ok(())
}

#[test]
fn test_load_dataset() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    store.load_dataset(Cursor::new(GRAPH_DATA), DatasetFormat::TriG, None)?;
    for q in quads(NamedNodeRef::new_unchecked(
        "http://www.wikidata.org/wiki/Special:EntityData/Q90",
    )) {
        assert!(store.contains(q)?);
    }
    store.validate()?;
    Ok(())
}

#[test]
fn test_bulk_load_dataset() -> Result<(), Box<dyn Error>> {
    let store = Store::new().unwrap();
    store
        .bulk_loader()
        .load_dataset(Cursor::new(GRAPH_DATA), DatasetFormat::TriG, None)?;
    let graph_name =
        NamedNodeRef::new_unchecked("http://www.wikidata.org/wiki/Special:EntityData/Q90");
    for q in quads(graph_name) {
        assert!(store.contains(q)?);
    }
    assert!(store.contains_named_graph(graph_name)?);
    store.validate()?;
    Ok(())
}

#[test]
fn test_load_graph_generates_new_blank_nodes() -> Result<(), Box<dyn Error>> {
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
fn test_dump_graph() -> Result<(), Box<dyn Error>> {
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
fn test_dump_dataset() -> Result<(), Box<dyn Error>> {
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
fn test_snapshot_isolation_iterator() -> Result<(), Box<dyn Error>> {
    let quad = QuadRef::new(
        NamedNodeRef::new_unchecked("http://example.com/s"),
        NamedNodeRef::new_unchecked("http://example.com/p"),
        NamedNodeRef::new_unchecked("http://example.com/o"),
        NamedNodeRef::new_unchecked("http://www.wikidata.org/wiki/Special:EntityData/Q90"),
    );
    let store = Store::new()?;
    store.insert(quad)?;
    let iter = store.iter();
    store.remove(quad)?;
    store.validate()?;
    assert_eq!(
        iter.collect::<Result<Vec<_>, _>>()?,
        vec![quad.into_owned()]
    );
    Ok(())
}

#[test]
fn test_bulk_load_on_existing_delete_overrides_the_delete() -> Result<(), Box<dyn Error>> {
    let quad = QuadRef::new(
        NamedNodeRef::new_unchecked("http://example.com/s"),
        NamedNodeRef::new_unchecked("http://example.com/p"),
        NamedNodeRef::new_unchecked("http://example.com/o"),
        NamedNodeRef::new_unchecked("http://www.wikidata.org/wiki/Special:EntityData/Q90"),
    );
    let store = Store::new()?;
    store.remove(quad)?;
    store.bulk_loader().load_quads([quad.into_owned()])?;
    assert_eq!(store.len()?, 1);
    Ok(())
}

#[test]
fn test_open_bad_dir() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::default();
    create_dir(&dir.0)?;
    {
        File::create(dir.0.join("CURRENT"))?.write_all(b"foo")?;
    }
    assert!(Store::open(&dir.0).is_err());
    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn test_bad_stt_open() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::default();
    let store = Store::open(&dir.0)?;
    remove_dir_all(&dir.0)?;
    assert!(store
        .bulk_loader()
        .load_quads(once(Quad {
            subject: NamedNode::new_unchecked("http://example.com/s").into(),
            predicate: NamedNode::new_unchecked("http://example.com/p"),
            object: NamedNode::new_unchecked("http://example.com/o").into(),
            graph_name: GraphName::DefaultGraph
        }))
        .is_err());
    Ok(())
}

#[test]
fn test_backup() -> Result<(), Box<dyn Error>> {
    let quad = QuadRef {
        subject: NamedNodeRef::new_unchecked("http://example.com/s").into(),
        predicate: NamedNodeRef::new_unchecked("http://example.com/p"),
        object: NamedNodeRef::new_unchecked("http://example.com/o").into(),
        graph_name: GraphNameRef::DefaultGraph,
    };
    let store_dir = TempDir::default();
    let backup_dir = TempDir::default();

    let store = Store::open(&store_dir.0)?;
    store.insert(quad)?;
    store.backup(&backup_dir.0)?;
    store.remove(quad)?;

    assert!(!store.contains(quad)?);
    let backup = Store::open(&backup_dir.0)?;
    backup.validate()?;
    assert!(backup.contains(quad)?);
    Ok(())
}

#[test]
fn test_bad_backup() -> Result<(), Box<dyn Error>> {
    let store_dir = TempDir::default();
    let backup_dir = TempDir::default();

    create_dir(&backup_dir.0)?;
    assert!(Store::open(&store_dir.0)?.backup(&backup_dir.0).is_err());
    Ok(())
}

#[test]
fn test_backup_on_in_memory() -> Result<(), Box<dyn Error>> {
    let backup_dir = TempDir::default();
    assert!(Store::new()?.backup(&backup_dir.0).is_err());
    Ok(())
}

#[test]
#[cfg(target_os = "linux")]
fn test_backward_compatibility() -> Result<(), Box<dyn Error>> {
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
            store.named_graphs().collect::<Result<Vec<_>, _>>()?
        );
    }
    reset_dir("tests/rocksdb_bc_data")?;
    Ok(())
}

fn reset_dir(dir: &str) -> Result<(), Box<dyn Error>> {
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

struct TempDir(PathBuf);

impl Default for TempDir {
    fn default() -> Self {
        Self(temp_dir().join(format!("oxigraph-test-{}", random::<u128>())))
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = remove_dir_all(&self.0);
    }
}
