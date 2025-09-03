#![cfg(test)]
#![allow(clippy::panic_in_result_fn)]

use oxigraph::io::RdfFormat;
use oxigraph::model::vocab::{rdf, xsd};
use oxigraph::model::*;
use oxigraph::store::Store;
use std::error::Error;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::fs::{File, create_dir_all, remove_dir, remove_dir_all};
#[cfg(all(target_os = "linux", feature = "rocksdb"))]
use std::fs::{read, read_dir, write};
#[cfg(all(target_os = "linux", feature = "rocksdb"))]
use std::io;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::io::Write;
use std::iter::empty;
#[cfg(all(target_os = "linux", feature = "rocksdb"))]
use std::iter::once;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use std::path::PathBuf;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use tempfile::TempDir;

#[expect(clippy::non_ascii_literal)]
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

#[expect(clippy::non_ascii_literal)]
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
            LiteralRef::new_language_tagged_literal_unchecked("la ville lumi\u{E8}re", "fr"),
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
    store.load_from_reader(RdfFormat::Turtle, DATA.as_bytes())?;
    for q in quads(GraphNameRef::DefaultGraph) {
        assert!(store.contains(q)?);
    }
    store.validate()?;
    Ok(())
}

#[test]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_load_graph_on_disk() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    let store = Store::open(&dir)?;
    store.load_from_reader(RdfFormat::Turtle, DATA.as_bytes())?;
    for q in quads(GraphNameRef::DefaultGraph) {
        assert!(store.contains(q)?);
    }
    store.validate()?;
    Ok(())
}

#[test]
fn test_bulk_load_graph() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    let mut loader = store.bulk_loader();
    loader.load_from_slice(RdfFormat::Turtle, DATA.as_bytes())?;
    loader.commit()?;
    for q in quads(GraphNameRef::DefaultGraph) {
        assert!(store.contains(q)?);
    }
    store.validate()?;
    Ok(())
}

#[test]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_bulk_load_graph_on_disk() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    let store = Store::open(&dir)?;
    let mut loader = store.bulk_loader();
    loader.load_from_slice(RdfFormat::Turtle, DATA.as_bytes())?;
    loader.commit()?;
    for q in quads(GraphNameRef::DefaultGraph) {
        assert!(store.contains(q)?);
    }
    store.validate()?;
    Ok(())
}

#[test]
fn test_bulk_load_graph_lenient() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    let mut loader = store.bulk_loader().on_parse_error(|_| Ok(()));
    loader.load_from_slice(
        RdfFormat::NTriples,
        b"<http://example.com> <http://example.com> <http://example.com##> .\n<http://example.com> <http://example.com> <http://example.com> .".as_slice(),
    )?;
    loader.commit()?;
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
fn test_bulk_load_empty() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    let mut loader = store.bulk_loader();
    loader.load_quads(empty::<Quad>())?;
    loader.commit()?;
    assert!(store.is_empty()?);
    store.validate()?;
    Ok(())
}

#[test]
fn test_load_dataset() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    store.load_from_reader(RdfFormat::TriG, GRAPH_DATA.as_bytes())?;
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
    let store = Store::new()?;
    let mut loader = store.bulk_loader();
    loader.load_from_slice(RdfFormat::TriG, GRAPH_DATA.as_bytes())?;
    loader.commit()?;
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
        store.load_from_reader(
            RdfFormat::NTriples,
            "_:a <http://example.com/p> <http://example.com/p> .".as_bytes(),
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
    store.dump_graph_to_writer(GraphNameRef::DefaultGraph, RdfFormat::NTriples, &mut buffer)?;
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

    let buffer = store.dump_to_writer(RdfFormat::NQuads, Vec::new())?;
    assert_eq!(
        buffer.into_iter().filter(|c| *c == b'\n').count(),
        NUMBER_OF_TRIPLES
    );
    Ok(())
}

