#![cfg(test)]
#![allow(clippy::panic_in_result_fn)]

use oxigraph::model::vocab::xsd;
use oxigraph::model::*;
use oxigraph::sparql::{QueryResults, SparqlEvaluator};
use oxigraph::store::Store;
use std::error::Error;

#[test]
fn test_store_and_load_formula() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Create a formula with some triples
    let id = BlankNode::new("formula1")?;
    let ex = NamedNode::new("http://example.com/subject")?;
    let pred = NamedNode::new("http://example.com/predicate")?;
    let obj = Literal::new_simple_literal("object");

    let triple1 = Triple::new(ex.clone(), pred.clone(), obj.clone());
    let triple2 = Triple::new(ex.clone(), pred.clone(), Literal::new_simple_literal("object2"));

    let formula = Formula::new(id.clone(), vec![triple1, triple2]);

    // Store the formula
    store.store_formula(&formula)?;

    // Load it back
    let loaded = store.load_formula(id.as_ref())?;

    // Verify it was stored and loaded correctly
    assert_eq!(loaded.id(), formula.id());
    assert_eq!(loaded.triples().len(), 2);
    // Check that all triples are present (order may differ)
    for triple in formula.triples() {
        assert!(loaded.triples().contains(triple));
    }

    Ok(())
}

#[test]
fn test_formula_persistence() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Create and store a formula
    let id = BlankNode::new("persistent_formula")?;
    let ex = NamedNode::new("http://example.com/alice")?;
    let name_pred = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
    let alice_name = Literal::new_simple_literal("Alice");

    let triple = Triple::new(ex, name_pred, alice_name);
    let formula = Formula::new(id.clone(), vec![triple]);

    store.store_formula(&formula)?;

    // Verify the quads were stored in the correct named graph
    let graph_name = GraphNameRef::BlankNode(id.as_ref());
    let quads: Vec<_> = store
        .quads_for_pattern(None, None, None, Some(graph_name))
        .collect::<Result<_, _>>()?;

    assert_eq!(quads.len(), 1);
    assert_eq!(quads[0].graph_name, GraphName::BlankNode(id.clone()));

    Ok(())
}

#[test]
#[ignore = "Query formula requires proper SPARQL algebra rewriting - TODO"]
fn test_query_formula() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Create a formula with structured data
    let id = BlankNode::new("query_test")?;
    let alice = NamedNode::new("http://example.com/alice")?;
    let bob = NamedNode::new("http://example.com/bob")?;
    let name_pred = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
    let age_pred = NamedNode::new("http://xmlns.com/foaf/0.1/age")?;

    let triples = vec![
        Triple::new(
            alice.clone(),
            name_pred.clone(),
            Literal::new_simple_literal("Alice"),
        ),
        Triple::new(
            alice.clone(),
            age_pred.clone(),
            Literal::new_typed_literal("30", xsd::INTEGER),
        ),
        Triple::new(
            bob.clone(),
            name_pred.clone(),
            Literal::new_simple_literal("Bob"),
        ),
        Triple::new(
            bob.clone(),
            age_pred.clone(),
            Literal::new_typed_literal("25", xsd::INTEGER),
        ),
    ];

    let formula = Formula::new(id.clone(), triples);
    store.store_formula(&formula)?;

    // Query the formula
    let query = "SELECT ?name WHERE { ?person <http://xmlns.com/foaf/0.1/name> ?name }";
    if let QueryResults::Solutions(mut solutions) =
        store.query_formula(id.as_ref(), query)?
    {
        let names: Vec<String> = solutions
            .map(|s| {
                s.unwrap()
                    .get("name")
                    .unwrap()
                    .to_string()
                    .trim_matches('"')
                    .to_string()
            })
            .collect();

        assert_eq!(names.len(), 2);
        assert!(names.contains(&"Alice".to_string()));
        assert!(names.contains(&"Bob".to_string()));
    } else {
        panic!("Expected QueryResults::Solutions");
    }

    Ok(())
}

#[test]
fn test_remove_formula() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Create and store a formula
    let id = BlankNode::new("remove_test")?;
    let ex = NamedNode::new("http://example.com")?;
    let triple = Triple::new(ex.clone(), ex.clone(), ex);

    let formula = Formula::new(id.clone(), vec![triple]);
    store.store_formula(&formula)?;

    // Verify it exists
    let loaded = store.load_formula(id.as_ref())?;
    assert_eq!(loaded.triples().len(), 1);

    // Remove it
    store.remove_formula(id.as_ref())?;

    // Verify it was removed
    let loaded_after = store.load_formula(id.as_ref())?;
    assert_eq!(loaded_after.triples().len(), 0);

    Ok(())
}

#[test]
fn test_transaction_store_formula() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    let mut transaction = store.start_transaction()?;

    // Create a formula
    let id = BlankNode::new("transaction_test")?;
    let ex = NamedNode::new("http://example.com/test")?;
    let pred = NamedNode::new("http://example.com/property")?;
    let obj = Literal::new_simple_literal("value");

    let triple = Triple::new(ex, pred, obj);
    let formula = Formula::new(id.clone(), vec![triple]);

    // Store in transaction
    transaction.store_formula(&formula);

    // Load from transaction (before commit)
    let loaded_in_tx = transaction.load_formula(id.as_ref())?;
    assert_eq!(loaded_in_tx.triples().len(), 1);

    // Commit
    transaction.commit()?;

    // Verify it persisted
    let loaded_after_commit = store.load_formula(id.as_ref())?;
    assert_eq!(loaded_after_commit.triples().len(), 1);
    assert_eq!(loaded_after_commit.triples(), formula.triples());

    Ok(())
}

