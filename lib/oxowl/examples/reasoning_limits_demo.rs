//! Demonstration of reasoning safety limits.
//!
//! This example shows:
//! 1. Reasoning without limits (potentially unbounded)
//! 2. Reasoning with timeout enforcement
//! 3. Reasoning with materialization limits
//! 4. Measurements and comparisons
//!
//! Run with: cargo run -p oxowl --example reasoning_limits_demo

use oxowl::{Axiom, Individual, ObjectProperty, Ontology, OwlClass, Reasoner, ReasonerConfig, RlReasoner, ClassExpression};
use oxrdf::NamedNode;
use std::time::{Duration, Instant};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("╔═══════════════════════════════════════════════════════╗");
    println!("║   OWL Reasoning Safety Limits Demonstration          ║");
    println!("╚═══════════════════════════════════════════════════════╝\n");

    // Demonstrate different scenarios
    demo_unlimited_reasoning()?;
    println!("\n{}\n", "─".repeat(60));

    demo_timeout_enforcement()?;
    println!("\n{}\n", "─".repeat(60));

    demo_materialization_limit()?;
    println!("\n{}\n", "─".repeat(60));

    demo_transitive_explosion()?;
    println!("\n{}\n", "─".repeat(60));

    demo_symmetric_transitive_worst_case()?;

    println!("\n╔═══════════════════════════════════════════════════════╗");
    println!("║   Summary: Safety Features                           ║");
    println!("╠═══════════════════════════════════════════════════════╣");
    println!("║ ✓ Timeout enforcement: IMPLEMENTED                   ║");
    println!("║ ✓ Materialization limit: IMPLEMENTED                 ║");
    println!("║ ✓ Iteration limit: IMPLEMENTED                       ║");
    println!("║ ⚠ Provenance tracking: NOT IMPLEMENTED               ║");
    println!("╚═══════════════════════════════════════════════════════╝\n");

    Ok(())
}

/// Demonstrate reasoning without limits
fn demo_unlimited_reasoning() -> Result<(), Box<dyn std::error::Error>> {
    println!("1️⃣  Unlimited Reasoning");
    println!("───────────────────────────────────────────────────────");

    let mut ontology = Ontology::with_iri("http://example.org/unlimited")?;

    // Simple ontology
    let person = OwlClass::new(NamedNode::new("http://example.org/Person")?);
    let knows = ObjectProperty::new(NamedNode::new("http://example.org/knows")?);

    ontology.add_axiom(Axiom::DeclareClass(person.clone()));
    ontology.add_axiom(Axiom::DeclareObjectProperty(knows.clone()));
    ontology.add_axiom(Axiom::SymmetricObjectProperty(knows.clone()));

    // Create small network
    let mut individuals = Vec::new();
    for i in 0..20 {
        let individual = Individual::Named(
            NamedNode::new(&format!("http://example.org/person_{}", i))?
        );
        ontology.add_axiom(Axiom::class_assertion(
            ClassExpression::class(person.clone()),
            individual.clone(),
        ));
        individuals.push(individual);
    }

    // Create connections
    for i in 0..19 {
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: knows.clone(),
            source: individuals[i].clone(),
            target: individuals[i + 1].clone(),
        });
    }

    println!("  Input:");
    println!("    - Individuals: 20");
    println!("    - Properties: symmetric 'knows'");
    println!("    - Initial axioms: {}", ontology.axiom_count());

    let config = ReasonerConfig::default(); // No limits

    let start = Instant::now();
    let mut reasoner = RlReasoner::with_config(&ontology, config);
    let result = reasoner.classify();
    let duration = start.elapsed();

    match result {
        Ok(_) => {
            let inferred = reasoner.get_inferred_axioms().len();
            println!("  Output:");
            println!("    - Status: ✓ Completed");
            println!("    - Time: {:?}", duration);
            println!("    - Inferred axioms: {}", inferred);
            println!("    - Memory: Unbounded");
        }
        Err(e) => {
            println!("  Output:");
            println!("    - Status: ✗ Failed");
            println!("    - Error: {}", e);
        }
    }

    Ok(())
}

