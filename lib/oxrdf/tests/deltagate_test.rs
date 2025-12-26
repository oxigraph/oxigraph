/// Tests for ΔGate protocol support in oxrdf
///
/// ΔGate is a protocol for computing and applying deltas (Δ) to RDF datasets.
/// These tests verify that the Dataset and Graph implementations support:
/// 1. Set operations (union, difference, intersection)
/// 2. Delta computation (diff)
/// 3. Delta application (apply_diff)
/// 4. Deterministic iteration for consistent hashing

use oxrdf::*;

#[test]
fn test_dataset_union() {
    let mut ds1 = Dataset::new();
    let ex1 = NamedNodeRef::new("http://example.com/1").unwrap();
    ds1.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));

    let mut ds2 = Dataset::new();
    let ex2 = NamedNodeRef::new("http://example.com/2").unwrap();
    ds2.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));

    let union = ds1.union(&ds2);
    assert_eq!(union.len(), 2);
    assert!(union.contains(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph)));
    assert!(union.contains(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph)));
}

#[test]
fn test_dataset_difference() {
    let mut ds1 = Dataset::new();
    let ex1 = NamedNodeRef::new("http://example.com/1").unwrap();
    let ex2 = NamedNodeRef::new("http://example.com/2").unwrap();
    ds1.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));
    ds1.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));

    let mut ds2 = Dataset::new();
    ds2.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));

    let diff = ds1.difference(&ds2);
    assert_eq!(diff.len(), 1);
    assert!(diff.contains(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph)));
    assert!(!diff.contains(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph)));
}

#[test]
fn test_dataset_intersection() {
    let mut ds1 = Dataset::new();
    let ex1 = NamedNodeRef::new("http://example.com/1").unwrap();
    let ex2 = NamedNodeRef::new("http://example.com/2").unwrap();
    ds1.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));
    ds1.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));

    let mut ds2 = Dataset::new();
    ds2.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));

    let intersection = ds1.intersection(&ds2);
    assert_eq!(intersection.len(), 1);
    assert!(intersection.contains(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph)));
}

#[test]
fn test_dataset_symmetric_difference() {
    let mut ds1 = Dataset::new();
    let ex1 = NamedNodeRef::new("http://example.com/1").unwrap();
    ds1.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));

    let mut ds2 = Dataset::new();
    let ex2 = NamedNodeRef::new("http://example.com/2").unwrap();
    ds2.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));

    let sym_diff = ds1.symmetric_difference(&ds2);
    assert_eq!(sym_diff.len(), 2);
    assert!(sym_diff.contains(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph)));
    assert!(sym_diff.contains(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph)));
}

#[test]
fn test_deltagate_diff_computation() {
    // Initial state (before)
    let mut before = Dataset::new();
    let ex1 = NamedNodeRef::new("http://example.com/1").unwrap();
    let ex2 = NamedNodeRef::new("http://example.com/2").unwrap();
    let ex3 = NamedNodeRef::new("http://example.com/3").unwrap();
    before.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));
    before.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));

    // Final state (after)
    let mut after = Dataset::new();
    after.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));
    after.insert(QuadRef::new(ex3, ex3, ex3, GraphNameRef::DefaultGraph));

    // Compute delta
    let (additions, removals) = before.diff(&after);

    // Verify Δ⁺ (additions)
    assert_eq!(additions.len(), 1);
    assert!(additions.contains(QuadRef::new(ex3, ex3, ex3, GraphNameRef::DefaultGraph)));

    // Verify Δ⁻ (removals)
    assert_eq!(removals.len(), 1);
    assert!(removals.contains(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph)));
}

#[test]
fn test_deltagate_apply_diff() {
    // Initial state
    let mut dataset = Dataset::new();
    let ex1 = NamedNodeRef::new("http://example.com/1").unwrap();
    dataset.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));

    // Define delta
    let mut additions = Dataset::new();
    let ex2 = NamedNodeRef::new("http://example.com/2").unwrap();
    additions.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));

    let mut removals = Dataset::new();
    removals.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));

    // Apply delta
    dataset.apply_diff(&additions, &removals);

    // Verify final state
    assert_eq!(dataset.len(), 1);
    assert!(dataset.contains(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph)));
    assert!(!dataset.contains(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph)));
}

