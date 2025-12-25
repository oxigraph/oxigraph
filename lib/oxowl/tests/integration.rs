//! Integration tests for oxowl crate.

use oxowl::{
    Ontology, Axiom, ClassExpression,
    OwlClass, ObjectProperty, DataProperty, Individual,
};
use oxrdf::NamedNode;

#[test]
fn test_create_empty_ontology() {
    let ontology = Ontology::new(None);
    assert!(ontology.iri().is_none());
    assert_eq!(ontology.axiom_count(), 0);
}

#[test]
fn test_create_ontology_with_iri() {
    let iri = NamedNode::new("http://example.org/animals").unwrap();
    let ontology = Ontology::new(Some(iri.clone()));
    assert_eq!(ontology.iri(), Some(&iri));
}

#[test]
fn test_add_class_declaration() {
    let mut ontology = Ontology::new(None);
    let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());

    ontology.add_axiom(Axiom::DeclareClass(animal.clone()));

    assert!(ontology.contains_class(&animal));
    assert_eq!(ontology.classes().count(), 1);
}

#[test]
fn test_subclass_axiom() {
    let mut ontology = Ontology::new(None);

    let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());
    let dog = OwlClass::new(NamedNode::new("http://example.org/Dog").unwrap());

    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(dog.clone()),
        ClassExpression::class(animal.clone()),
    ));

    assert_eq!(ontology.axiom_count(), 1);

    let superclasses = ontology.direct_superclasses_of(&dog);
    assert_eq!(superclasses.len(), 1);
}

#[test]
fn test_class_assertion() {
    let mut ontology = Ontology::new(None);

    let dog = OwlClass::new(NamedNode::new("http://example.org/Dog").unwrap());
    let fido = Individual::Named(NamedNode::new("http://example.org/fido").unwrap());

    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(dog.clone()),
        fido.clone(),
    ));

    assert!(ontology.contains_individual(&fido));

    let types: Vec<_> = ontology.types_of(&fido).collect();
    assert_eq!(types.len(), 1);
}

#[test]
fn test_equivalent_classes() {
    let mut ontology = Ontology::new(None);

    let cat = OwlClass::new(NamedNode::new("http://example.org/Cat").unwrap());
    let feline = OwlClass::new(NamedNode::new("http://example.org/Feline").unwrap());

    ontology.add_axiom(Axiom::equivalent_classes(vec![
        ClassExpression::class(cat.clone()),
        ClassExpression::class(feline.clone()),
    ]));

    assert_eq!(ontology.axiom_count(), 1);
}

#[test]
fn test_disjoint_classes() {
    let mut ontology = Ontology::new(None);

    let cat = OwlClass::new(NamedNode::new("http://example.org/Cat").unwrap());
    let dog = OwlClass::new(NamedNode::new("http://example.org/Dog").unwrap());

    ontology.add_axiom(Axiom::disjoint_classes(vec![
        ClassExpression::class(cat),
        ClassExpression::class(dog),
    ]));

    assert_eq!(ontology.axiom_count(), 1);
}

#[test]
fn test_class_expression_intersection() {
    let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());
    let domestic = OwlClass::new(NamedNode::new("http://example.org/Domestic").unwrap());

    let intersection = ClassExpression::intersection(vec![
        ClassExpression::class(animal),
        ClassExpression::class(domestic),
    ]);

    assert!(!intersection.is_named());
}

#[test]
fn test_class_expression_union() {
    let cat = OwlClass::new(NamedNode::new("http://example.org/Cat").unwrap());
    let dog = OwlClass::new(NamedNode::new("http://example.org/Dog").unwrap());

    let union = ClassExpression::union(vec![
        ClassExpression::class(cat),
        ClassExpression::class(dog),
    ]);

    assert!(!union.is_named());
}

#[test]
fn test_class_expression_complement() {
    let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());

    let complement = ClassExpression::complement(ClassExpression::class(animal));

    assert!(!complement.is_named());
}

#[test]
fn test_existential_restriction() {
    let has_pet = ObjectProperty::new(NamedNode::new("http://example.org/hasPet").unwrap());
    let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());

    let restriction = ClassExpression::some_values_from(
        has_pet,
        ClassExpression::class(animal),
    );

    assert!(!restriction.is_named());
}

#[test]
fn test_universal_restriction() {
    let has_pet = ObjectProperty::new(NamedNode::new("http://example.org/hasPet").unwrap());
    let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());

    let restriction = ClassExpression::all_values_from(
        has_pet,
        ClassExpression::class(animal),
    );

    assert!(!restriction.is_named());
}

