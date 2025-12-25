//! Integration tests for N3 and OWL interoperability.

use oxowl::n3_integration::{parse_n3_ontology, parse_n3_ontology_with_config};
use oxowl::n3_rules::{extend_ontology_with_n3_rules, N3Rule, N3RuleExtractor};
use oxowl::{parse_ontology_from_n3, Axiom, ClassExpression, Individual, Ontology, ParserConfig};
use oxrdf::{BlankNode, Formula, NamedNode, Quad, Triple};
use oxrdf::vocab::rdf;

#[test]
fn test_load_simple_ontology_from_n3() {
    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <http://example.org/> .

ex:Ontology a owl:Ontology .

ex:Animal a owl:Class .
ex:Dog a owl:Class .
ex:Cat a owl:Class .

ex:Dog rdfs:subClassOf ex:Animal .
ex:Cat rdfs:subClassOf ex:Animal .
"#;

    let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();

    // Should have class declarations
    assert!(ontology.axiom_count() >= 3);

    // Check for subclass axioms
    let has_dog_subclass = ontology.axioms().iter().any(|ax| {
        matches!(
            ax,
            Axiom::SubClassOf { sub_class, super_class }
                if matches!(sub_class, ClassExpression::Class(c) if c.iri().as_str().contains("Dog"))
                && matches!(super_class, ClassExpression::Class(c) if c.iri().as_str().contains("Animal"))
        )
    });
    assert!(has_dog_subclass, "Should have Dog subClassOf Animal");
}

#[test]
fn test_load_ontology_with_individuals() {
    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <http://example.org/> .

ex:Dog a owl:Class .
ex:fido a ex:Dog .
ex:rex a ex:Dog .
"#;

    let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();

    // Should have class declaration and individual assertions
    let has_fido = ontology.axioms().iter().any(|ax| {
        matches!(
            ax,
            Axiom::ClassAssertion { class, individual }
                if matches!(class, ClassExpression::Class(c) if c.iri().as_str().contains("Dog"))
                && matches!(individual, Individual::Named(n) if n.as_str().contains("fido"))
        )
    });
    assert!(has_fido, "Should have fido as instance of Dog");
}

#[test]
fn test_load_ontology_with_properties() {
    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <http://example.org/> .

ex:hasOwner a owl:ObjectProperty .
ex:hasFriend a owl:ObjectProperty .

ex:hasOwner rdfs:domain ex:Dog .
ex:hasOwner rdfs:range ex:Person .

ex:hasFriend a owl:SymmetricProperty .
ex:hasFriend a owl:TransitiveProperty .
"#;

    let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();

    // Should have property declarations and characteristics
    assert!(ontology.axiom_count() > 0);

    // Check for symmetric property
    let has_symmetric = ontology.axioms().iter().any(|ax| {
        matches!(ax, Axiom::SymmetricObjectProperty(p) if p.iri().as_str().contains("hasFriend"))
    });
    assert!(has_symmetric, "Should have hasFriend as symmetric");

    // Check for transitive property
    let has_transitive = ontology.axioms().iter().any(|ax| {
        matches!(ax, Axiom::TransitiveObjectProperty(p) if p.iri().as_str().contains("hasFriend"))
    });
    assert!(has_transitive, "Should have hasFriend as transitive");
}

#[test]
fn test_n3_with_prefixes() {
    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix : <http://example.org/animals#> .

:Mammal a owl:Class .
:Dog a owl:Class .
:Dog rdfs:subClassOf :Mammal .
"#;

    let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();
    assert!(ontology.axiom_count() >= 2);
}

#[test]
fn test_n3_with_blank_nodes() {
    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix ex: <http://example.org/> .

ex:Dog a owl:Class .

# Anonymous individual
_:b1 a ex:Dog .
_:b2 a ex:Dog .
"#;

    let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();

    // Should have anonymous individuals
    let has_anonymous = ontology.axioms().iter().any(|ax| {
        matches!(
            ax,
            Axiom::ClassAssertion { individual, .. }
                if matches!(individual, Individual::Anonymous(_))
        )
    });
    assert!(has_anonymous, "Should have anonymous individuals");
}

#[test]
fn test_parse_ontology_from_n3_convenience() {
    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix ex: <http://example.org/> .

ex:TestClass a owl:Class .
"#;

    // Test the convenience function from parser module
    let ontology = parse_ontology_from_n3(n3_data.as_bytes()).unwrap();
    assert!(ontology.axiom_count() >= 1);
}

#[test]
fn test_parse_with_lenient_config() {
    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix ex: <http://example.org/> .

# This uses a class without explicit declaration
ex:fido a ex:Dog .
"#;

    let config = ParserConfig::new().lenient();
    let ontology = parse_n3_ontology_with_config(n3_data.as_bytes(), config).unwrap();
    assert!(ontology.axiom_count() > 0);
}

#[test]
fn test_n3_formulas_extraction() {
    use oxowl::n3_integration::formulas::extract_formulas;
    use oxrdf::{GraphName, NamedOrBlankNode, Term};

    let bn = BlankNode::new("f1").unwrap();
    let ex = NamedNode::new("http://example.org/test").unwrap();

    let quad = Quad {
        subject: NamedOrBlankNode::NamedNode(ex.clone()),
        predicate: ex.clone(),
        object: Term::NamedNode(ex),
        graph_name: GraphName::BlankNode(bn.clone()),
    };

    let formulas = extract_formulas(&[quad]);
    assert_eq!(formulas.len(), 1);
    assert_eq!(formulas[0].id(), &bn);
}

