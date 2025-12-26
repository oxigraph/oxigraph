//! Example demonstrating loading and reasoning with OWL ontologies in N3 format.
//!
//! This example shows how to:
//! - Load an OWL ontology from N3 format
//! - Apply OWL reasoning
//! - Work with N3 formulas and rules
//!
//! Run with: cargo run --example n3_ontology

use oxowl::n3_integration::parse_n3_ontology;
use oxowl::n3_rules::{extend_ontology_with_n3_rules, N3Rule};
use oxowl::{Axiom, ClassExpression, Individual};
use oxrdf::{BlankNode, Formula, NamedNode, Triple};
use oxrdf::vocab::rdf;

#[cfg(feature = "reasoner-rl")]
use oxowl::{Reasoner, RlReasoner};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OWL Ontology Loading from N3 Format ===\n");

    // Example 1: Basic ontology loading
    basic_ontology_example()?;

    // Example 2: Ontology with individuals and reasoning
    #[cfg(feature = "reasoner-rl")]
    reasoning_example()?;

    // Example 3: N3 rules and OWL integration
    n3_rules_example()?;

    Ok(())
}

fn basic_ontology_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 1: Basic Ontology Loading ---");

    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <http://example.org/animals#> .

# Ontology declaration
ex:AnimalOntology a owl:Ontology ;
    rdfs:label "Animal Ontology"@en ;
    owl:versionInfo "1.0" .

# Class hierarchy
ex:Animal a owl:Class ;
    rdfs:label "Animal"@en .

ex:Mammal a owl:Class ;
    rdfs:label "Mammal"@en ;
    rdfs:subClassOf ex:Animal .

ex:Dog a owl:Class ;
    rdfs:label "Dog"@en ;
    rdfs:subClassOf ex:Mammal .

ex:Cat a owl:Class ;
    rdfs:label "Cat"@en ;
    rdfs:subClassOf ex:Mammal .

ex:Bird a owl:Class ;
    rdfs:label "Bird"@en ;
    rdfs:subClassOf ex:Animal .

# Disjoint classes
ex:Mammal owl:disjointWith ex:Bird .
ex:Dog owl:disjointWith ex:Cat .
"#;

    // Parse the ontology
    let ontology = parse_n3_ontology(n3_data.as_bytes())?;

    println!("Loaded ontology: {}", ontology.iri().map_or("(anonymous)", |i| i.as_str()));
    println!("Number of axioms: {}", ontology.axiom_count());

    // Display some axioms
    println!("\nSample axioms:");
    for (i, axiom) in ontology.axioms().iter().take(5).enumerate() {
        println!("  {}. {:?}", i + 1, axiom);
    }

    println!();
    Ok(())
}

#[cfg(feature = "reasoner-rl")]
fn reasoning_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 2: Reasoning with N3 Ontology ---");

    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <http://example.org/pets#> .

# Class hierarchy
ex:Animal a owl:Class .
ex:Pet a owl:Class ;
    rdfs:subClassOf ex:Animal .
ex:Dog a owl:Class ;
    rdfs:subClassOf ex:Pet .
ex:Puppy a owl:Class ;
    rdfs:subClassOf ex:Dog .

# Individuals
ex:fido a ex:Puppy .
ex:rex a ex:Dog .
ex:fluffy a ex:Pet .
"#;

    let ontology = parse_n3_ontology(n3_data.as_bytes())?;

    println!("Ontology loaded with {} axioms", ontology.axiom_count());

    // Create a reasoner
    let mut reasoner = RlReasoner::new(&ontology);

    // Perform classification
    println!("\nPerforming OWL 2 RL reasoning...");
    reasoner.classify()?;

    // Query inferred types
    let fido = Individual::Named(NamedNode::new("http://example.org/pets#fido")?);
    let types = reasoner.get_types(&fido);

    println!("\nInferred types for 'fido' (a Puppy):");
    for class in types {
        println!("  - {}", class.iri());
    }

    // Should include: Puppy (asserted), Dog, Pet, Animal (inferred)
    println!("\nReasoning complete. Fido is inferred to be:");
    println!("  - a Puppy (asserted)");
    println!("  - a Dog (inferred via Puppy ⊑ Dog)");
    println!("  - a Pet (inferred via Dog ⊑ Pet)");
    println!("  - an Animal (inferred via Pet ⊑ Animal)");

    println!();
    Ok(())
}

fn n3_rules_example() -> Result<(), Box<dyn std::error::Error>> {
    println!("--- Example 3: N3 Rules and OWL Integration ---");

    // Demonstrate N3 rule patterns
    let dog = NamedNode::new("http://example.org/Dog")?;
    let animal = NamedNode::new("http://example.org/Animal")?;
    let var_x = BlankNode::new("x")?;

    // Create an N3 rule: { ?x a :Dog } => { ?x a :Animal }
    // This represents: Dog ⊑ Animal
    let antecedent = Formula::new(
        BlankNode::default(),
        vec![Triple::new(var_x.clone(), rdf::TYPE, dog.clone())],
    );

    let consequent = Formula::new(
        BlankNode::default(),
        vec![Triple::new(var_x, rdf::TYPE, animal.clone())],
    );

    let rule = N3Rule::new(antecedent, consequent);

    println!("Created N3 rule:");
    println!("  Antecedent: {{ ?x rdf:type ex:Dog }}");
    println!("  Consequent: {{ ?x rdf:type ex:Animal }}");

    // Check if it's OWL-compatible
    if rule.is_owl_compatible() {
        println!("\n✓ Rule is OWL-compatible!");

        // Convert to OWL axioms
        let axioms = rule.to_owl_axioms();
        println!("\nConverted to {} OWL axiom(s):", axioms.len());

        for axiom in &axioms {
            match axiom {
                Axiom::SubClassOf { sub_class, super_class } => {
                    if let (ClassExpression::Class(sub), ClassExpression::Class(sup)) =
                        (sub_class, super_class)
                    {
                        println!("  SubClassOf: {} ⊑ {}", sub.iri(), sup.iri());
                    }
                }
                _ => println!("  {:?}", axiom),
            }
        }
    } else {
        println!("\n✗ Rule is not OWL-compatible");
    }

    println!("\nN3 rules can express logical implications that extend");
    println!("beyond standard OWL axioms. When they match known patterns");
    println!("(like the subclass pattern above), they can be converted");
    println!("to OWL axioms for integration with OWL reasoning.");

    println!();
    Ok(())
}