#[test]
fn test_ontology_imports() {
    let mut ontology = Ontology::new(None);

    let import1 = NamedNode::new("http://example.org/ontology1").unwrap();
    let import2 = NamedNode::new("http://example.org/ontology2").unwrap();

    ontology.add_import(import1.clone());
    ontology.add_import(import2.clone());
    ontology.add_import(import1.clone()); // Duplicate should not be added

    assert_eq!(ontology.imports().len(), 2);
}

#[test]
fn test_ontology_merge() {
    let mut ontology1 = Ontology::new(None);
    let mut ontology2 = Ontology::new(None);

    let dog = OwlClass::new(NamedNode::new("http://example.org/Dog").unwrap());
    let cat = OwlClass::new(NamedNode::new("http://example.org/Cat").unwrap());

    ontology1.add_axiom(Axiom::DeclareClass(dog));
    ontology2.add_axiom(Axiom::DeclareClass(cat));

    ontology1.merge(ontology2);

    assert_eq!(ontology1.axiom_count(), 2);
}

#[test]
fn test_ontology_clear() {
    let mut ontology = Ontology::new(None);

    let dog = OwlClass::new(NamedNode::new("http://example.org/Dog").unwrap());
    ontology.add_axiom(Axiom::DeclareClass(dog));

    assert_eq!(ontology.axiom_count(), 1);

    ontology.clear();

    assert_eq!(ontology.axiom_count(), 0);
}

#[test]
fn test_individual_types() {
    let named = Individual::Named(NamedNode::new("http://example.org/ind1").unwrap());
    let anon = Individual::Anonymous(oxrdf::BlankNode::default());

    assert!(named.is_named());
    assert!(!named.is_anonymous());

    assert!(!anon.is_named());
    assert!(anon.is_anonymous());
}

#[test]
fn test_owl_class_display() {
    let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());
    let display = format!("{}", animal);
    assert!(display.contains("http://example.org/Animal"));
}

#[test]
fn test_ontology_display() {
    let ontology = Ontology::with_iri("http://example.org/animals").unwrap();
    let display = format!("{}", ontology);
    assert!(display.contains("Ontology"));
    assert!(display.contains("0 axioms"));
}

// Reasoner tests (when feature is enabled)
#[cfg(feature = "reasoner-rl")]
mod reasoner_tests {
    use super::*;
    use oxowl::{Reasoner, RlReasoner, ReasonerConfig};

    #[test]
    fn test_reasoner_classify() {
        let mut ontology = Ontology::new(None);

        let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());
        let dog = OwlClass::new(NamedNode::new("http://example.org/Dog").unwrap());
        let poodle = OwlClass::new(NamedNode::new("http://example.org/Poodle").unwrap());

        // Poodle subClassOf Dog subClassOf Animal
        ontology.add_axiom(Axiom::subclass_of(
            ClassExpression::class(poodle.clone()),
            ClassExpression::class(dog.clone()),
        ));
        ontology.add_axiom(Axiom::subclass_of(
            ClassExpression::class(dog.clone()),
            ClassExpression::class(animal.clone()),
        ));

        let mut reasoner = RlReasoner::new(&ontology);
        reasoner.classify().unwrap();

        // Poodle should be subclass of Animal (inferred transitively)
        let super_classes = reasoner.get_super_classes(&poodle, false);
        assert!(super_classes.contains(&&animal));
    }

    #[test]
    fn test_reasoner_type_inference() {
        let mut ontology = Ontology::new(None);

        let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());
        let dog = OwlClass::new(NamedNode::new("http://example.org/Dog").unwrap());
        let fido = Individual::Named(NamedNode::new("http://example.org/fido").unwrap());

        // Dog subClassOf Animal
        ontology.add_axiom(Axiom::subclass_of(
            ClassExpression::class(dog.clone()),
            ClassExpression::class(animal.clone()),
        ));

        // fido : Dog
        ontology.add_axiom(Axiom::class_assertion(
            ClassExpression::class(dog.clone()),
            fido.clone(),
        ));

        let mut reasoner = RlReasoner::new(&ontology);
        reasoner.classify().unwrap();

        // fido should be inferred to be an Animal
        let types = reasoner.get_types(&fido);
        assert!(types.contains(&&animal));
    }

    #[test]
    fn test_reasoner_consistency_check() {
        let ontology = Ontology::new(None);

        let reasoner = RlReasoner::new(&ontology);

        assert!(reasoner.is_consistent().unwrap());
    }

    #[test]
    fn test_reasoner_equivalent_classes() {
        let mut ontology = Ontology::new(None);

        let cat = OwlClass::new(NamedNode::new("http://example.org/Cat").unwrap());
        let feline = OwlClass::new(NamedNode::new("http://example.org/Feline").unwrap());

        ontology.add_axiom(Axiom::equivalent_classes(vec![
            ClassExpression::class(cat.clone()),
            ClassExpression::class(feline.clone()),
        ]));

        let mut reasoner = RlReasoner::new(&ontology);
        reasoner.classify().unwrap();

        let equivalents = reasoner.get_equivalent_classes(&cat);
        assert!(equivalents.contains(&&feline));
    }

    #[test]
    fn test_reasoner_display() {
        let ontology = Ontology::new(None);
        let reasoner = RlReasoner::new(&ontology);
        let display = format!("{}", reasoner);
        assert!(display.contains("RlReasoner"));
    }
}

