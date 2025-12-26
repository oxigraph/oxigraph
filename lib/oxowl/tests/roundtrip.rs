//! Round-trip tests for OWL parsing and serialization.
//!
//! These tests verify that ontologies can be parsed from RDF, serialized back to RDF,
//! and that the resulting graphs are semantically equivalent.

use oxowl::{
    parse_ontology, serialize_ontology, Axiom, ClassExpression, DataProperty, Individual,
    ObjectProperty, Ontology, OwlClass, SerializerConfig,
};
use oxrdf::{vocab::xsd, Graph, Literal, NamedNode, Triple};

#[test]
fn test_roundtrip_simple_subclass() {
    let mut ontology = Ontology::with_iri("http://example.org/test").unwrap();

    let animal = OwlClass::new(NamedNode::new_unchecked("http://example.org/Animal"));
    let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));

    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(dog.clone()),
        ClassExpression::class(animal.clone()),
    ));

    // Serialize
    let graph = serialize_ontology(&ontology);

    // Parse back
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    // Verify
    assert_eq!(parsed.axiom_count(), 1);
    assert!(matches!(
        parsed.axioms()[0],
        Axiom::SubClassOf { .. }
    ));
}

#[test]
fn test_roundtrip_class_assertion() {
    let mut ontology = Ontology::new(None);

    let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));
    let fido = Individual::Named(NamedNode::new_unchecked("http://example.org/fido"));

    ontology.add_axiom(Axiom::class_assertion(ClassExpression::class(dog), fido));

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    assert_eq!(parsed.axiom_count(), 1);
    assert!(matches!(
        parsed.axioms()[0],
        Axiom::ClassAssertion { .. }
    ));
}

#[test]
fn test_roundtrip_object_property_assertion() {
    let mut ontology = Ontology::new(None);

    let owns = ObjectProperty::new(NamedNode::new_unchecked("http://example.org/owns"));
    let alice = Individual::Named(NamedNode::new_unchecked("http://example.org/Alice"));
    let fido = Individual::Named(NamedNode::new_unchecked("http://example.org/fido"));

    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: owns,
        source: alice,
        target: fido,
    });

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    // Should have the property assertion
    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::ObjectPropertyAssertion { .. }) {
            found = true;
            break;
        }
    }
    assert!(found, "ObjectPropertyAssertion not found after round-trip");
}

#[test]
fn test_roundtrip_data_property_assertion() {
    let mut ontology = Ontology::new(None);

    let age = DataProperty::new(NamedNode::new_unchecked("http://example.org/age"));
    let alice = Individual::Named(NamedNode::new_unchecked("http://example.org/Alice"));
    let age_value = Literal::new_typed_literal("30", xsd::INTEGER);

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: age,
        source: alice,
        target: age_value,
    });

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::DataPropertyAssertion { .. }) {
            found = true;
            break;
        }
    }
    assert!(found, "DataPropertyAssertion not found after round-trip");
}

#[test]
fn test_roundtrip_equivalent_classes() {
    let mut ontology = Ontology::new(None);

    let human = OwlClass::new(NamedNode::new_unchecked("http://example.org/Human"));
    let person = OwlClass::new(NamedNode::new_unchecked("http://example.org/Person"));

    ontology.add_axiom(Axiom::equivalent_classes(vec![
        ClassExpression::class(human),
        ClassExpression::class(person),
    ]));

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::EquivalentClasses(_)) {
            found = true;
            break;
        }
    }
    assert!(found, "EquivalentClasses not found after round-trip");
}

#[test]
fn test_roundtrip_disjoint_classes() {
    let mut ontology = Ontology::new(None);

    let cat = OwlClass::new(NamedNode::new_unchecked("http://example.org/Cat"));
    let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));

    ontology.add_axiom(Axiom::disjoint_classes(vec![
        ClassExpression::class(cat),
        ClassExpression::class(dog),
    ]));

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::DisjointClasses(_)) {
            found = true;
            break;
        }
    }
    assert!(found, "DisjointClasses not found after round-trip");
}

#[test]
fn test_roundtrip_property_domain() {
    let mut ontology = Ontology::new(None);

    let owns = ObjectProperty::new(NamedNode::new_unchecked("http://example.org/owns"));
    let person = OwlClass::new(NamedNode::new_unchecked("http://example.org/Person"));

    ontology.add_axiom(Axiom::ObjectPropertyDomain {
        property: owns,
        domain: ClassExpression::class(person),
    });

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::ObjectPropertyDomain { .. }) {
            found = true;
            break;
        }
    }
    assert!(found, "ObjectPropertyDomain not found after round-trip");
}