#[test]
fn test_deltagate_roundtrip() {
    // Original state
    let mut original = Dataset::new();
    let ex1 = NamedNodeRef::new("http://example.com/1").unwrap();
    original.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));

    // Modified state
    let mut modified = Dataset::new();
    let ex2 = NamedNodeRef::new("http://example.com/2").unwrap();
    modified.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));

    // Compute delta
    let (additions, removals) = original.diff(&modified);

    // Apply delta to original
    let mut result = original.clone();
    result.apply_diff(&additions, &removals);

    // Verify roundtrip: result should equal modified
    assert_eq!(result, modified);
}

#[test]
fn test_deterministic_iteration_order() {
    // Create dataset with multiple quads
    let mut ds = Dataset::new();
    let ex1 = NamedNodeRef::new("http://example.com/1").unwrap();
    let ex2 = NamedNodeRef::new("http://example.com/2").unwrap();
    let ex3 = NamedNodeRef::new("http://example.com/3").unwrap();

    ds.insert(QuadRef::new(ex3, ex3, ex3, GraphNameRef::DefaultGraph));
    ds.insert(QuadRef::new(ex1, ex1, ex1, GraphNameRef::DefaultGraph));
    ds.insert(QuadRef::new(ex2, ex2, ex2, GraphNameRef::DefaultGraph));

    // Collect quads in iteration order
    let quads1: Vec<_> = ds.iter().collect();
    let quads2: Vec<_> = ds.iter().collect();

    // Verify iteration is deterministic (same order every time)
    assert_eq!(quads1, quads2);

    // Verify they're actually sorted (BTreeSet guarantees this)
    assert_eq!(quads1.len(), 3);
}

#[test]
fn test_graph_diff_operations() {
    // Test Graph diff operations (wrapping Dataset)
    let mut g1 = Graph::new();
    let ex1 = NamedNodeRef::new("http://example.com/1").unwrap();
    g1.insert(TripleRef::new(ex1, ex1, ex1));

    let mut g2 = Graph::new();
    let ex2 = NamedNodeRef::new("http://example.com/2").unwrap();
    g2.insert(TripleRef::new(ex2, ex2, ex2));

    // Test union
    let union = g1.union(&g2);
    assert_eq!(union.len(), 2);

    // Test diff
    let (additions, removals) = g1.diff(&g2);
    assert_eq!(additions.len(), 1);
    assert_eq!(removals.len(), 1);

    // Test apply_diff
    let mut g3 = g1.clone();
    g3.apply_diff(&additions, &removals);
    assert_eq!(g3, g2);
}

#[test]
fn test_deltagate_with_named_graphs() {
    // Test ΔGate operations with named graphs
    let mut ds1 = Dataset::new();
    let ex = NamedNodeRef::new("http://example.com").unwrap();
    let g1 = NamedNodeRef::new("http://graph.com/1").unwrap();
    let g2 = NamedNodeRef::new("http://graph.com/2").unwrap();

    ds1.insert(QuadRef::new(ex, ex, ex, g1));

    let mut ds2 = Dataset::new();
    ds2.insert(QuadRef::new(ex, ex, ex, g2));

    let (additions, removals) = ds1.diff(&ds2);
    assert_eq!(additions.len(), 1);
    assert_eq!(removals.len(), 1);

    // Verify the additions are in the correct graph
    assert!(additions.contains(QuadRef::new(ex, ex, ex, g2)));
    assert!(removals.contains(QuadRef::new(ex, ex, ex, g1)));
}

#[test]
fn test_empty_diff() {
    let ds1 = Dataset::new();
    let ds2 = Dataset::new();

    let (additions, removals) = ds1.diff(&ds2);
    assert!(additions.is_empty());
    assert!(removals.is_empty());
}

#[test]
fn test_idempotent_diff_application() {
    let mut ds = Dataset::new();
    let ex = NamedNodeRef::new("http://example.com").unwrap();
    ds.insert(QuadRef::new(ex, ex, ex, GraphNameRef::DefaultGraph));

    let (additions, removals) = ds.diff(&ds);

    // Applying a diff to itself should result in no change
    let original = ds.clone();
    ds.apply_diff(&additions, &removals);
    assert_eq!(ds, original);
}