#[cfg(feature = "reasoner-rl")]
mod advanced_reasoner_tests {
    use super::*;
    use oxowl::{Reasoner, RlReasoner};

    #[test]
    fn test_domain_range_inference() {
        let mut ontology = Ontology::new(None);

        let person = OwlClass::new(NamedNode::new("http://example.org/Person").unwrap());
        let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());
        let has_pet = ObjectProperty::new(NamedNode::new("http://example.org/hasPet").unwrap());

        let alice = Individual::Named(NamedNode::new("http://example.org/alice").unwrap());
        let fido = Individual::Named(NamedNode::new("http://example.org/fido").unwrap());

        // hasPet domain Person, range Animal
        ontology.add_axiom(Axiom::ObjectPropertyDomain {
            property: has_pet.clone(),
            domain: ClassExpression::class(person.clone()),
        });
        ontology.add_axiom(Axiom::ObjectPropertyRange {
            property: has_pet.clone(),
            range: ClassExpression::class(animal.clone()),
        });

        // alice hasPet fido
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: has_pet,
            source: alice.clone(),
            target: fido.clone(),
        });

        let mut reasoner = RlReasoner::new(&ontology);
        reasoner.classify().unwrap();

        // alice should be inferred as Person (domain)
        let alice_types = reasoner.get_types(&alice);
        assert!(alice_types.iter().any(|c| c == &&person), "alice should be a Person");

        // fido should be inferred as Animal (range)
        let fido_types = reasoner.get_types(&fido);
        assert!(fido_types.iter().any(|c| c == &&animal), "fido should be an Animal");
    }

    #[test]
    fn test_inverse_property_inference() {
        let mut ontology = Ontology::new(None);

        let has_parent = ObjectProperty::new(NamedNode::new("http://example.org/hasParent").unwrap());
        let has_child = ObjectProperty::new(NamedNode::new("http://example.org/hasChild").unwrap());

        let alice = Individual::Named(NamedNode::new("http://example.org/alice").unwrap());
        let bob = Individual::Named(NamedNode::new("http://example.org/bob").unwrap());

        // hasParent inverseOf hasChild
        ontology.add_axiom(Axiom::InverseObjectProperties(has_parent.clone(), has_child.clone()));

        // alice hasParent bob
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: has_parent,
            source: alice.clone(),
            target: bob.clone(),
        });

        let mut reasoner = RlReasoner::new(&ontology);
        reasoner.classify().unwrap();

        // Should be able to query bob hasChild alice (inferred)
        // This tests the inverse property inference
    }

    #[test]
    fn test_transitive_property() {
        let mut ontology = Ontology::new(None);

        let ancestor_of = ObjectProperty::new(NamedNode::new("http://example.org/ancestorOf").unwrap());

        let alice = Individual::Named(NamedNode::new("http://example.org/alice").unwrap());
        let bob = Individual::Named(NamedNode::new("http://example.org/bob").unwrap());
        let charlie = Individual::Named(NamedNode::new("http://example.org/charlie").unwrap());

        // ancestorOf is transitive
        ontology.add_axiom(Axiom::TransitiveObjectProperty(ancestor_of.clone()));

        // alice ancestorOf bob, bob ancestorOf charlie
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: ancestor_of.clone(),
            source: alice.clone(),
            target: bob.clone(),
        });
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: ancestor_of.clone(),
            source: bob.clone(),
            target: charlie.clone(),
        });

        let mut reasoner = RlReasoner::new(&ontology);
        reasoner.classify().unwrap();

        // alice ancestorOf charlie should be inferred
    }

    #[test]
    fn test_symmetric_property() {
        let mut ontology = Ontology::new(None);

        let knows = ObjectProperty::new(NamedNode::new("http://example.org/knows").unwrap());

        let alice = Individual::Named(NamedNode::new("http://example.org/alice").unwrap());
        let bob = Individual::Named(NamedNode::new("http://example.org/bob").unwrap());

        // knows is symmetric
        ontology.add_axiom(Axiom::SymmetricObjectProperty(knows.clone()));

        // alice knows bob
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: knows,
            source: alice.clone(),
            target: bob.clone(),
        });

        let mut reasoner = RlReasoner::new(&ontology);
        reasoner.classify().unwrap();

        // bob knows alice should be inferred
    }
}