#[test]
fn test_roundtrip_property_range() {
    let mut ontology = Ontology::new(None);

    let owns = ObjectProperty::new(NamedNode::new_unchecked("http://example.org/owns"));
    let animal = OwlClass::new(NamedNode::new_unchecked("http://example.org/Animal"));

    ontology.add_axiom(Axiom::ObjectPropertyRange {
        property: owns,
        range: ClassExpression::class(animal),
    });

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::ObjectPropertyRange { .. }) {
            found = true;
            break;
        }
    }
    assert!(found, "ObjectPropertyRange not found after round-trip");
}

#[test]
fn test_roundtrip_transitive_property() {
    let mut ontology = Ontology::new(None);

    let ancestor_of =
        ObjectProperty::new(NamedNode::new_unchecked("http://example.org/ancestorOf"));

    ontology.add_axiom(Axiom::TransitiveObjectProperty(ancestor_of));

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::TransitiveObjectProperty(_)) {
            found = true;
            break;
        }
    }
    assert!(found, "TransitiveObjectProperty not found after round-trip");
}

#[test]
fn test_roundtrip_functional_property() {
    let mut ontology = Ontology::new(None);

    let has_father =
        ObjectProperty::new(NamedNode::new_unchecked("http://example.org/hasFather"));

    ontology.add_axiom(Axiom::FunctionalObjectProperty(has_father));

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::FunctionalObjectProperty(_)) {
            found = true;
            break;
        }
    }
    assert!(found, "FunctionalObjectProperty not found after round-trip");
}

#[test]
fn test_roundtrip_symmetric_property() {
    let mut ontology = Ontology::new(None);

    let married_to =
        ObjectProperty::new(NamedNode::new_unchecked("http://example.org/marriedTo"));

    ontology.add_axiom(Axiom::SymmetricObjectProperty(married_to));

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::SymmetricObjectProperty(_)) {
            found = true;
            break;
        }
    }
    assert!(found, "SymmetricObjectProperty not found after round-trip");
}

#[test]
fn test_roundtrip_inverse_properties() {
    let mut ontology = Ontology::new(None);

    let has_parent =
        ObjectProperty::new(NamedNode::new_unchecked("http://example.org/hasParent"));
    let has_child = ObjectProperty::new(NamedNode::new_unchecked("http://example.org/hasChild"));

    ontology.add_axiom(Axiom::InverseObjectProperties(has_parent, has_child));

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::InverseObjectProperties(_, _)) {
            found = true;
            break;
        }
    }
    assert!(found, "InverseObjectProperties not found after round-trip");
}

#[test]
fn test_roundtrip_same_individual() {
    let mut ontology = Ontology::new(None);

    let alice1 = Individual::Named(NamedNode::new_unchecked("http://example.org/Alice"));
    let alice2 = Individual::Named(NamedNode::new_unchecked("http://example.org/AliceSmith"));

    ontology.add_axiom(Axiom::SameIndividual(vec![alice1, alice2]));

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::SameIndividual(_)) {
            found = true;
            break;
        }
    }
    assert!(found, "SameIndividual not found after round-trip");
}

#[test]
fn test_roundtrip_different_individuals() {
    let mut ontology = Ontology::new(None);

    let alice = Individual::Named(NamedNode::new_unchecked("http://example.org/Alice"));
    let bob = Individual::Named(NamedNode::new_unchecked("http://example.org/Bob"));

    ontology.add_axiom(Axiom::DifferentIndividuals(vec![alice, bob]));

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found = false;
    for axiom in parsed.axioms() {
        if matches!(axiom, Axiom::DifferentIndividuals(_)) {
            found = true;
            break;
        }
    }
    assert!(found, "DifferentIndividuals not found after round-trip");
}

#[test]
fn test_roundtrip_restriction_some_values_from() {
    let mut ontology = Ontology::new(None);

    let has_pet = ObjectProperty::new(NamedNode::new_unchecked("http://example.org/hasPet"));
    let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));
    let dog_owner = OwlClass::new(NamedNode::new_unchecked("http://example.org/DogOwner"));

    // DogOwner ⊑ ∃hasPet.Dog
    ontology.add_axiom(Axiom::SubClassOf {
        sub_class: ClassExpression::class(dog_owner),
        super_class: ClassExpression::some_values_from(has_pet, ClassExpression::class(dog)),
    });

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    // Verify the restriction is present
    let mut found_restriction = false;
    for axiom in parsed.axioms() {
        if let Axiom::SubClassOf {
            sub_class: _,
            super_class,
        } = axiom
        {
            if matches!(super_class, ClassExpression::ObjectSomeValuesFrom { .. }) {
                found_restriction = true;
                break;
            }
        }
    }
    assert!(
        found_restriction,
        "ObjectSomeValuesFrom restriction not found after round-trip"
    );
}

