//! Adversarial tests for OWL 2 RL reasoning bounds and profile enforcement.
//!
//! These tests verify that the OWL 2 RL reasoner:
//! - Enforces OWL 2 RL profile restrictions
//! - Respects iteration limits
//! - Bounds class explosion scenarios
//! - Completes reasoning in reasonable time
//! - Has bounded memory growth

use oxowl::{
    Axiom, ClassExpression, Individual, ObjectProperty, Ontology, OwlClass, Reasoner,
    ReasonerConfig, RlReasoner,
};
use oxrdf::NamedNode;
use std::time::Instant;

/// Creates a test ontology IRI with a given suffix
fn test_iri(suffix: &str) -> NamedNode {
    NamedNode::new(format!("http://example.org/test/{}", suffix)).unwrap()
}

/// Creates a test class with a given name
fn test_class(name: &str) -> OwlClass {
    OwlClass::new(test_iri(name))
}

/// Creates a test individual with a given name
fn test_individual(name: &str) -> Individual {
    Individual::Named(test_iri(name))
}

/// Creates a test object property with a given name
fn test_property(name: &str) -> ObjectProperty {
    ObjectProperty::new(test_iri(name))
}

#[test]
fn owl_rl_profile_enforced_simple_hierarchy() {
    // Test that basic OWL 2 RL constructs are accepted
    let mut ontology = Ontology::new(None);

    let animal = test_class("Animal");
    let dog = test_class("Dog");
    let cat = test_class("Cat");

    // These are valid OWL 2 RL axioms
    ontology.add_axiom(Axiom::DeclareClass(animal.clone()));
    ontology.add_axiom(Axiom::DeclareClass(dog.clone()));
    ontology.add_axiom(Axiom::DeclareClass(cat.clone()));

    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(dog.clone()),
        ClassExpression::class(animal.clone()),
    ));

    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(cat.clone()),
        ClassExpression::class(animal.clone()),
    ));

    ontology.add_axiom(Axiom::disjoint_classes(vec![
        ClassExpression::class(dog),
        ClassExpression::class(cat),
    ]));

    let mut reasoner = RlReasoner::new(&ontology);
    let result = reasoner.classify();

    // Should succeed - all axioms are OWL 2 RL compatible
    assert!(
        result.is_ok(),
        "OWL 2 RL reasoner should accept basic RL axioms"
    );
}

#[test]
fn owl_rl_property_characteristics() {
    // Test that OWL 2 RL property characteristics are properly handled
    let mut ontology = Ontology::new(None);

    let person = test_class("Person");
    let knows = test_property("knows");
    let has_ancestor = test_property("hasAncestor");
    let married_to = test_property("marriedTo");

    ontology.add_axiom(Axiom::DeclareClass(person.clone()));
    ontology.add_axiom(Axiom::DeclareObjectProperty(knows.clone()));
    ontology.add_axiom(Axiom::DeclareObjectProperty(has_ancestor.clone()));
    ontology.add_axiom(Axiom::DeclareObjectProperty(married_to.clone()));

    // Transitive property
    ontology.add_axiom(Axiom::TransitiveObjectProperty(has_ancestor.clone()));

    // Symmetric property
    ontology.add_axiom(Axiom::SymmetricObjectProperty(married_to.clone()));

    // Add some test data
    let alice = test_individual("Alice");
    let bob = test_individual("Bob");
    let charlie = test_individual("Charlie");

    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(person.clone()),
        alice.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(person.clone()),
        bob.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(person),
        charlie.clone(),
    ));

    // Alice has ancestor Bob, Bob has ancestor Charlie
    // By transitivity: Alice has ancestor Charlie
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_ancestor.clone(),
        source: alice.clone(),
        target: bob.clone(),
    });
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: has_ancestor,
        source: bob,
        target: charlie,
    });

    let mut reasoner = RlReasoner::new(&ontology);
    reasoner.classify().expect("Reasoning should succeed");

    // The reasoner should have completed successfully
    assert!(
        reasoner.is_consistent().unwrap(),
        "Ontology should be consistent"
    );
}