/// Demonstrate timeout enforcement
fn demo_timeout_enforcement() -> Result<(), Box<dyn std::error::Error>> {
    println!("2️⃣  Timeout Enforcement");
    println!("───────────────────────────────────────────────────────");

    let mut ontology = Ontology::with_iri("http://example.org/timeout")?;

    // Create complex ontology
    let prop = ObjectProperty::new(NamedNode::new("http://example.org/related")?);
    ontology.add_axiom(Axiom::DeclareObjectProperty(prop.clone()));
    ontology.add_axiom(Axiom::TransitiveObjectProperty(prop.clone()));

    // Large chain
    let mut individuals = Vec::new();
    for i in 0..500 {
        let individual = Individual::Named(
            NamedNode::new(&format!("http://example.org/node_{}", i))?
        );
        individuals.push(individual);
    }

    for i in 0..499 {
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: prop.clone(),
            source: individuals[i].clone(),
            target: individuals[i + 1].clone(),
        });
    }

    println!("  Input:");
    println!("    - Individuals: 500");
    println!("    - Properties: transitive 'related'");
    println!("    - Chain length: 499");
    println!("    - Timeout: 50ms");

    let config = ReasonerConfig {
        max_iterations: 100_000,
        timeout: Some(Duration::from_millis(50)),
        max_inferred_triples: None,
        check_consistency: false,
        materialize: true,
    };

    let start = Instant::now();
    let mut reasoner = RlReasoner::with_config(&ontology, config);
    let result = reasoner.classify();
    let duration = start.elapsed();

    match result {
        Ok(_) => {
            let inferred = reasoner.get_inferred_axioms().len();
            println!("  Output:");
            println!("    - Status: ✓ Completed within timeout");
            println!("    - Time: {:?}", duration);
            println!("    - Inferred axioms: {}", inferred);
        }
        Err(e) => {
            println!("  Output:");
            println!("    - Status: ⏱  Timeout enforced");
            println!("    - Time: {:?}", duration);
            println!("    - Error: {}", e);
            println!("    - Result: Reasoning terminated early (safe)");
        }
    }

    Ok(())
}

/// Demonstrate materialization limit
fn demo_materialization_limit() -> Result<(), Box<dyn std::error::Error>> {
    println!("3️⃣  Materialization Limit");
    println!("───────────────────────────────────────────────────────");

    let mut ontology = Ontology::with_iri("http://example.org/materialize")?;

    // Symmetric + Transitive = potential O(n²) explosion
    let connected = ObjectProperty::new(NamedNode::new("http://example.org/connected")?);
    ontology.add_axiom(Axiom::DeclareObjectProperty(connected.clone()));
    ontology.add_axiom(Axiom::SymmetricObjectProperty(connected.clone()));
    ontology.add_axiom(Axiom::TransitiveObjectProperty(connected.clone()));

    // Star pattern
    let mut individuals = Vec::new();
    for i in 0..100 {
        let individual = Individual::Named(
            NamedNode::new(&format!("http://example.org/entity_{}", i))?
        );
        individuals.push(individual);
    }

    let center = individuals[0].clone();
    for i in 1..100 {
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: connected.clone(),
            source: center.clone(),
            target: individuals[i].clone(),
        });
    }

    println!("  Input:");
    println!("    - Individuals: 100");
    println!("    - Properties: symmetric + transitive 'connected'");
    println!("    - Pattern: star graph");
    println!("    - Max inferred triples: 5,000");

    let config = ReasonerConfig {
        max_iterations: 100_000,
        timeout: None,
        max_inferred_triples: Some(5_000),
        check_consistency: false,
        materialize: true,
    };

    let start = Instant::now();
    let mut reasoner = RlReasoner::with_config(&ontology, config);
    let result = reasoner.classify();
    let duration = start.elapsed();

    match result {
        Ok(_) => {
            let inferred = reasoner.get_inferred_axioms().len();
            println!("  Output:");
            println!("    - Status: ✓ Completed within limit");
            println!("    - Time: {:?}", duration);
            println!("    - Inferred axioms: {}", inferred);
            println!("    - Limit: 5,000");
        }
        Err(e) => {
            println!("  Output:");
            println!("    - Status: 🛑 Limit enforced");
            println!("    - Time: {:?}", duration);
            println!("    - Error: {}", e);
            println!("    - Result: Materialization stopped (safe)");
        }
    }

    Ok(())
}

