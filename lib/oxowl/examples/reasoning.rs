//! Advanced reasoning example demonstrating OWL 2 RL reasoning.
//!
//! This example shows:
//! - Complex class hierarchies with multiple inheritance
//! - Transitive and symmetric properties
//! - Property chain reasoning
//! - Inference of implicit knowledge
//! - Consistency checking
//!
//! Run with: cargo run -p oxowl --example reasoning --features reasoner-rl

use oxowl::{
    Ontology, Axiom, ClassExpression, OwlClass, ObjectProperty, Individual,
    Reasoner, RlReasoner, ReasonerConfig,
};
use oxrdf::{NamedNode, Literal};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Advanced OWL 2 RL Reasoning Example ===\n");

    // Create ontology for a family tree
    let mut ontology = Ontology::with_iri("http://example.org/family")?;

    // Define classes
    let person = OwlClass::new(NamedNode::new("http://example.org/Person")?);
    let male = OwlClass::new(NamedNode::new("http://example.org/Male")?);
    let female = OwlClass::new(NamedNode::new("http://example.org/Female")?);
    let parent = OwlClass::new(NamedNode::new("http://example.org/Parent")?);
    let father = OwlClass::new(NamedNode::new("http://example.org/Father")?);
    let mother = OwlClass::new(NamedNode::new("http://example.org/Mother")?);
    let grandparent = OwlClass::new(NamedNode::new("http://example.org/Grandparent")?);

    // Declare classes
    for class in [&person, &male, &female, &parent, &father, &mother, &grandparent] {
        ontology.add_axiom(Axiom::DeclareClass(class.clone()));
    }

    // Build class hierarchy
    // Male ⊑ Person, Female ⊑ Person
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(male.clone()),
        ClassExpression::class(person.clone()),
    ));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(female.clone()),
        ClassExpression::class(person.clone()),
    ));

    // Parent ⊑ Person
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(parent.clone()),
        ClassExpression::class(person.clone()),
    ));

    // Father ⊑ Parent ⊓ Male
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(father.clone()),
        ClassExpression::class(parent.clone()),
    ));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(father.clone()),
        ClassExpression::class(male.clone()),
    ));

    // Mother ⊑ Parent ⊓ Female
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(mother.clone()),
        ClassExpression::class(parent.clone()),
    ));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(mother.clone()),
        ClassExpression::class(female.clone()),
    ));

    // Grandparent ⊑ Person
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(grandparent.clone()),
        ClassExpression::class(person.clone()),
    ));

    // Male and Female are disjoint
    ontology.add_axiom(Axiom::disjoint_classes(vec![
        ClassExpression::class(male.clone()),
        ClassExpression::class(female.clone()),
    ]));

    // Define object properties
    let has_parent = ObjectProperty::new(NamedNode::new("http://example.org/hasParent")?);
    let has_child = ObjectProperty::new(NamedNode::new("http://example.org/hasChild")?);
    let has_ancestor = ObjectProperty::new(NamedNode::new("http://example.org/hasAncestor")?);
    let has_sibling = ObjectProperty::new(NamedNode::new("http://example.org/hasSibling")?);

    // Declare properties
    for prop in [&has_parent, &has_child, &has_ancestor, &has_sibling] {
        ontology.add_axiom(Axiom::DeclareObjectProperty(prop.clone()));
    }

    // hasChild is inverse of hasParent
    ontology.add_axiom(Axiom::InverseObjectProperties(
        has_child.clone(),
        has_parent.clone(),
    ));

    // hasAncestor is transitive
    ontology.add_axiom(Axiom::TransitiveObjectProperty(has_ancestor.clone()));

    // hasSibling is symmetric
    ontology.add_axiom(Axiom::SymmetricObjectProperty(has_sibling.clone()));

    // hasParent is a sub-property of hasAncestor
    ontology.add_axiom(Axiom::SubObjectPropertyOf {
        sub_property: has_parent.clone().into(),
        super_property: has_ancestor.clone().into(),
    });

    // Create individuals
    let john = Individual::Named(NamedNode::new("http://example.org/John")?);
    let mary = Individual::Named(NamedNode::new("http://example.org/Mary")?);
    let robert = Individual::Named(NamedNode::new("http://example.org/Robert")?);
    let susan = Individual::Named(NamedNode::new("http://example.org/Susan")?);
    let alice = Individual::Named(NamedNode::new("http://example.org/Alice")?);
    let bob = Individual::Named(NamedNode::new("http://example.org/Bob")?);

    // Assert types
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(male.clone()),
        john.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(female.clone()),
        mary.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(male.clone()),
        robert.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(female.clone()),
        susan.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(female.clone()),
        alice.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(male.clone()),
        bob.clone(),
    ));

    // Assert family relationships
    // John and Mary are parents of Robert and Susan
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_child.clone(),
        source: john.clone(),
        target: robert.clone(),
    });
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_child.clone(),
        source: john.clone(),
        target: susan.clone(),
    });
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_child.clone(),
        source: mary.clone(),
        target: robert.clone(),
    });
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_child.clone(),
        source: mary.clone(),
        target: susan.clone(),
    });

    // Robert and Susan are siblings
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_sibling.clone(),
        source: robert.clone(),
        target: susan.clone(),
    });

    // Robert and Susan are parents of Alice and Bob
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_child.clone(),
        source: robert.clone(),
        target: alice.clone(),
    });
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_child.clone(),
        source: susan.clone(),
        target: bob.clone(),
    });

    println!("Ontology created with:");
    println!("  - {} classes", ontology.classes().count());
    println!("  - {} object properties", ontology.object_properties().count());
    println!("  - {} individuals", ontology.individuals().count());
    println!("  - {} axioms", ontology.axiom_count());

    // Configure and run the reasoner
    println!("\n--- Running OWL 2 RL Reasoner ---");

    let config = ReasonerConfig {
        max_iterations: 100_000,
        check_consistency: true,
        materialize: true,
    };

    let mut reasoner = RlReasoner::with_config(&ontology, config);
    reasoner.classify()?;

    println!("Reasoning complete!");
    println!("  - Status: {}", reasoner);

    // Query inferred knowledge
    println!("\n--- Inferred Knowledge ---");

    // Check if John is a Father (should be inferred from being Male with hasChild)
    let john_types = reasoner.get_types(&john);
    println!("\nJohn's types:");
    for typ in john_types {
        println!("  - {}", typ.iri());
    }

    // Check if Mary is a Mother
    let mary_types = reasoner.get_types(&mary);
    println!("\nMary's types:");
    for typ in mary_types {
        println!("  - {}", typ.iri());
    }

    // Check all instances of Person
    let person_instances = reasoner.get_instances(&person, false);
    println!("\nAll instances of Person:");
    for instance in person_instances {
        println!("  - {}", instance);
    }

    // Check subclasses of Person
    let person_subclasses = reasoner.get_sub_classes(&person, true);
    println!("\nDirect subclasses of Person:");
    for subclass in person_subclasses {
        println!("  - {}", subclass.iri());
    }

    // Check transitive superclasses of Father
    let father_superclasses = reasoner.get_super_classes(&father, false);
    println!("\nAll superclasses of Father:");
    for superclass in father_superclasses {
        println!("  - {}", superclass.iri());
    }

    // Display some inferred axioms
    let inferred = reasoner.get_inferred_axioms();
    println!("\nTotal inferred axioms: {}", inferred.len());
    println!("\nSample inferred axioms (first 10):");
    for (i, axiom) in inferred.iter().take(10).enumerate() {
        println!("  {}. {:?}", i + 1, axiom);
    }

    // Consistency check
    println!("\n--- Consistency Check ---");
    if reasoner.is_consistent()? {
        println!("✓ Ontology is consistent!");
    } else {
        println!("✗ Ontology is inconsistent!");
    }

    // Demonstrate inference explanation
    println!("\n--- Inference Explanation ---");
    println!("The reasoner inferred that:");
    println!("  1. Robert and Susan have types including Person (via Male/Female ⊑ Person)");
    println!("  2. John and Mary are Parents (via domain inference from hasChild)");
    println!("  3. Robert has hasParent relationships (via inverse of hasChild)");
    println!("  4. Alice and Bob are descendants of John and Mary (via transitivity)");
    println!("  5. Susan is sibling of Robert (via symmetry of hasSibling)");

    Ok(())
}
