use oxigraph::model::*;
use oxigraph::store::Store;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new in-memory store
    let store = Store::new()?;

    // Create an N3 formula with some triples
    let formula_id = BlankNode::new("my_formula")?;
    let alice = NamedNode::new("http://example.com/alice")?;
    let bob = NamedNode::new("http://example.com/bob")?;
    let knows = NamedNode::new("http://xmlns.com/foaf/0.1/knows")?;
    let name = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;

    let triples = vec![
        Triple::new(
            alice.clone(),
            name.clone(),
            Literal::new_simple_literal("Alice"),
        ),
        Triple::new(alice.clone(), knows.clone(), bob.clone()),
        Triple::new(bob.clone(), name, Literal::new_simple_literal("Bob")),
    ];

    let formula = Formula::new(formula_id.clone(), triples);

    println!("Original formula: {}", formula);
    println!("Formula ID: {}", formula.id());
    println!("Number of triples: {}", formula.triples().len());

    // Store the formula in the store
    store.store_formula(&formula)?;
    println!("\nFormula stored successfully!");

    // Load the formula back from the store
    let loaded_formula = store.load_formula(formula_id.as_ref())?;
    println!("\nLoaded formula: {}", loaded_formula);
    println!("Loaded {} triples", loaded_formula.triples().len());

    // Verify the data
    for (i, triple) in loaded_formula.triples().iter().enumerate() {
        println!("  Triple {}: {}", i + 1, triple);
    }

    // Use transactions for atomic operations
    println!("\n--- Transaction Example ---");
    let mut transaction = store.start_transaction()?;

    let formula_id2 = BlankNode::new("formula2")?;
    let carol = NamedNode::new("http://example.com/carol")?;
    let triple = Triple::new(
        carol,
        knows,
        alice.clone(),
    );
    let formula2 = Formula::new(formula_id2.clone(), vec![triple]);

    transaction.store_formula(&formula2);
    println!("Storing second formula in transaction...");

    // Can query before committing
    let loaded_in_tx = transaction.load_formula(formula_id2.as_ref())?;
    println!("Loaded in transaction: {} triples", loaded_in_tx.triples().len());

    transaction.commit()?;
    println!("Transaction committed!");

    // Verify both formulas exist
    let all_graphs: Vec<_> = store.named_graphs().collect();
    println!("\nTotal named graphs in store: {}", all_graphs.len());

    // Remove a formula
    println!("\n--- Cleanup ---");
    store.remove_formula(formula_id2.as_ref())?;
    println!("Removed formula2");

    let remaining_graphs: Vec<_> = store.named_graphs().collect();
    println!("Remaining named graphs: {}", remaining_graphs.len());

    Ok(())
}