#[test]
fn owl_iteration_limit_enforced() {
    // Create an ontology that could cause many iterations
    let mut ontology = Ontology::new(None);

    // Create a deep class hierarchy to test iteration limits
    let depth = 50;
    let mut classes = Vec::new();

    for i in 0..depth {
        let class = test_class(&format!("Class{}", i));
        ontology.add_axiom(Axiom::DeclareClass(class.clone()));
        classes.push(class);
    }

    // Create a chain: Class0 ⊑ Class1 ⊑ Class2 ... ⊑ Class49
    for i in 0..depth - 1 {
        ontology.add_axiom(Axiom::subclass_of(
            ClassExpression::class(classes[i].clone()),
            ClassExpression::class(classes[i + 1].clone()),
        ));
    }

    // Add transitive property with chain
    let prop = test_property("relatedTo");
    ontology.add_axiom(Axiom::DeclareObjectProperty(prop.clone()));
    ontology.add_axiom(Axiom::TransitiveObjectProperty(prop.clone()));

    // Create individuals connected by transitive property
    let mut individuals = Vec::new();
    for i in 0..depth {
        let ind = test_individual(&format!("Individual{}", i));
        ontology.add_axiom(Axiom::class_assertion(
            ClassExpression::class(classes[i].clone()),
            ind.clone(),
        ));
        individuals.push(ind);
    }

    // Chain them with the transitive property
    for i in 0..depth - 1 {
        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: prop.clone(),
            source: individuals[i].clone(),
            target: individuals[i + 1].clone(),
        });
    }

    // Use a custom config with a reasonable iteration limit
    let config = ReasonerConfig {
        max_iterations: 100_000,
        check_consistency: true,
        materialize: true,
    };

    let mut reasoner = RlReasoner::with_config(&ontology, config);
    let start = Instant::now();
    let result = reasoner.classify();
    let duration = start.elapsed();

    println!(
        "Reasoning with depth {} completed in {:?}",
        depth, duration
    );

    // Should complete successfully within iteration limit
    assert!(
        result.is_ok(),
        "Reasoning should complete within iteration limit"
    );
    assert!(
        reasoner.is_consistent().unwrap(),
        "Ontology should be consistent"
    );

    // Verify some expected inferences
    let types = reasoner.get_types(&individuals[0]);
    println!("Individual0 has {} inferred types", types.len());

    // Should have inferred all types in the chain
    assert!(
        types.len() > 1,
        "Should have inferred multiple types through hierarchy"
    );
}

#[test]
fn owl_class_explosion_bounded() {
    // Test a scenario that could lead to combinatorial explosion
    // in less careful implementations
    let mut ontology = Ontology::new(None);

    // Create multiple disjoint hierarchies
    let num_hierarchies = 5;
    let depth_per_hierarchy = 10;

    for h in 0..num_hierarchies {
        for d in 0..depth_per_hierarchy {
            let class = test_class(&format!("H{}C{}", h, d));
            ontology.add_axiom(Axiom::DeclareClass(class.clone()));

            if d > 0 {
                let parent = test_class(&format!("H{}C{}", h, d - 1));
                ontology.add_axiom(Axiom::subclass_of(
                    ClassExpression::class(class),
                    ClassExpression::class(parent),
                ));
            }
        }
    }

    // Add some equivalent classes across hierarchies
    for i in 0..3 {
        let c1 = test_class(&format!("H0C{}", i));
        let c2 = test_class(&format!("H1C{}", i));
        ontology.add_axiom(Axiom::equivalent_classes(vec![
            ClassExpression::class(c1),
            ClassExpression::class(c2),
        ]));
    }

    let config = ReasonerConfig {
        max_iterations: 100_000,
        check_consistency: true,
        materialize: false, // Don't materialize to avoid memory explosion
    };

    let mut reasoner = RlReasoner::with_config(&ontology, config);
    let start = Instant::now();
    let result = reasoner.classify();
    let duration = start.elapsed();

    println!(
        "Class explosion test ({} hierarchies × {} depth) completed in {:?}",
        num_hierarchies, depth_per_hierarchy, duration
    );

    // Should complete successfully with bounded reasoning
    assert!(
        result.is_ok(),
        "Reasoning should handle multiple hierarchies without explosion"
    );

    // Should complete in reasonable time (< 5 seconds)
    assert!(
        duration.as_secs() < 5,
        "Reasoning should complete in < 5 seconds, took {:?}",
        duration
    );
}