#[test]
fn test_snapshot_isolation_iterator() -> Result<(), Box<dyn Error>> {
    let quad = QuadRef::new(
        NamedNodeRef::new("http://example.com/s")?,
        NamedNodeRef::new("http://example.com/p")?,
        NamedNodeRef::new("http://example.com/o")?,
        NamedNodeRef::new("http://www.wikidata.org/wiki/Special:EntityData/Q90")?,
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
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_snapshot_isolation_iterator_on_disk() -> Result<(), Box<dyn Error>> {
    let quad = QuadRef::new(
        NamedNodeRef::new("http://example.com/s")?,
        NamedNodeRef::new("http://example.com/p")?,
        NamedNodeRef::new("http://example.com/o")?,
        NamedNodeRef::new("http://www.wikidata.org/wiki/Special:EntityData/Q90")?,
    );
    let dir = TempDir::new()?;
    let store = Store::open(&dir)?;
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
fn test_bulk_load_on_existing_delete_overrides_the_delete() -> Result<(), Box<dyn Error>> {
    let quad = QuadRef::new(
        NamedNodeRef::new_unchecked("http://example.com/s"),
        NamedNodeRef::new_unchecked("http://example.com/p"),
        NamedNodeRef::new_unchecked("http://example.com/o"),
        NamedNodeRef::new_unchecked("http://www.wikidata.org/wiki/Special:EntityData/Q90"),
    );
    let store = Store::new()?;
    store.remove(quad)?;
    let mut loader = store.bulk_loader();
    loader.load_quads([quad.into_owned()])?;
    loader.commit()?;
    assert_eq!(store.len()?, 1);
    Ok(())
}

#[test]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_bulk_load_on_existing_delete_overrides_the_delete_on_disk() -> Result<(), Box<dyn Error>> {
    let quad = QuadRef::new(
        NamedNodeRef::new_unchecked("http://example.com/s"),
        NamedNodeRef::new_unchecked("http://example.com/p"),
        NamedNodeRef::new_unchecked("http://example.com/o"),
        NamedNodeRef::new_unchecked("http://www.wikidata.org/wiki/Special:EntityData/Q90"),
    );
    let dir = TempDir::new()?;
    let store = Store::open(&dir)?;
    store.remove(quad)?;
    let mut loader = store.bulk_loader();
    loader.load_quads([quad.into_owned()])?;
    loader.commit()?;
    assert_eq!(store.len()?, 1);
    Ok(())
}

#[test]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_open_bad_dir() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    create_dir_all(&dir)?;
    {
        File::create(dir.as_ref().join("CURRENT"))?.write_all(b"foo")?;
    }
    assert!(Store::open(&dir).is_err());
    Ok(())
}

#[test]
#[cfg(all(target_os = "linux", feature = "rocksdb"))]
fn test_bad_stt_open() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    let store = Store::open(&dir)?;
    remove_dir_all(&dir)?;
    let mut loader = store.bulk_loader();
    loader.load_quads(once(Quad::new(
        NamedNode::new_unchecked("http://example.com/s"),
        NamedNode::new_unchecked("http://example.com/p"),
        NamedNode::new_unchecked("http://example.com/o"),
        GraphName::DefaultGraph,
    )))?;
    loader.commit().unwrap_err();
    Ok(())
}

#[test]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_backup() -> Result<(), Box<dyn Error>> {
    let quad = QuadRef::new(
        NamedNodeRef::new_unchecked("http://example.com/s"),
        NamedNodeRef::new_unchecked("http://example.com/p"),
        NamedNodeRef::new_unchecked("http://example.com/o"),
        GraphNameRef::DefaultGraph,
    );
    let store_dir = TempDir::new()?;
    let backup_from_rw_dir = TempDir::new()?;
    remove_dir(&backup_from_rw_dir)?;
    let backup_from_ro_dir = TempDir::new()?;
    remove_dir(&backup_from_ro_dir)?;

    let store = Store::open(&store_dir)?;
    store.insert(quad)?;
    store.backup(&backup_from_rw_dir)?;
    store.remove(quad)?;
    assert!(!store.contains(quad)?);

    let backup_from_rw = Store::open_read_only(&backup_from_rw_dir)?;
    backup_from_rw.validate()?;
    assert!(backup_from_rw.contains(quad)?);
    backup_from_rw.backup(&backup_from_ro_dir)?;

    let backup_from_ro = Store::open_read_only(&backup_from_ro_dir)?;
    backup_from_ro.validate()?;
    assert!(backup_from_ro.contains(quad)?);

    Ok(())
}

