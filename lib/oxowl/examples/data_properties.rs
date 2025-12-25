//! Data properties example demonstrating literal values and data property axioms.
//!
//! This example shows:
//! - Data property declarations
//! - Data property assertions with typed literals
//! - Data property domain and range constraints
//! - Functional data properties
//! - Data property hierarchies
//!
//! Run with: cargo run -p oxowl --example data_properties

use oxowl::{
    Ontology, Axiom, ClassExpression, OwlClass, DataProperty, Individual, DataRange,
};
use oxrdf::{NamedNode, Literal};
use oxrdf::vocab::xsd;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== OWL 2 Data Properties Example ===\n");

    // Create ontology for a person database
    let mut ontology = Ontology::with_iri("http://example.org/people")?;

    // Define classes
    let person = OwlClass::new(NamedNode::new("http://example.org/Person")?);
    let adult = OwlClass::new(NamedNode::new("http://example.org/Adult")?);
    let employee = OwlClass::new(NamedNode::new("http://example.org/Employee")?);

    // Declare classes
    for class in [&person, &adult, &employee] {
        ontology.add_axiom(Axiom::DeclareClass(class.clone()));
    }

    // Build class hierarchy
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(adult.clone()),
        ClassExpression::class(person.clone()),
    ));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(employee.clone()),
        ClassExpression::class(adult.clone()),
    ));

    println!("✓ Created class hierarchy: Employee ⊑ Adult ⊑ Person");

    // Define data properties
    let has_name = DataProperty::new(NamedNode::new("http://example.org/hasName")?);
    let has_age = DataProperty::new(NamedNode::new("http://example.org/hasAge")?);
    let has_email = DataProperty::new(NamedNode::new("http://example.org/hasEmail")?);
    let has_salary = DataProperty::new(NamedNode::new("http://example.org/hasSalary")?);
    let has_first_name = DataProperty::new(NamedNode::new("http://example.org/hasFirstName")?);
    let has_last_name = DataProperty::new(NamedNode::new("http://example.org/hasLastName")?);

    // Declare data properties
    for prop in [&has_name, &has_age, &has_email, &has_salary, &has_first_name, &has_last_name] {
        ontology.add_axiom(Axiom::DeclareDataProperty(prop.clone()));
    }

    println!("✓ Declared data properties");

    // Define data property domains and ranges
    // hasName: Person → xsd:string
    ontology.add_axiom(Axiom::DataPropertyDomain {
        property: has_name.clone(),
        domain: ClassExpression::class(person.clone()),
    });
    ontology.add_axiom(Axiom::DataPropertyRange {
        property: has_name.clone(),
        range: DataRange::datatype(xsd::STRING.into_owned()),
    });

    // hasAge: Person → xsd:integer
    ontology.add_axiom(Axiom::DataPropertyDomain {
        property: has_age.clone(),
        domain: ClassExpression::class(person.clone()),
    });
    ontology.add_axiom(Axiom::DataPropertyRange {
        property: has_age.clone(),
        range: DataRange::datatype(xsd::INTEGER.into_owned()),
    });

    // hasEmail: Person → xsd:string
    ontology.add_axiom(Axiom::DataPropertyDomain {
        property: has_email.clone(),
        domain: ClassExpression::class(person.clone()),
    });
    ontology.add_axiom(Axiom::DataPropertyRange {
        property: has_email.clone(),
        range: DataRange::datatype(xsd::STRING.into_owned()),
    });

    // hasSalary: Employee → xsd:decimal
    ontology.add_axiom(Axiom::DataPropertyDomain {
        property: has_salary.clone(),
        domain: ClassExpression::class(employee.clone()),
    });
    ontology.add_axiom(Axiom::DataPropertyRange {
        property: has_salary.clone(),
        range: DataRange::datatype(xsd::DECIMAL.into_owned()),
    });

    println!("✓ Defined domains and ranges for data properties");

    // Define functional data properties
    // A person can have only one age
    ontology.add_axiom(Axiom::FunctionalDataProperty(has_age.clone()));

    // An employee can have only one salary
    ontology.add_axiom(Axiom::FunctionalDataProperty(has_salary.clone()));

    println!("✓ Declared functional properties (hasAge, hasSalary)");

    // Define data property hierarchy
    // hasFirstName ⊑ hasName, hasLastName ⊑ hasName
    ontology.add_axiom(Axiom::SubDataPropertyOf {
        sub_property: has_first_name.clone(),
        super_property: has_name.clone(),
    });
    ontology.add_axiom(Axiom::SubDataPropertyOf {
        sub_property: has_last_name.clone(),
        super_property: has_name.clone(),
    });

    println!("✓ Created property hierarchy: hasFirstName ⊑ hasName, hasLastName ⊑ hasName");

    // Create individuals with data property assertions
    let alice = Individual::Named(NamedNode::new("http://example.org/Alice")?);
    let bob = Individual::Named(NamedNode::new("http://example.org/Bob")?);
    let charlie = Individual::Named(NamedNode::new("http://example.org/Charlie")?);

    // Alice - a person
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(person.clone()),
        alice.clone(),
    ));

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_first_name.clone(),
        source: alice.clone(),
        target: Literal::new_simple_literal("Alice"),
    });

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_last_name.clone(),
        source: alice.clone(),
        target: Literal::new_simple_literal("Smith"),
    });

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_age.clone(),
        source: alice.clone(),
        target: Literal::new_typed_literal("25", xsd::INTEGER),
    });

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_email.clone(),
        source: alice.clone(),
        target: Literal::new_simple_literal("alice@example.org"),
    });

    println!("\n✓ Created Alice (Person):");
    println!("  - hasFirstName: \"Alice\"");
    println!("  - hasLastName: \"Smith\"");
    println!("  - hasAge: 25");
    println!("  - hasEmail: \"alice@example.org\"");

    // Bob - an employee
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(employee.clone()),
        bob.clone(),
    ));

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_name.clone(),
        source: bob.clone(),
        target: Literal::new_simple_literal("Bob Johnson"),
    });

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_age.clone(),
        source: bob.clone(),
        target: Literal::new_typed_literal("32", xsd::INTEGER),
    });

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_email.clone(),
        source: bob.clone(),
        target: Literal::new_simple_literal("bob.johnson@example.org"),
    });

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_salary.clone(),
        source: bob.clone(),
        target: Literal::new_typed_literal("75000.00", xsd::DECIMAL),
    });

    println!("\n✓ Created Bob (Employee):");
    println!("  - hasName: \"Bob Johnson\"");
    println!("  - hasAge: 32");
    println!("  - hasEmail: \"bob.johnson@example.org\"");
    println!("  - hasSalary: 75000.00");

    // Charlie - a person with multiple emails (showing non-functional property)
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(person.clone()),
        charlie.clone(),
    ));

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_name.clone(),
        source: charlie.clone(),
        target: Literal::new_simple_literal("Charlie Brown"),
    });

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_age.clone(),
        source: charlie.clone(),
        target: Literal::new_typed_literal("28", xsd::INTEGER),
    });

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_email.clone(),
        source: charlie.clone(),
        target: Literal::new_simple_literal("charlie@example.org"),
    });

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_email.clone(),
        source: charlie.clone(),
        target: Literal::new_simple_literal("charlie.brown@example.org"),
    });

    println!("\n✓ Created Charlie (Person):");
    println!("  - hasName: \"Charlie Brown\"");
    println!("  - hasAge: 28");
    println!("  - hasEmail: \"charlie@example.org\"");
    println!("  - hasEmail: \"charlie.brown@example.org\" (multiple values allowed)");

    // Summary
    println!("\n=== Ontology Summary ===");
    println!("Classes: {}", ontology.classes().count());
    println!("Data Properties: {}", ontology.data_properties().count());
    println!("Individuals: {}", ontology.individuals().count());
    println!("Total Axioms: {}", ontology.axiom_count());

    println!("\n=== Data Property Characteristics ===");
    println!("Functional Properties:");
    println!("  - hasAge (each person has exactly one age)");
    println!("  - hasSalary (each employee has exactly one salary)");
    println!("\nNon-Functional Properties:");
    println!("  - hasEmail (a person can have multiple emails)");
    println!("  - hasName (a person can have multiple names)");

    println!("\n=== Data Property Hierarchy ===");
    println!("hasFirstName ⊑ hasName");
    println!("hasLastName ⊑ hasName");
    println!("(Alice's first/last names are also values of hasName)");

    Ok(())
}