#[test]
fn test_n3_rule_subclass_pattern() {
    let dog = NamedNode::new("http://example.org/Dog").unwrap();
    let animal = NamedNode::new("http://example.org/Animal").unwrap();
    let var = BlankNode::new("x").unwrap();

    // Create rule: { ?x a :Dog } => { ?x a :Animal }
    let ant_triple = Triple::new(var.clone(), rdf::TYPE, dog.clone());
    let cons_triple = Triple::new(var, rdf::TYPE, animal.clone());

    let ant_formula = Formula::new(BlankNode::default(), vec![ant_triple]);
    let cons_formula = Formula::new(BlankNode::default(), vec![cons_triple]);

    let rule = N3Rule::new(ant_formula, cons_formula);

    // Should be recognized as a subclass pattern
    assert!(rule.is_subclass_pattern());
    assert!(rule.is_owl_compatible());

    // Should generate a SubClassOf axiom
    let axioms = rule.to_owl_axioms();
    assert_eq!(axioms.len(), 1);

    match &axioms[0] {
        Axiom::SubClassOf { sub_class, super_class } => {
            assert!(matches!(sub_class, ClassExpression::Class(c) if c.iri() == &dog));
            assert!(matches!(super_class, ClassExpression::Class(c) if c.iri() == &animal));
        }
        _ => panic!("Expected SubClassOf axiom"),
    }
}

#[test]
fn test_n3_rule_extractor() {
    let extractor = N3RuleExtractor::new(vec![]);
    let rules = extractor.extract_rules();
    assert_eq!(rules.len(), 0);
}

#[test]
fn test_extend_ontology_with_rules() {
    let mut ontology = Ontology::new(None);
    let initial_count = ontology.axiom_count();

    // Empty quads should not add any axioms
    let added = extend_ontology_with_n3_rules(&mut ontology, &[]);
    assert_eq!(added, 0);
    assert_eq!(ontology.axiom_count(), initial_count);
}

#[test]
fn test_complex_ontology_with_multiple_axiom_types() {
    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <http://example.org/> .

# Ontology declaration
ex:PetOntology a owl:Ontology ;
    owl:versionIRI ex:PetOntology-v1.0 .

# Class hierarchy
ex:Animal a owl:Class .
ex:Mammal a owl:Class .
ex:Dog a owl:Class .
ex:Cat a owl:Class .
ex:Person a owl:Class .

ex:Mammal rdfs:subClassOf ex:Animal .
ex:Dog rdfs:subClassOf ex:Mammal .
ex:Cat rdfs:subClassOf ex:Mammal .

# Properties
ex:hasOwner a owl:ObjectProperty ;
    rdfs:domain ex:Dog ;
    rdfs:range ex:Person .

ex:hasFriend a owl:ObjectProperty ,
              owl:SymmetricProperty .

# Individuals
ex:fido a ex:Dog .
ex:alice a ex:Person .

# Property assertions
ex:fido ex:hasOwner ex:alice .

# Disjoint classes
ex:Dog owl:disjointWith ex:Cat .
"#;

    let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();

    // Verify ontology IRI
    assert!(ontology.iri().is_some());
    assert!(ontology.iri().unwrap().as_str().contains("PetOntology"));

    // Verify version IRI
    assert!(ontology.version_iri().is_some());

    // Should have substantial number of axioms
    assert!(ontology.axiom_count() >= 10);

    // Check for specific axioms
    let has_mammal_subclass = ontology.axioms().iter().any(|ax| {
        matches!(
            ax,
            Axiom::SubClassOf { sub_class, super_class }
                if matches!(sub_class, ClassExpression::Class(c) if c.iri().as_str().contains("Mammal"))
                && matches!(super_class, ClassExpression::Class(c) if c.iri().as_str().contains("Animal"))
        )
    });
    assert!(has_mammal_subclass);

    // Check for disjoint classes
    let has_disjoint = ontology.axioms().iter().any(|ax| {
        matches!(ax, Axiom::DisjointClasses(_))
    });
    assert!(has_disjoint);

    // Note: Property assertions (ex:fido ex:hasOwner ex:alice) are currently
    // parsed as general triples but not yet converted to ObjectPropertyAssertion axioms.
    // This is a future enhancement for the OWL parser.
    // For now, we verify that the ontology at least has the expected structure.

    // The ontology should have been successfully parsed with multiple axiom types
    assert!(ontology.axiom_count() >= 10, "Should have at least 10 axioms");
}

#[test]
fn test_n3_with_multiple_prefixes() {
    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix foaf: <http://xmlns.com/foaf/0.1/> .
@prefix ex: <http://example.org/> .
@prefix : <http://default.example.org/> .

:Thing a owl:Class .
foaf:Person a owl:Class .
ex:Employee a owl:Class .

ex:Employee rdfs:subClassOf foaf:Person .
"#;

    let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();
    assert!(ontology.axiom_count() >= 3);
}

#[test]
fn test_error_handling_invalid_n3() {
    let invalid_n3 = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .

# Missing closing bracket
ex:Class a owl:Class
"#;

    let result = parse_n3_ontology(invalid_n3.as_bytes());
    assert!(result.is_err());
}

#[test]
fn test_n3_with_literals() {
    let n3_data = r#"
@prefix owl: <http://www.w3.org/2002/07/owl#> .
@prefix rdfs: <http://www.w3.org/2000/01/rdf-schema#> .
@prefix ex: <http://example.org/> .

ex:Person a owl:Class ;
    rdfs:label "Person"@en ;
    rdfs:comment "A human being" .

ex:Dog a owl:Class ;
    rdfs:label "Dog"@en ,
               "Hund"@de .
"#;

    let ontology = parse_n3_ontology(n3_data.as_bytes()).unwrap();
    assert!(ontology.axiom_count() >= 2);
}
