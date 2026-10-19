//! Checks that the query optimizer uses the store statistics to choose the join order.
//!
//! The queries join a frequent predicate with a rare one, written frequent first.
//! The heuristics of the optimizer do not distinguish the two patterns, so without statistics the written order is kept.
//! With statistics, the evaluation must start from the rare predicate.

#![cfg(test)]
#![expect(clippy::panic, clippy::panic_in_result_fn)]

use oxigraph::model::{GraphName, NamedNode, Quad};
use oxigraph::sparql::SparqlEvaluator;
use oxigraph::store::Store;
use serde_json::Value;
use std::error::Error;
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
use tempfile::TempDir;

const QUERY: &str =
    "SELECT * WHERE { ?s <http://example.com/frequent> ?o . ?s <http://example.com/rare> ?r }";

fn insert_data(
    store: &Store,
    frequent_count: usize,
    rare_count: usize,
) -> Result<(), Box<dyn Error>> {
    let frequent = NamedNode::new("http://example.com/frequent")?;
    let rare = NamedNode::new("http://example.com/rare")?;
    let mut loader = store.bulk_loader();
    let mut quads = Vec::new();
    for i in 0..frequent_count {
        let subject = NamedNode::new(format!("http://example.com/s{i}"))?;
        quads.push(Quad::new(
            subject.clone(),
            frequent.clone(),
            NamedNode::new(format!("http://example.com/o{i}"))?,
            GraphName::DefaultGraph,
        ));
        if i < rare_count {
            quads.push(Quad::new(
                subject,
                rare.clone(),
                NamedNode::new(format!("http://example.com/r{i}"))?,
                GraphName::DefaultGraph,
            ));
        }
    }
    loader.load_quads(quads)?;
    loader.commit()?;
    Ok(())
}

/// The predicate of the first quad pattern evaluated by the plan
fn first_evaluated_predicate(store: &Store) -> Result<String, Box<dyn Error>> {
    fn first_quad_pattern(node: &Value) -> Option<String> {
        let name = node.get("name")?.as_str()?;
        if name.starts_with("QuadPattern(") {
            return Some(name.to_owned());
        }
        node.get("children")?
            .as_array()?
            .iter()
            .find_map(first_quad_pattern)
    }

    let (results, explanation) = SparqlEvaluator::new()
        .parse_query(QUERY)?
        .on_store(store)
        .explain();
    drop(results?);
    let mut buffer = Vec::new();
    explanation.write_in_json(&mut buffer)?;
    let json: Value = serde_json::from_slice(&buffer)?;
    let Some(pattern) = first_quad_pattern(&json["plan"]) else {
        panic!("No quad pattern in the plan {json:#}")
    };
    Ok(if pattern.contains("rare") {
        "rare".into()
    } else if pattern.contains("frequent") {
        "frequent".into()
    } else {
        panic!("Unexpected pattern {pattern}")
    })
}

#[test]
fn test_memory_store_statistics() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    insert_data(&store, 5_000, 1_500)?;
    // The in-memory store always has statistics
    assert_eq!(first_evaluated_predicate(&store)?, "rare");
    Ok(())
}

#[test]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_on_disk_store_statistics() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    {
        let store = Store::open(&dir)?;
        insert_data(&store, 5_000, 1_500)?;
        // There are too many triples to count them when planning: no statistics
        assert_eq!(first_evaluated_predicate(&store)?, "frequent");
        store.update_statistics()?;
        assert_eq!(first_evaluated_predicate(&store)?, "rare");
    }
    // The statistics are saved
    assert_eq!(first_evaluated_predicate(&Store::open(&dir)?)?, "rare");
    assert_eq!(
        first_evaluated_predicate(&Store::open_read_only(&dir)?)?,
        "rare"
    );
    Ok(())
}

#[test]
#[cfg(all(not(target_family = "wasm"), feature = "rocksdb"))]
fn test_on_disk_store_live_counts() -> Result<(), Box<dyn Error>> {
    let dir = TempDir::new()?;
    let store = Store::open(&dir)?;
    insert_data(&store, 5_000, 10)?;
    // The rare predicate is small enough to be counted when planning
    assert_eq!(first_evaluated_predicate(&store)?, "rare");
    Ok(())
}