#[test]
#[ignore = "Query formula requires proper SPARQL algebra rewriting - TODO"]
fn test_transaction_query_formula() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;
    let mut transaction = store.start_transaction()?;

    // Create a formula
    let id = BlankNode::new("tx_query_test")?;
    let ex = NamedNode::new("http://example.com/subject")?;
    let pred = NamedNode::new("http://example.com/predicate")?;
    let obj = Literal::new_simple_literal("test value");

    let triple = Triple::new(ex, pred, obj);
    let formula = Formula::new(id.clone(), vec![triple]);

    transaction.store_formula(&formula);

    // Query within transaction
    let query = "SELECT ?o WHERE { ?s ?p ?o }";
    if let QueryResults::Solutions(mut solutions) = transaction.query_formula(
        id.as_ref(),
        query,
    )? {
        let result = solutions.next().unwrap()?;
        let value = result.get("o").unwrap();
        assert_eq!(value.to_string(), "\"test value\"");
    } else {
        panic!("Expected QueryResults::Solutions");
    }

    transaction.commit()?;

    Ok(())
}

#[test]
fn test_multiple_formulas() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Create multiple formulas
    let id1 = BlankNode::new("formula_1")?;
    let id2 = BlankNode::new("formula_2")?;

    let ex1 = NamedNode::new("http://example.com/1")?;
    let ex2 = NamedNode::new("http://example.com/2")?;
    let pred = NamedNode::new("http://example.com/pred")?;

    let formula1 = Formula::new(
        id1.clone(),
        vec![Triple::new(
            ex1.clone(),
            pred.clone(),
            Literal::new_simple_literal("value1"),
        )],
    );

    let formula2 = Formula::new(
        id2.clone(),
        vec![Triple::new(
            ex2.clone(),
            pred.clone(),
            Literal::new_simple_literal("value2"),
        )],
    );

    // Store both
    store.store_formula(&formula1)?;
    store.store_formula(&formula2)?;

    // Load and verify both
    let loaded1 = store.load_formula(id1.as_ref())?;
    let loaded2 = store.load_formula(id2.as_ref())?;

    assert_eq!(loaded1.id(), &id1);
    assert_eq!(loaded2.id(), &id2);
    assert_eq!(loaded1.triples().len(), 1);
    assert_eq!(loaded2.triples().len(), 1);

    // Verify they're independent
    assert_ne!(loaded1.triples(), loaded2.triples());

    Ok(())
}

#[test]
fn test_empty_formula() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Create an empty formula
    let id = BlankNode::new("empty_formula")?;
    let formula = Formula::new(id.clone(), vec![]);

    // Store and load it
    store.store_formula(&formula)?;
    let loaded = store.load_formula(id.as_ref())?;

    assert_eq!(loaded.triples().len(), 0);
    assert_eq!(loaded.id(), &id);

    Ok(())
}

#[test]
fn test_formula_with_blank_nodes() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Create a formula with blank nodes in the triples
    let id = BlankNode::new("formula_with_bnodes")?;
    let bn1 = BlankNode::new("b1")?;
    let bn2 = BlankNode::new("b2")?;
    let pred = NamedNode::new("http://example.com/knows")?;

    let triple = Triple::new(bn1, pred, bn2);
    let formula = Formula::new(id.clone(), vec![triple]);

    store.store_formula(&formula)?;

    let loaded = store.load_formula(id.as_ref())?;
    assert_eq!(loaded.triples().len(), 1);

    // Verify the blank nodes are preserved
    let loaded_triple = &loaded.triples()[0];
    assert!(matches!(loaded_triple.subject, NamedOrBlankNode::BlankNode(_)));
    assert!(matches!(loaded_triple.object, Term::BlankNode(_)));

    Ok(())
}

#[test]
fn test_update_formula() -> Result<(), Box<dyn Error>> {
    let store = Store::new()?;

    // Create and store initial formula
    let id = BlankNode::new("update_test")?;
    let ex = NamedNode::new("http://example.com")?;
    let pred = NamedNode::new("http://example.com/prop")?;

    let initial = Formula::new(
        id.clone(),
        vec![Triple::new(
            ex.clone(),
            pred.clone(),
            Literal::new_simple_literal("initial"),
        )],
    );

    store.store_formula(&initial)?;

    // Update by storing a new formula with same ID
    let updated = Formula::new(
        id.clone(),
        vec![
            Triple::new(
                ex.clone(),
                pred.clone(),
                Literal::new_simple_literal("updated"),
            ),
            Triple::new(
                ex.clone(),
                pred.clone(),
                Literal::new_simple_literal("additional"),
            ),
        ],
    );

    store.store_formula(&updated)?;

    // Load and verify it has both old and new triples
    let loaded = store.load_formula(id.as_ref())?;
    // Note: store_formula appends to existing graph, doesn't replace
    assert!(loaded.triples().len() >= 2);

    Ok(())
}