/// Demonstrate transitive property explosion
fn demo_transitive_explosion() -> Result<(), Box<dyn std::error::Error>> {
    println!("4️⃣  Transitive Property Explosion");
    println!("───────────────────────────────────────────────────────");

    let mut ontology = Ontology::with_iri("http://example.org/transitive")?;

    let ancestor = ObjectProperty::new(NamedNode::new("http://example.org/ancestor")?);
    ontology.add_axiom(Axiom::DeclareObjectProperty(ancestor.clone()));
    ontology.add_axiom(Axiom::TransitiveObjectProperty(ancestor.clone()));

    // Long chain: person_0 → person_1 → ... → person_99
    let mut individuals = Vec::new();
    for i in 0..100 {
        let individual = Individual::Named(
            NamedNode::new(&format!("http://example.org/person_{}", i))?
        );
        individuals.push(individual);
    }

    for i in 0..99 {
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: ancestor.clone(),
            source: individuals[i + 1].clone(),
            target: individuals[i].clone(),
        });
    }

    println!("  Input:");
    println!("    - Chain length: 100 individuals");
    println!("    - Properties: transitive 'ancestor'");
    println!("    - Expected O(n²): ~4,950 inferred relationships");

    let start = Instant::now();
    let mut reasoner = RlReasoner::new(&ontology);
    let result = reasoner.classify();
    let duration = start.elapsed();

    match result {
        Ok(_) => {
            let inferred = reasoner.get_inferred_axioms().len();
            println!("  Output:");
            println!("    - Status: ✓ Completed");
            println!("    - Time: {:?}", duration);
            println!("    - Inferred axioms: {}", inferred);
            println!("    - Complexity: O(n²) = {} → {}", 100, inferred);

            if inferred > 1000 {
                println!("    - ⚠️  WARNING: Quadratic explosion occurred!");
            }
        }
        Err(e) => {
            println!("  Output:");
            println!("    - Status: ✗ Failed");
            println!("    - Error: {}", e);
        }
    }

    Ok(())
}

/// Demonstrate symmetric + transitive worst case
fn demo_symmetric_transitive_worst_case() -> Result<(), Box<dyn std::error::Error>> {
    println!("5️⃣  Symmetric + Transitive (Worst Case)");
    println!("───────────────────────────────────────────────────────");

    let mut ontology = Ontology::with_iri("http://example.org/worst")?;

    let related = ObjectProperty::new(NamedNode::new("http://example.org/related")?);
    ontology.add_axiom(Axiom::DeclareObjectProperty(related.clone()));
    ontology.add_axiom(Axiom::SymmetricObjectProperty(related.clone()));
    ontology.add_axiom(Axiom::TransitiveObjectProperty(related.clone()));

    // Path: 0 → 1 → 2 → ... → 29
    let mut individuals = Vec::new();
    for i in 0..30 {
        let individual = Individual::Named(
            NamedNode::new(&format!("http://example.org/node_{}", i))?
        );
        individuals.push(individual);
    }

    for i in 0..29 {
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: related.clone(),
            source: individuals[i].clone(),
            target: individuals[i + 1].clone(),
        });
    }

    println!("  Input:");
    println!("    - Path length: 30 nodes");
    println!("    - Properties: BOTH symmetric AND transitive");
    println!("    - Expected: Complete graph (30 × 29 = 870 edges)");

    let config = ReasonerConfig {
        max_iterations: 100_000,
        timeout: Some(Duration::from_secs(5)),
        max_inferred_triples: Some(2_000),
        check_consistency: false,
        materialize: true,
    };

    let start = Instant::now();
    let mut reasoner = RlReasoner::with_config(&ontology, config);
    let result = reasoner.classify();
    let duration = start.elapsed();

    match result {
        Ok(_) => {
            let inferred = reasoner.get_inferred_axioms().len();
            println!("  Output:");
            println!("    - Status: ✓ Completed");
            println!("    - Time: {:?}", duration);
            println!("    - Inferred axioms: {}", inferred);
            println!("    - Pattern: Complete graph created");
            println!("    - ⚠️  Demonstrates worst-case O(n²) behavior");
        }
        Err(e) => {
            println!("  Output:");
            println!("    - Status: 🛑 Safety limit hit");
            println!("    - Time: {:?}", duration);
            println!("    - Error: {}", e);
            println!("    - Result: Prevented unbounded materialization");
        }
    }

    Ok(())
}