#[test]
fn test_roundtrip_restriction_all_values_from() {
    let mut ontology = Ontology::new(None);

    let has_pet = ObjectProperty::new(NamedNode::new_unchecked("http://example.org/hasPet"));
    let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));
    let dog_only_owner =
        OwlClass::new(NamedNode::new_unchecked("http://example.org/DogOnlyOwner"));

    // DogOnlyOwner ⊑ ∀hasPet.Dog
    ontology.add_axiom(Axiom::SubClassOf {
        sub_class: ClassExpression::class(dog_only_owner),
        super_class: ClassExpression::all_values_from(has_pet, ClassExpression::class(dog)),
    });

    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    let mut found_restriction = false;
    for axiom in parsed.axioms() {
        if let Axiom::SubClassOf {
            sub_class: _,
            super_class,
        } = axiom
        {
            if matches!(super_class, ClassExpression::ObjectAllValuesFrom { .. }) {
                found_restriction = true;
                break;
            }
        }
    }
    assert!(
        found_restriction,
        "ObjectAllValuesFrom restriction not found after round-trip"
    );
}

#[test]
fn test_roundtrip_complex_ontology() {
    let mut ontology = Ontology::with_iri("http://example.org/animals").unwrap();

    // Classes
    let animal = OwlClass::new(NamedNode::new_unchecked("http://example.org/Animal"));
    let mammal = OwlClass::new(NamedNode::new_unchecked("http://example.org/Mammal"));
    let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));
    let cat = OwlClass::new(NamedNode::new_unchecked("http://example.org/Cat"));

    // Properties
    let has_owner = ObjectProperty::new(NamedNode::new_unchecked("http://example.org/hasOwner"));
    let age = DataProperty::new(NamedNode::new_unchecked("http://example.org/age"));

    // Individuals
    let fido = Individual::Named(NamedNode::new_unchecked("http://example.org/fido"));
    let alice = Individual::Named(NamedNode::new_unchecked("http://example.org/Alice"));

    // Class hierarchy
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(mammal.clone()),
        ClassExpression::class(animal),
    ));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(dog.clone()),
        ClassExpression::class(mammal.clone()),
    ));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(cat.clone()),
        ClassExpression::class(mammal),
    ));

    // Disjoint classes
    ontology.add_axiom(Axiom::disjoint_classes(vec![
        ClassExpression::class(dog.clone()),
        ClassExpression::class(cat),
    ]));

    // Assertions
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(dog),
        fido.clone(),
    ));

    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_owner,
        source: fido.clone(),
        target: alice.clone(),
    });

    ontology.add_axiom(Axiom::DataPropertyAssertion {
        property: age,
        source: fido,
        target: Literal::new_typed_literal("5", xsd::INTEGER),
    });

    // Serialize and parse back
    let graph = serialize_ontology(&ontology);
    let parsed = parse_ontology(&graph).expect("Failed to parse");

    // Should have multiple axioms (at least the ones we added, possibly more from parsing)
    assert!(parsed.axiom_count() >= 6, "Expected at least 6 axioms");

    // Verify ontology IRI is preserved
    assert_eq!(
        parsed.iri().map(|iri| iri.as_str()),
        Some("http://example.org/animals")
    );
}

#[test]
fn test_serializer_config_include_declarations() {
    let mut ontology = Ontology::new(None);

    let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));
    ontology.add_axiom(Axiom::DeclareClass(dog.clone()));

    // With declarations
    let config = SerializerConfig::new().include_declarations(true);
    let graph = ontology.to_graph_with_config(config);

    let mut found = false;
    for triple in graph.iter() {
        if let oxrdf::TermRef::NamedNode(obj) = triple.object {
            if obj.as_str() == "http://www.w3.org/2002/07/owl#Class" {
                found = true;
                break;
            }
        }
    }
    assert!(found, "Declaration should be included");

    // Without declarations (default)
    let config = SerializerConfig::new().include_declarations(false);
    let graph = ontology.to_graph_with_config(config);

    let mut found = false;
    for triple in graph.iter() {
        if let oxrdf::TermRef::NamedNode(obj) = triple.object {
            if obj.as_str() == "http://www.w3.org/2002/07/owl#Class" {
                found = true;
                break;
            }
        }
    }
    assert!(!found, "Declaration should not be included");
}

#[test]
fn test_ontology_to_graph_method() {
    let mut ontology = Ontology::with_iri("http://example.org/test").unwrap();

    let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));
    let animal = OwlClass::new(NamedNode::new_unchecked("http://example.org/Animal"));

    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(dog),
        ClassExpression::class(animal),
    ));

    // Use the convenience method
    let graph = ontology.to_graph();

    assert!(graph.len() > 0, "Graph should contain triples");
}