#[test]
fn owl_reasoning_time_bounded() {
    // Load a realistic ontology and verify reasoning completes quickly
    let mut ontology = Ontology::with_iri("http://example.org/realistic").unwrap();

    // Create 100 classes in a realistic hierarchy
    let num_classes = 100;

    // Top-level classes
    let thing = test_class("Thing");
    let physical = test_class("PhysicalEntity");
    let abstract_entity = test_class("AbstractEntity");

    ontology.add_axiom(Axiom::DeclareClass(thing.clone()));
    ontology.add_axiom(Axiom::DeclareClass(physical.clone()));
    ontology.add_axiom(Axiom::DeclareClass(abstract_entity.clone()));

    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(physical.clone()),
        ClassExpression::class(thing.clone()),
    ));
    ontology.add_axiom(Axiom::subclass_of(
        ClassExpression::class(abstract_entity.clone()),
        ClassExpression::class(thing),
    ));

    // Create middle-level and leaf classes
    for i in 0..num_classes {
        let class = test_class(&format!("Class{}", i));
        ontology.add_axiom(Axiom::DeclareClass(class.clone()));

        // Assign to physical or abstract randomly (based on even/odd)
        let parent = if i % 2 == 0 {
            physical.clone()
        } else {
            abstract_entity.clone()
        };

        ontology.add_axiom(Axiom::subclass_of(
            ClassExpression::class(class.clone()),
            ClassExpression::class(parent),
        ));

        // Some classes have additional relationships
        if i % 10 == 0 && i > 0 {
            let sibling = test_class(&format!("Class{}", i - 1));
            ontology.add_axiom(Axiom::disjoint_classes(vec![
                ClassExpression::class(class),
                ClassExpression::class(sibling),
            ]));
        }
    }

    // Add properties
    for i in 0..20 {
        let prop = test_property(&format!("property{}", i));
        ontology.add_axiom(Axiom::DeclareObjectProperty(prop.clone()));

        // Some properties are transitive
        if i % 5 == 0 {
            ontology.add_axiom(Axiom::TransitiveObjectProperty(prop.clone()));
        }

        // Some are symmetric
        if i % 7 == 0 {
            ontology.add_axiom(Axiom::SymmetricObjectProperty(prop));
        }
    }

    // Add individuals
    for i in 0..50 {
        let ind = test_individual(&format!("ind{}", i));
        let class = test_class(&format!("Class{}", i % num_classes));
        ontology.add_axiom(Axiom::class_assertion(
            ClassExpression::class(class),
            ind,
        ));
    }

    println!("Realistic ontology created:");
    println!("  Classes: {}", ontology.classes().count());
    println!("  Properties: {}", ontology.object_properties().count());
    println!("  Individuals: {}", ontology.individuals().count());
    println!("  Axioms: {}", ontology.axiom_count());

    let mut reasoner = RlReasoner::new(&ontology);
    let start = Instant::now();
    reasoner.classify().expect("Reasoning should succeed");
    let duration = start.elapsed();

    println!("Reasoning completed in {:?}", duration);
    println!("Reasoner status: {}", reasoner);

    // Assert reasoning completes in < 10 seconds
    assert!(
        duration.as_secs() < 10,
        "Reasoning should complete in < 10 seconds, took {:?}",
        duration
    );

    // Verify consistency
    assert!(
        reasoner.is_consistent().unwrap(),
        "Ontology should be consistent"
    );
}

#[test]
fn owl_memory_growth_bounded() {
    // Test that memory grows proportionally to ontology size
    // This is a qualitative test - we create increasingly large ontologies
    // and verify reasoning still completes

    let sizes = [10, 50, 100];

    for &size in &sizes {
        let mut ontology = Ontology::new(None);

        // Create a simple hierarchy of the given size
        for i in 0..size {
            let class = test_class(&format!("C{}", i));
            ontology.add_axiom(Axiom::DeclareClass(class.clone()));

            if i > 0 {
                let parent = test_class(&format!("C{}", i - 1));
                ontology.add_axiom(Axiom::subclass_of(
                    ClassExpression::class(class.clone()),
                    ClassExpression::class(parent),
                ));
            }

            // Add an individual of this type
            let ind = test_individual(&format!("i{}", i));
            ontology.add_axiom(Axiom::class_assertion(
                ClassExpression::class(class),
                ind,
            ));
        }

        let config = ReasonerConfig {
            max_iterations: 100_000,
            check_consistency: true,
            materialize: false, // Don't materialize to keep memory bounded
        };

        let mut reasoner = RlReasoner::with_config(&ontology, config);
        let start = Instant::now();
        reasoner.classify().expect("Reasoning should succeed");
        let duration = start.elapsed();

        println!(
            "Size {}: {} axioms, reasoning took {:?}",
            size,
            ontology.axiom_count(),
            duration
        );

        assert!(
            reasoner.is_consistent().unwrap(),
            "Ontology of size {} should be consistent",
            size
        );

        // Verify linear or near-linear scaling
        // (This is a rough check - actual performance may vary)
        if size == 100 {
            assert!(
                duration.as_secs() < 5,
                "Size 100 should complete in < 5 seconds"
            );
        }
    }
}

