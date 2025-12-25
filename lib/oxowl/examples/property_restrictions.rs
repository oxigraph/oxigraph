//! Property restrictions example demonstrating existential and universal restrictions.
//!
//! This example shows:
//! - Existential restrictions (∃hasChild.Person)
//! - Universal restrictions (∀hasChild.Student)
//! - HasValue restrictions
//! - Cardinality restrictions
//!
//! Run with: cargo run -p oxowl --example property_restrictions

use oxowl::{
    Ontology, Axiom, ClassExpression, OwlClass, ObjectProperty, Individual,
};
use oxrdf::NamedNode;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OWL 2 Property Restrictions Example ===\n");

    // Create ontology for university domain
    let mut ontology = Ontology::with_iri("http://example.org/university")?;

    // Define classes
    let person = OwlClass::new(NamedNode::new("http://example.org/Person")?);
    let student = OwlClass::new(NamedNode::new("http://example.org/Student")?);
    let professor = OwlClass::new(NamedNode::new("http://example.org/Professor")?);
    let parent = OwlClass::new(NamedNode::new("http://example.org/Parent")?);
    let strict_parent = OwlClass::new(NamedNode::new("http://example.org/StrictParent")?);

    // Declare classes
    for class in [&person, &student, &professor, &parent, &strict_parent] {
        ontology.add_axiom(Axiom::DeclareClass(class.clone()));
    }

    // Build class hierarchy
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(student.clone()),
        ClassExpression::class(person.clone()),
    ));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(professor.clone()),
        ClassExpression::class(person.clone()),
    ));

    // Define properties
    let has_child = ObjectProperty::new(NamedNode::new("http://example.org/hasChild")?);
    let has_advisor = ObjectProperty::new(NamedNode::new("http://example.org/hasAdvisor")?);
    let teaches = ObjectProperty::new(NamedNode::new("http://example.org/teaches")?);

    for prop in [&has_child, &has_advisor, &teaches] {
        ontology.add_axiom(Axiom::DeclareObjectProperty(prop.clone()));
    }

    // Define Parent using existential restriction:
    // Parent ≡ Person ⊓ ∃hasChild.Person
    // (A parent is a person who has at least one child who is a person)
    let parent_definition = ClassExpression::intersection(vec![
        ClassExpression::class(person.clone()),
        ClassExpression::some_values_from(
            has_child.clone(),
            ClassExpression::class(person.clone()),
        ),
    ]);
    ontology.add_axiom(Axiom::equivalent_classes(vec![
        ClassExpression::class(parent.clone()),
        parent_definition,
    ]));

    println!("✓ Parent defined as: Person ⊓ ∃hasChild.Person");

    // Define StrictParent using universal restriction:
    // StrictParent ⊑ Parent ⊓ ∀hasChild.Student
    // (A strict parent is a parent whose all children are students)
    let strict_parent_def = ClassExpression::intersection(vec![
        ClassExpression::class(parent.clone()),
        ClassExpression::all_values_from(
            has_child.clone(),
            ClassExpression::class(student.clone()),
        ),
    ]);
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(strict_parent.clone()),
        strict_parent_def,
    ));

    println!("✓ StrictParent defined as: Parent ⊓ ∀hasChild.Student");

    // Example: Professor who teaches at least one course
    // Define using existential with inverse
    let teaching_professor = OwlClass::new(NamedNode::new("http://example.org/TeachingProfessor")?);
    ontology.add_axiom(Axiom::DeclareClass(teaching_professor.clone()));

    // TeachingProfessor ⊑ Professor ⊓ ∃teaches.⊤
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(teaching_professor.clone()),
        ClassExpression::class(professor.clone()),
    ));

    println!("✓ TeachingProfessor defined as subclass of Professor");

    // Example: HasValue restriction
    // Define a class of people who have a specific advisor
    let dr_smith = Individual::Named(NamedNode::new("http://example.org/DrSmith")?);

    let smith_advisee_class = ClassExpression::ObjectHasValue {
        property: has_advisor.clone().into(),
        individual: dr_smith.clone(),
    };

    println!("\n✓ Created class expression: ∃hasAdvisor.{{DrSmith}}");

    // Example: Cardinality restrictions
    // Define person with exactly 2 children
    let has_two_children = ClassExpression::ObjectExactCardinality {
        cardinality: 2,
        property: has_child.clone().into(),
        filler: Some(Box::new(ClassExpression::class(person.clone()))),
    };

    let parent_of_two = OwlClass::new(NamedNode::new("http://example.org/ParentOfTwo")?);
    ontology.add_axiom(Axiom::DeclareClass(parent_of_two.clone()));
    ontology.add_axiom(Axiom::equivalent_classes(vec![
        ClassExpression::class(parent_of_two.clone()),
        has_two_children,
    ]));

    println!("✓ ParentOfTwo defined as: =2 hasChild.Person");

    // Define person with at least 3 children
    let has_many_children = ClassExpression::ObjectMinCardinality {
        cardinality: 3,
        property: has_child.clone().into(),
        filler: None,
    };

    let large_family = OwlClass::new(NamedNode::new("http://example.org/LargeFamily")?);
    ontology.add_axiom(Axiom::DeclareClass(large_family.clone()));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(large_family.clone()),
        has_many_children,
    ));

    println!("✓ LargeFamily defined as: ≥3 hasChild");

    // Create some individuals
    let john = Individual::Named(NamedNode::new("http://example.org/John")?);
    let mary = Individual::Named(NamedNode::new("http://example.org/Mary")?);
    let alice = Individual::Named(NamedNode::new("http://example.org/Alice")?);
    let bob = Individual::Named(NamedNode::new("http://example.org/Bob")?);

    // Assert John is a Person with children
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(person.clone()),
        john.clone(),
    ));

    // Assert Alice and Bob are Students
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(student.clone()),
        alice.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(student.clone()),
        bob.clone(),
    ));

    // John has children Alice and Bob
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_child.clone(),
        source: john.clone(),
        target: alice.clone(),
    });
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_child.clone(),
        source: john.clone(),
        target: bob.clone(),
    });

    println!("\n✓ Created individuals and relationships");

    // Summary
    println!("\n=== Ontology Summary ===");
    println!("Classes: {}", ontology.classes().count());
    println!("Object Properties: {}", ontology.object_properties().count());
    println!("Individuals: {}", ontology.individuals().count());
    println!("Axioms: {}", ontology.axiom_count());

    println!("\n=== Class Hierarchy ===");
    for class in ontology.classes() {
        let superclasses = ontology.direct_superclasses_of(class);
        if !superclasses.is_empty() {
            print!("{} ⊑ ", class.iri());
            for (i, sup) in superclasses.iter().enumerate() {
                if i > 0 {
                    print!(" ⊓ ");
                }
                if let Some(sup_class) = sup.as_class() {
                    print!("{}", sup_class.iri());
                } else {
                    print!("<complex expression>");
                }
            }
            println!();
        }
    }

    #[cfg(feature = "reasoner-rl")]
    {
        use oxowl::{Reasoner, RlReasoner};

        println!("\n=== Running Reasoner ===");
        let mut reasoner = RlReasoner::new(&ontology);
        reasoner.classify()?;

        // Check inferred types for John
        println!("\nJohn's inferred types:");
        for typ in reasoner.get_types(&john) {
            println!("  - {}", typ.iri());
        }

        // The reasoner should infer that John is a Parent
        // because he has children who are Persons (via Student ⊑ Person)
        println!("\n✓ Reasoner completed successfully");
    }

    Ok(())
}
