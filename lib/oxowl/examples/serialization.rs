//! Example demonstrating OWL ontology serialization to RDF.
//!
//! This example shows how to create an OWL ontology, add axioms, and serialize
//! it to RDF format that can be written to Turtle, RDF/XML, etc.

use oxowl::{
    serialize_ontology, Axiom, ClassExpression, DataProperty, Individual, ObjectProperty,
    Ontology, OwlClass, SerializerConfig,
};
use oxrdf::{vocab::xsd, Literal, NamedNode};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Create an ontology
    let mut ontology = Ontology::with_iri("http://example.org/university")?;

    // Define classes
    let person = OwlClass::new(NamedNode::new("http://example.org/Person")?);
    let student = OwlClass::new(NamedNode::new("http://example.org/Student")?);
    let professor = OwlClass::new(NamedNode::new("http://example.org/Professor")?);
    let course = OwlClass::new(NamedNode::new("http://example.org/Course")?);

    // Define properties
    let enrolls_in =
        ObjectProperty::new(NamedNode::new("http://example.org/enrollsIn")?);
    let teaches = ObjectProperty::new(NamedNode::new("http://example.org/teaches")?);
    let has_name = DataProperty::new(NamedNode::new("http://example.org/hasName")?);
    let age = DataProperty::new(NamedNode::new("http://example.org/age")?);

    // Define individuals
    let alice = Individual::Named(NamedNode::new("http://example.org/Alice")?);
    let bob = Individual::Named(NamedNode::new("http://example.org/Bob")?);
    let cs101 = Individual::Named(NamedNode::new("http://example.org/CS101")?);

    // === Class Hierarchy ===
    // Student ⊑ Person
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(student.clone()),
        ClassExpression::class(person.clone()),
    ));

    // Professor ⊑ Person
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(professor.clone()),
        ClassExpression::class(person),
    ));

    // Student and Professor are disjoint
    ontology.add_axiom(Axiom::disjoint_classes(vec![
        ClassExpression::class(student.clone()),
        ClassExpression::class(professor.clone()),
    ]));

    // === Property Axioms ===
    // enrollsIn has domain Student
    ontology.add_axiom(Axiom::ObjectPropertyDomain {
        property: enrolls_in.clone(),
        domain: ClassExpression::class(student.clone()),
    });

    // enrollsIn has range Course
    ontology.add_axiom(Axiom::ObjectPropertyRange {
        property: enrolls_in.clone(),
        range: ClassExpression::class(course.clone()),
    });

    // teaches has domain Professor
    ontology.add_axiom(Axiom::ObjectPropertyDomain {
        property: teaches.clone(),
        domain: ClassExpression::class(professor),
    });

    // teaches has range Course
    ontology.add_axiom(Axiom::ObjectPropertyRange {
        property: teaches.clone(),
        range: ClassExpression::class(course),
    });

    // === Individual Assertions ===
    // Alice is a Student
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(student),
        alice.clone(),
    ));

    // Alice enrolls in CS101
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: enrolls_in,
        source: alice.clone(),
        target: cs101,
    });

    // Alice has name "Alice"
    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: has_name,
        source: alice.clone(),
        target: Literal::new_simple_literal("Alice"),
    });

    // Alice has age 20
    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: age,
        source: alice,
        target: Literal::new_typed_literal("20", xsd::INTEGER),
    });

    // === Complex Class Expression ===
    // Create a restriction: ∃enrollsIn.Course (someone who enrolls in some course)
    let enrolled_student = ClassExpression::some_values_from(
        ObjectProperty::new(NamedNode::new("http://example.org/enrollsIn")?),
        ClassExpression::class(OwlClass::new(NamedNode::new("http://example.org/Course")?)),
    );

    // EnrolledStudent ≡ Student ⊓ ∃enrollsIn.Course
    ontology.add_axiom(Axiom::SubClassOf {
        sub_class: ClassExpression::class(OwlClass::new(NamedNode::new(
            "http://example.org/EnrolledStudent",
        )?)),
        super_class: ClassExpression::intersection(vec![
            ClassExpression::class(OwlClass::new(NamedNode::new("http://example.org/Student")?)),
            enrolled_student,
        ]),
    });

    println!("Ontology created with {} axioms", ontology.axiom_count());

    // === Serialize to RDF ===
    println!("\n=== Serializing to RDF ===\n");

    // Method 1: Using the convenience method
    let graph = ontology.to_graph();
    println!("Generated {} RDF triples", graph.len());

    // Method 2: Using custom configuration
    let config = SerializerConfig::new().include_declarations(true);
    let graph_with_decls = ontology.to_graph_with_config(config);
    println!(
        "Generated {} RDF triples (with declarations)",
        graph_with_decls.len()
    );

    // Method 3: Using the standalone function
    let graph2 = serialize_ontology(&ontology);
    println!("Generated {} RDF triples (standalone)", graph2.len());

    // Print a sample of the generated triples
    println!("\n=== Sample RDF Triples ===\n");
    for (i, triple) in graph.iter().take(10).enumerate() {
        println!("{}: {} {} {}", i + 1, triple.subject, triple.predicate, triple.object);
    }

    // === Write to Turtle format ===
    // Note: In a real application, you would use oxrdfio to serialize to Turtle
    // For example:
    // use oxrdfio::RdfSerializer;
    // let mut writer = RdfSerializer::from_format(RdfFormat::Turtle)
    //     .serialize_to_write(std::io::stdout());
    // for triple in graph.iter() {
    //     writer.write_triple(triple)?;
    // }

    println!("\n=== Round-trip Test ===\n");

    // Parse the graph back to an ontology
    let parsed = oxowl::parse_ontology(&graph)?;
    println!("Parsed back {} axioms", parsed.axiom_count());

    // Verify the ontology IRI is preserved
    if let Some(iri) = parsed.iri() {
        println!("Ontology IRI: {}", iri);
    }

    Ok(())
}
