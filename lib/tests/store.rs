use oxigraph::io::{DatasetFormat, GraphFormat};
use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::model::*;
use oxigraph::store::Store;
#[cfg(not(target_family = "wasm"))]
use rand::random;
#[cfg(not(target_family = "wasm"))]
use std::env::temp_dir;
use std::error::Error;
#[cfg(not(target_family = "wasm"))]
use std::fs::{create_dir, remove_dir_all, File};
use std::io::Cursor;
#[cfg(not(target_family = "wasm"))]
use std::io::Write;
#[cfg(not(target_family = "wasm"))]
use std::iter::empty;
#[cfg(target_os = "linux")]
use std::iter::once;
#[cfg(not(target_family = "wasm"))]
use std::path::{Path, PathBuf};
#[cfg(target_os = "linux")]
use std::process::Command;

#[allow(clippy::non_ascii_literal)]
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

#[allow(clippy::non_ascii_literal)]
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
            LiteralRef::new_language_tagged_literal_unchecked("la ville lumi\u{e8}re", "fr"),
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
#[cfg(not(target_family = "wasm"))]
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
#[cfg(not(target_family = "wasm"))]
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
#[cfg(not(target_family = "wasm"))]
fn test_bulk_load_empty() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    store.bulk_loader().load_quads(empty::<Quad>())?;
    assert!(store.is_empty()?);
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
#[cfg(not(target_family = "wasm"))]
fn test_bulk_load_dataset() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
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
    assert_eq!(
        iter.collect::<Result<Vec<_>, _>>()?,
        vec![quad.into_owned()]
    );
    store.validate()?;
    Ok(())
}