#[test]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_bad_backup() -> Result<(), Box<dyn Error>> {
    let store_dir = TempDir::new()?;
    let backup_dir = TempDir::new()?;

    create_dir_all(&backup_dir)?;
    Store::open(&store_dir)?.backup(&backup_dir).unwrap_err();
    Ok(())
}

#[test]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_backup_on_in_memory() -> Result<(), Box<dyn Error>> {
    let backup_dir = TempDir::new()?;
    Store::new()?.backup(&backup_dir).unwrap_err();
    Ok(())
}

#[test]
#[cfg(all(target_os = "linux", feature = "rocksdb"))]
fn test_backward_compatibility() -> Result<(), Box<dyn Error>> {
    // We run twice to check if data is properly saved and closed
    let _reset = DirSaver::new("tests/rocksdb_bc_data")?;
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
    Ok(())
}

#[test]
#[cfg(all(target_os = "linux", feature = "rocksdb", feature = "rdf-12"))]
fn test_rdf_star_backward_compatibility() -> Result<(), Box<dyn Error>> {
    // We run twice to check if data is properly saved and closed
    let _reset = DirSaver::new("tests/rocksdb_bc_rdf_star_data")?;
    let s = NamedNodeRef::new_unchecked("http://example.com/s");
    let p = NamedNodeRef::new_unchecked("http://example.com/p");
    let o = NamedNodeRef::new_unchecked("http://example.com/o");
    let g = NamedNodeRef::new_unchecked("http://example.com/g");
    let bnode = BlankNodeRef::new_unchecked("f2fef82410957224105241225fd0a648");
    for _ in 0..2 {
        let store = Store::open("tests/rocksdb_bc_rdf_star_data")?;
        assert!(store.contains(QuadRef::new(s, p, o, g))?);
        assert!(store.contains(QuadRef::new(s, p, o, GraphNameRef::DefaultGraph))?);
        assert!(store.contains(QuadRef::new(bnode, p, o, g))?);
        assert!(store.contains(QuadRef::new(bnode, p, o, GraphNameRef::DefaultGraph))?);
        assert!(store.contains(QuadRef::new(s, p, bnode, g))?);
        assert!(store.contains(QuadRef::new(s, p, bnode, GraphNameRef::DefaultGraph))?);
        assert!(store.contains(QuadRef::new(
            bnode,
            rdf::REIFIES,
            &Term::from(Triple::new(s, p, o)),
            g
        ))?);
        assert!(store.contains(QuadRef::new(
            bnode,
            rdf::REIFIES,
            &Term::from(Triple::new(s, p, o)),
            GraphNameRef::DefaultGraph
        ))?);
    }
    Ok(())
}

#[test]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
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
    let store_dir = TempDir::new()?;

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
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_open_read_only_bad_dir() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    create_dir_all(&dir)?;
    {
        File::create(dir.as_ref().join("CURRENT"))?.write_all(b"foo")?;
    }
    assert!(Store::open_read_only(&dir).is_err());
    Ok(())
}

#[cfg(all(target_os = "linux", feature = "rocksdb"))]
struct DirSaver {
    path: PathBuf,
    elements: Vec<(PathBuf, Vec<u8>)>,
}

#[cfg(all(target_os = "linux", feature = "rocksdb"))]
impl DirSaver {
    fn new(path: &str) -> io::Result<Self> {
        Ok(Self {
            path: path.into(),
            elements: read_dir(path)?
                .map(|item| {
                    let path = item?.path();
                    let content = read(&path)?;
                    Ok((path, content))
                })
                .collect::<io::Result<Vec<_>>>()?,
        })
    }
}

#[cfg(all(target_os = "linux", feature = "rocksdb"))]
impl Drop for DirSaver {
    fn drop(&mut self) {
        remove_dir_all(&self.path).unwrap();
        create_dir_all(&self.path).unwrap();
        for (path, content) in &self.elements {
            write(path, content).unwrap();
        }
    }
}
