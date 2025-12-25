//! Example demonstrating basic RDF term creation and manipulation
//!
//! This example shows how to:
//! - Create NamedNodes (IRIs)
//! - Create BlankNodes
//! - Create Literals (simple, typed, and language-tagged)
//! - Build Triples and Quads
//! - Display and compare RDF terms
//!
//! Run with: cargo run -p oxrdf --example basic_rdf

use oxrdf::*;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Basic RDF Terms Example ===\n");

    // ==========================================
    // Creating Named Nodes (IRIs)
    // ==========================================
    println!("1. Creating Named Nodes (IRIs):");

    // Named nodes represent IRIs in RDF
    let subject = NamedNode::new("http://example.org/alice")?;
    let predicate = NamedNode::new("http://xmlns.com/foaf/0.1/name")?;
    let type_predicate = NamedNode::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?;
    let person_class = NamedNode::new("http://xmlns.com/foaf/0.1/Person")?;

    println!("  Subject: {}", subject);
    println!("  Predicate: {}", predicate);
    println!("  Type predicate: {}", type_predicate);
    println!("  Person class: {}", person_class);
    println!();

    // ==========================================
    // Creating Blank Nodes
    // ==========================================
    println!("2. Creating Blank Nodes:");

    // Blank nodes are anonymous nodes with unique identifiers
    // Use default() to generate a random, unique blank node
    let blank1 = BlankNode::default();
    let blank2 = BlankNode::default();

    // Or create a blank node with a specific identifier
    let blank_named = BlankNode::new("b1")?;

    println!("  Random blank node 1: {}", blank1);
    println!("  Random blank node 2: {}", blank2);
    println!("  Named blank node: {}", blank_named);
    println!("  Blank nodes are unique: {}", blank1 != blank2);
    println!();

    // ==========================================
    // Creating Literals
    // ==========================================
    println!("3. Creating Literals:");

    // Simple literal (plain string)
    let simple_literal = Literal::new_simple_literal("Alice");
    println!("  Simple literal: {}", simple_literal);

    // Typed literal with xsd:integer datatype
    let integer_literal = Literal::new_typed_literal("42", vocab::xsd::INTEGER);
    println!("  Integer literal: {}", integer_literal);

    // Typed literal with xsd:date datatype
    let date_literal = Literal::new_typed_literal("2024-01-15", vocab::xsd::DATE);
    println!("  Date literal: {}", date_literal);

    // Typed literal with xsd:boolean datatype
    let boolean_literal = Literal::new_typed_literal("true", vocab::xsd::BOOLEAN);
    println!("  Boolean literal: {}", boolean_literal);

    // Language-tagged string (for multilingual content)
    let lang_literal_en = Literal::new_language_tagged_literal("Alice", "en")?;
    let lang_literal_fr = Literal::new_language_tagged_literal("Alice", "fr")?;
    println!("  Language-tagged (English): {}", lang_literal_en);
    println!("  Language-tagged (French): {}", lang_literal_fr);

    // Literals can be created from Rust native types
    let from_int = Literal::from(42);
    let from_bool = Literal::from(true);
    let from_float = Literal::from(3.14);
    println!("  From Rust integer: {}", from_int);
    println!("  From Rust boolean: {}", from_bool);
    println!("  From Rust float: {}", from_float);
    println!();

    // ==========================================
    // Accessing Literal Properties
    // ==========================================
    println!("4. Accessing Literal Properties:");

    println!("  Value of simple literal: '{}'", simple_literal.value());
    println!("  Datatype of simple literal: {}", simple_literal.datatype());
    println!("  Value of integer literal: '{}'", integer_literal.value());
    println!("  Datatype of integer literal: {}", integer_literal.datatype());
    println!("  Language of English literal: {:?}", lang_literal_en.language());
    println!();

    // ==========================================
    // Creating Triples
    // ==========================================
    println!("5. Creating Triples:");

    // A triple consists of subject, predicate, and object
    let triple1 = Triple::new(
        subject.clone(),
        predicate.clone(),
        simple_literal.clone(),
    );
    println!("  Triple 1: {}", triple1);

    // Triple with a blank node as subject
    let triple2 = Triple::new(
        blank1.clone(),
        type_predicate.clone(),
        person_class.clone(),
    );
    println!("  Triple 2: {}", triple2);

    // Triple with a blank node as object
    let triple3 = Triple::new(
        subject.clone(),
        NamedNode::new("http://xmlns.com/foaf/0.1/knows")?,
        blank1.clone(),
    );
    println!("  Triple 3: {}", triple3);
    println!();

    // ==========================================
    // Creating Quads (Triples with Graph Names)
    // ==========================================
    println!("6. Creating Quads:");

    // Quads extend triples with a graph name for dataset context
    let graph_name = NamedNode::new("http://example.org/graph1")?;

    // Quad with a named graph
    let quad1 = Quad::new(
        subject.clone(),
        predicate.clone(),
        simple_literal.clone(),
        graph_name.clone(),
    );
    println!("  Quad in named graph: {}", quad1);

    // Quad in the default graph
    let quad2 = Quad::new(
        blank1.clone(),
        type_predicate.clone(),
        person_class.clone(),
        GraphName::DefaultGraph,
    );
    println!("  Quad in default graph: {}", quad2);

    // Convert a triple to a quad by specifying its graph
    let quad_from_triple = triple1.in_graph(graph_name.clone());
    println!("  Quad from triple: {}", quad_from_triple);
    println!();

    // ==========================================
    // Working with Terms (Union Type)
    // ==========================================
    println!("7. Working with Terms:");

    // Term is a union type that can be NamedNode, BlankNode, or Literal
    let term_named: Term = subject.clone().into();
    let term_blank: Term = blank2.clone().into();
    let term_literal: Term = simple_literal.clone().into();

    println!("  Term from NamedNode: {}", term_named);
    println!("  Term from BlankNode: {}", term_blank);
    println!("  Term from Literal: {}", term_literal);

    // Check term types
    println!("  Is term_named a named node? {}", term_named.is_named_node());
    println!("  Is term_blank a blank node? {}", term_blank.is_blank_node());
    println!("  Is term_literal a literal? {}", term_literal.is_literal());
    println!();

    // ==========================================
    // Comparing and Pattern Matching
    // ==========================================
    println!("8. Comparing and Pattern Matching:");

    // Literals can be compared
    let literal_a = Literal::new_simple_literal("Alice");
    let literal_b = Literal::new_simple_literal("Alice");
    let literal_c = Literal::new_simple_literal("Bob");

    println!("  literal_a == literal_b: {}", literal_a == literal_b);
    println!("  literal_a == literal_c: {}", literal_a == literal_c);

    // Pattern matching on Term
    match term_literal {
        Term::NamedNode(n) => println!("  It's a named node: {}", n),
        Term::BlankNode(b) => println!("  It's a blank node: {}", b),
        Term::Literal(l) => println!("  It's a literal with value: '{}'", l.value()),
        #[cfg(feature = "rdf-12")]
        Term::Triple(_) => println!("  It's a triple (RDF-star)"),
    }
    println!();

    // ==========================================
    // Using Vocabulary Constants
    // ==========================================
    println!("9. Using Vocabulary Constants:");

    // The vocab module provides common RDF vocabulary constants
    println!("  RDF type: {}", vocab::rdf::TYPE);
    println!("  RDFS label: {}", vocab::rdfs::LABEL);
    println!("  XSD string: {}", vocab::xsd::STRING);
    println!("  XSD integer: {}", vocab::xsd::INTEGER);
    println!("  XSD boolean: {}", vocab::xsd::BOOLEAN);
    println!();

    // ==========================================
    // Error Handling
    // ==========================================
    println!("10. Error Handling:");

    // Invalid IRIs will return an error
    match NamedNode::new("not a valid iri") {
        Ok(node) => println!("  Created node: {}", node),
        Err(e) => println!("  Error creating invalid IRI: {}", e),
    }

    // Invalid blank node identifiers will return an error
    match BlankNode::new("") {
        Ok(node) => println!("  Created blank node: {}", node),
        Err(e) => println!("  Error creating invalid blank node: {}", e),
    }

    // Invalid language tags will return an error
    match Literal::new_language_tagged_literal("Hello", "invalid-lang-tag-!!!") {
        Ok(lit) => println!("  Created literal: {}", lit),
        Err(e) => println!("  Error creating invalid language tag: {}", e),
    }

    println!("\n=== Example Complete ===");

    Ok(())
}