#[test]
#[cfg(not(target_family = "wasm"))]
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
#[cfg(not(target_family = "wasm"))]
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
        .load_quads(once(Quad::new(
            NamedNode::new_unchecked("http://example.com/s"),
            NamedNode::new_unchecked("http://example.com/p"),
            NamedNode::new_unchecked("http://example.com/o"),
            GraphName::DefaultGraph
        )))
        .is_err());
    Ok(())
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn test_backup() -> Result<(), Box<dyn Error>> {
    let quad = QuadRef::new(
        NamedNodeRef::new_unchecked("http://example.com/s"),
        NamedNodeRef::new_unchecked("http://example.com/p"),
        NamedNodeRef::new_unchecked("http://example.com/o"),
        GraphNameRef::DefaultGraph,
    );
    let store_dir = TempDir::default();
    let backup_from_rw_dir = TempDir::default();
    let backup_from_ro_dir = TempDir::default();
    let backup_from_secondary_dir = TempDir::default();

    let store = Store::open(&store_dir)?;
    store.insert(quad)?;
    let secondary_store = Store::open_secondary(&store_dir)?;
    store.flush()?;

    store.backup(&backup_from_rw_dir)?;
    secondary_store.backup(&backup_from_secondary_dir)?;
    store.remove(quad)?;
    assert!(!store.contains(quad)?);

    let backup_from_rw = Store::open_read_only(&backup_from_rw_dir.0)?;
    backup_from_rw.validate()?;
    assert!(backup_from_rw.contains(quad)?);
    backup_from_rw.backup(&backup_from_ro_dir)?;

    let backup_from_ro = Store::open_read_only(&backup_from_ro_dir.0)?;
    backup_from_ro.validate()?;
    assert!(backup_from_ro.contains(quad)?);

    let backup_from_secondary = Store::open_read_only(&backup_from_secondary_dir.0)?;
    backup_from_secondary.validate()?;
    assert!(backup_from_secondary.contains(quad)?);

    Ok(())
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn test_bad_backup() -> Result<(), Box<dyn Error>> {
    let store_dir = TempDir::default();
    let backup_dir = TempDir::default();

    create_dir(&backup_dir.0)?;
    assert!(Store::open(&store_dir)?.backup(&backup_dir.0).is_err());
    Ok(())
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn test_backup_on_in_memory() -> Result<(), Box<dyn Error>> {
    let backup_dir = TempDir::default();
    assert!(Store::new()?.backup(&backup_dir).is_err());
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

#[test]
#[cfg(not(target_family = "wasm"))]
fn test_secondary() -> Result<(), Box<dyn Error>> {
    let quad = QuadRef::new(
        NamedNodeRef::new_unchecked("http://example.com/s"),
        NamedNodeRef::new_unchecked("http://example.com/p"),
        NamedNodeRef::new_unchecked("http://example.com/o"),
        GraphNameRef::DefaultGraph,
    );
    let primary_dir = TempDir::default();

    // We open the store
    let primary = Store::open(&primary_dir)?;
    let secondary = Store::open_secondary(&primary_dir)?;

    // We insert a quad
    primary.insert(quad)?;
    primary.flush()?;

    // It is readable from both stores
    for store in &[&primary, &secondary] {
        assert!(store.contains(quad)?);
        assert_eq!(
            store.iter().collect::<Result<Vec<_>, _>>()?,
            vec![quad.into_owned()]
        );
    }

    // We validate the states
    primary.validate()?;
    secondary.validate()?;

    // We close the primary store and remove its content
    drop(primary);
    remove_dir_all(&primary_dir)?;

    // We secondary store is still readable
    assert!(secondary.contains(quad)?);
    secondary.validate()?;

    Ok(())
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn test_open_secondary_bad_dir() -> Result<(), Box<dyn Error>> {
    let primary_dir = TempDir::default();
    create_dir(&primary_dir.0)?;
    {
        File::create(primary_dir.0.join("CURRENT"))?.write_all(b"foo")?;
    }
    assert!(Store::open_secondary(&primary_dir).is_err());
    Ok(())
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn test_read_only() -> Result<(), Box<dyn Error>> {
    let s = NamedNodeRef::new_unchecked("http://example.com/s");
    let p = NamedNodeRef::new_unchecked("http://example.com/p");
    let first_quad = QuadRef::new(
        s,
        p,
        NamedNodeRef::new_unchecked("http://example.com/o"),
        GraphNameRef::DefaultGraph,
    );
    let second_quad = QuadRef::new(
        s,
        p,
        NamedNodeRef::new_unchecked("http://example.com/o2"),
        GraphNameRef::DefaultGraph,
    );
    let store_dir = TempDir::default();

    // We write to the store and close it
    {
        let read_write = Store::open(&store_dir)?;
        read_write.insert(first_quad)?;
        read_write.flush()?;
    }

    // We open as read-only
    let read_only = Store::open_read_only(&store_dir)?;
    assert!(read_only.contains(first_quad)?);
    assert_eq!(
        read_only.iter().collect::<Result<Vec<_>, _>>()?,
        vec![first_quad.into_owned()]
    );
    read_only.validate()?;

    // We open as read-write again
    let read_write = Store::open(&store_dir)?;
    read_write.insert(second_quad)?;
    read_write.flush()?;
    read_write.optimize()?; // Makes sure it's well flushed

    // The new quad is in the read-write instance but not the read-only instance
    assert!(read_write.contains(second_quad)?);
    assert!(!read_only.contains(second_quad)?);
    read_only.validate()?;

    Ok(())
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn test_open_read_only_bad_dir() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::default();
    create_dir(&dir.0)?;
    {
        File::create(dir.0.join("CURRENT"))?.write_all(b"foo")?;
    }
    assert!(Store::open_read_only(&dir).is_err());
    Ok(())
}

#[cfg(target_os = "linux")]
fn reset_dir(dir: &str) -> Result<(), Box<dyn Error>> {
    assert!(Command::new("git")
        .args(["clean", "-fX", dir])
        .status()?
        .success());
    assert!(Command::new("git")
        .args(["checkout", "HEAD", "--", dir])
        .status()?
        .success());
    Ok(())
}

#[cfg(not(target_family = "wasm"))]
struct TempDir(PathBuf);

#[cfg(not(target_family = "wasm"))]
impl Default for TempDir {
    fn default() -> Self {
        Self(temp_dir().join(format!("oxigraph-test-{}", random::<u128>())))
    }
}

#[cfg(not(target_family = "wasm"))]
impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for TempDir {
    fn drop(&mut self) {
        if self.0.is_dir() {
            remove_dir_all(&self.0).unwrap();
        }
    }
}