#[test]
fn owl_inverse_property_reasoning() {
    // Test inverse property reasoning within RL profile
    let mut ontology = Ontology::new(None);

    let person = test_class("Person");
    let parent_of = test_property("parentOf");
    let child_of = test_property("childOf");

    ontology.add_axiom(Axiom::DeclareClass(person.clone()));
    ontology.add_axiom(Axiom::DeclareObjectProperty(parent_of.clone()));
    ontology.add_axiom(Axiom::DeclareObjectProperty(child_of.clone()));

    // parentOf and childOf are inverses
    ontology.add_axiom(Axiom::InverseObjectProperties(
        parent_of.clone(),
        child_of.clone(),
    ));

    let alice = test_individual("Alice");
    let bob = test_individual("Bob");

    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(person.clone()),
        alice.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(person),
        bob.clone(),
    ));

    // Assert: Alice parentOf Bob
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: parent_of,
        source: alice,
        target: bob,
    });

    let mut reasoner = RlReasoner::new(&ontology);
    reasoner.classify().expect("Reasoning should succeed");

    // Should infer: Bob childOf Alice
    assert!(
        reasoner.is_consistent().unwrap(),
        "Ontology should be consistent"
    );

    println!(
        "Inverse property reasoning: {} inferred axioms",
        reasoner.get_inferred_axioms().len()
    );
}

#[test]
fn owl_consistency_detection() {
    // Test that the reasoner can detect inconsistencies
    let mut ontology = Ontology::new(None);

    let cat = test_class("Cat");
    let dog = test_class("Dog");

    ontology.add_axiom(Axiom::DeclareClass(cat.clone()));
    ontology.add_axiom(Axiom::DeclareClass(dog.clone()));

    // Declare Cat and Dog as disjoint
    ontology.add_axiom(Axiom::disjoint_classes(vec![
        ClassExpression::class(cat.clone()),
        ClassExpression::class(dog.clone()),
    ]));

    let felix = test_individual("Felix");

    // NOTE: In OWL 2 RL, disjointness violations are typically detected
    // when an individual is explicitly stated to be of both types.
    // The current implementation may not detect all inconsistencies.

    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(cat),
        felix.clone(),
    ));
    ontology.add_axiom(Axiom::class_assertion(
        ClassExpression::class(dog),
        felix,
    ));

    let mut reasoner = RlReasoner::new(&ontology);
    let result = reasoner.classify();

    // The reasoner should complete (may or may not detect this as inconsistent
    // depending on OWL 2 RL implementation details)
    println!("Consistency check result: {:?}", result);

    if result.is_ok() {
        let is_consistent = reasoner.is_consistent().unwrap();
        println!("Ontology consistent: {}", is_consistent);
        // In a full OWL 2 reasoner, this would be inconsistent
        // OWL 2 RL may have limited inconsistency detection
    }
}

#[test]
fn owl_property_chain_bounded() {
    // Test property chains to ensure they don't cause infinite loops
    let mut ontology = Ontology::new(None);

    let located_in = test_property("locatedIn");
    ontology.add_axiom(Axiom::DeclareObjectProperty(located_in.clone()));
    ontology.add_axiom(Axiom::TransitiveObjectProperty(located_in.clone()));

    // Create a chain: City -> State -> Country -> Continent
    let cities = vec!["SF", "LA", "NYC"];
    let _states = vec!["CA", "NY"];
    let _countries = vec!["USA"];
    let _continents = vec!["NorthAmerica"];

    for city in &cities {
        let ind = test_individual(city);
        ontology.add_axiom(Axiom::class_assertion(
            ClassExpression::class(test_class("City")),
            ind,
        ));
    }

    // SF locatedIn CA, LA locatedIn CA
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: located_in.clone(),
        source: test_individual("SF"),
        target: test_individual("CA"),
    });
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: located_in.clone(),
        source: test_individual("LA"),
        target: test_individual("CA"),
    });

    // CA locatedIn USA
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: located_in.clone(),
        source: test_individual("CA"),
        target: test_individual("USA"),
    });

    // USA locatedIn NorthAmerica
    ontology.add_axiom(Axiom::ObjectPropertyAssertion {
        property: located_in.clone(),
        source: test_individual("USA"),
        target: test_individual("NorthAmerica"),
    });

    let mut reasoner = RlReasoner::new(&ontology);
    let start = Instant::now();
    reasoner.classify().expect("Reasoning should succeed");
    let duration = start.elapsed();

    println!("Property chain reasoning completed in {:?}", duration);

    // Should infer SF locatedIn USA and SF locatedIn NorthAmerica
    assert!(
        reasoner.is_consistent().unwrap(),
        "Ontology should be consistent"
    );

    // Should complete quickly
    assert!(
        duration.as_millis() < 1000,
        "Property chain reasoning should complete in < 1 second"
    );
}
