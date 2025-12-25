//! Basic ontology example demonstrating OWL 2 support.
//!
//! Run with: cargo run -p oxowl --example basic_ontology

use oxowl::{Ontology, Axiom, ClassExpression, OwlClass, ObjectProperty, Individual};
use oxrdf::NamedNode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create a new ontology
    let mut ontology = Ontology::with_iri("http://example.org/animals")?;

    // Define classes
    let animal = OwlClass::new(NamedNode::new("http://example.org/Animal")?);
    let mammal = OwlClass::new(NamedNode::new("http://example.org/Mammal")?);
    let dog = OwlClass::new(NamedNode::new("http://example.org/Dog")?);
    let cat = OwlClass::new(NamedNode::new("http://example.org/Cat")?);

    // Declare classes
    ontology.add_axiom(Axiom::DeclareClass(animal.clone()));
    ontology.add_axiom(Axiom::DeclareClass(mammal.clone()));
    ontology.add_axiom(Axiom::DeclareClass(dog.clone()));
    ontology.add_axiom(Axiom::DeclareClass(cat.clone()));

    // Add class hierarchy: Dog ⊑ Mammal ⊑ Animal
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(mammal.clone()),
        ClassExpression::class(animal.clone()),
    ));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(dog.clone()),
        ClassExpression::class(mammal.clone()),
    ));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(cat.clone()),
        ClassExpression::class(mammal.clone()),
    ));

    // Dog and Cat are disjoint
    ontology.add_axiom(Axiom::disjoint_classes(vec![
        ClassExpression::class(dog.clone()),
        ClassExpression::class(cat.clone()),
    ]));

    // Create individuals
    let fido = Individual::Named(NamedNode::new("http://example.org/fido")?);
    let whiskers = Individual::Named(NamedNode::new("http://example.org/whiskers")?);

    // Add class assertions
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(dog.clone()),
        fido.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(cat.clone()),
        whiskers.clone(),
    ));

    println!("Created ontology: {}", ontology);
    println!("Classes: {:?}", ontology.classes().count());
    println!("Individuals: {:?}", ontology.individuals().count());
    println!("Axioms: {:?}", ontology.axiom_count());

    // Use the reasoner
    #[cfg(feature = "reasoner-rl")]
    {
        use oxowl::{Reasoner, RlReasoner};

        let mut reasoner = RlReasoner::new(&ontology);
        reasoner.classify()?;

        println!("\n--- Reasoning Results ---");

        // Check inferred types for fido
        let fido_types = reasoner.get_types(&fido);
        println!("Fido is an instance of: {:?}", fido_types.iter().map(|c| c.iri().as_str()).collect::<Vec<_>>());

        // Check subclasses of Animal
        let animal_subclasses = reasoner.get_sub_classes(&animal, false);
        println!("Subclasses of Animal: {:?}", animal_subclasses.iter().map(|c| c.iri().as_str()).collect::<Vec<_>>());

        println!("\nReasoner: {}", reasoner);
    }

    Ok(())
}
