//! OWL 2 serializer to RDF graphs.
//!
//! This module provides functionality to serialize OWL 2 ontologies to RDF
//! representations that can be written as Turtle, RDF/XML, N-Triples, etc.

use crate::axiom::Axiom;
use crate::entity::Individual;
use crate::expression::{ClassExpression, ObjectPropertyExpression, DataRange};
use crate::ontology::Ontology;
use oxrdf::{
    BlankNode, Graph, Literal, NamedNode, NamedNodeRef, NamedOrBlankNode, Term, Triple,
    vocab::{rdf, rdfs, owl, xsd},
};
use std::collections::HashMap;

/// OWL 2 namespace constant.
const OWL: &str = "http://www.w3.org/2002/07/owl#";

/// Helper to create OWL vocabulary named nodes.
fn owl_term(local: &str) -> NamedNode {
    NamedNode::new_unchecked(format!("{}{}", OWL, local))
}

/// Helper to create OWL vocabulary named node refs.
fn owl_term_ref(local: &str) -> NamedNodeRef<'static> {
    match local {
        "NegativePropertyAssertion" => NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#NegativePropertyAssertion"),
        "sourceIndividual" => NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#sourceIndividual"),
        "assertionProperty" => NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#assertionProperty"),
        "targetIndividual" => NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#targetIndividual"),
        "targetValue" => NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#targetValue"),
        "onDatatype" => NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#onDatatype"),
        "withRestrictions" => NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#withRestrictions"),
        _ => panic!("Unknown OWL term: {}", local),
    }
}

/// Serializer configuration.
#[derive(Debug, Clone, Default)]
pub struct SerializerConfig {
    /// Whether to include declaration axioms explicitly.
    pub include_declarations: bool,
    /// Whether to use compact notation for restrictions.
    pub compact_restrictions: bool,
}

impl SerializerConfig {
    /// Creates a new serializer configuration with default values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to include explicit declaration axioms.
    #[must_use]
    pub fn include_declarations(mut self, value: bool) -> Self {
        self.include_declarations = value;
        self
    }

    /// Sets whether to use compact notation for restrictions.
    #[must_use]
    pub fn compact_restrictions(mut self, value: bool) -> Self {
        self.compact_restrictions = value;
        self
    }
}

/// Serializes an OWL ontology to RDF triples.
pub struct OntologySerializer {
    config: SerializerConfig,
    blank_node_counter: u64,
    blank_node_cache: HashMap<String, BlankNode>,
}

impl OntologySerializer {
    /// Creates a new serializer with default configuration.
    pub fn new() -> Self {
        Self::with_config(SerializerConfig::new())
    }

    /// Creates a new serializer with custom configuration.
    pub fn with_config(config: SerializerConfig) -> Self {
        Self {
            config,
            blank_node_counter: 0,
            blank_node_cache: HashMap::new(),
        }
    }

    /// Serializes an ontology to an RDF graph.
    pub fn serialize(&mut self, ontology: &Ontology) -> Graph {
        let mut graph = Graph::new();

        // Serialize ontology header
        if let Some(iri) = ontology.iri() {
            graph.insert(&Triple {
                subject: iri.as_ref().into(),
                predicate: rdf::TYPE.into(),
                object: owl::ONTOLOGY.into(),
            });

            // Add version IRI if present
            if let Some(version_iri) = ontology.version_iri() {
                graph.insert(&Triple {
                    subject: iri.as_ref().into(),
                    predicate: owl::VERSION_IRI.into(),
                    object: version_iri.as_ref().into(),
                });
            }

            // Add imports
            for import in ontology.imports() {
                graph.insert(&Triple {
                    subject: iri.as_ref().into(),
                    predicate: owl::IMPORTS.into(),
                    object: import.as_ref().into(),
                });
            }
        }

        // Serialize axioms
        for axiom in ontology.axioms() {
            self.serialize_axiom(axiom, &mut graph);
        }

        graph
    }

    /// Serializes a single axiom to the graph.
    fn serialize_axiom(&mut self, axiom: &Axiom, graph: &mut Graph) {
        match axiom {
            // === Class Axioms ===
            Axiom::SubClassOf { sub_class, super_class } => {
                let sub_term = self.serialize_class_expression(sub_class, graph);
                let super_term = self.serialize_class_expression(super_class, graph);
                graph.insert(&Triple {
                    subject: self.term_to_subject(sub_term),
                    predicate: rdfs::SUB_CLASS_OF.into(),
                    object: super_term.into(),
                });
            }

            Axiom::EquivalentClasses(classes) => {
                if classes.len() >= 2 {
                    // Create pairwise equivalences
                    for i in 0..classes.len() - 1 {
                        let class1 = self.serialize_class_expression(&classes[i], graph);
                        let class2 = self.serialize_class_expression(&classes[i + 1], graph);
                        graph.insert(&Triple {
                            subject: self.term_to_subject(class1),
                            predicate: owl::EQUIVALENT_CLASS.into(),
                            object: class2.into(),
                        });
                    }
                }
            }

            Axiom::DisjointClasses(classes) => {
                if classes.len() >= 2 {
                    // Create pairwise disjointness
                    for i in 0..classes.len() - 1 {
                        let class1 = self.serialize_class_expression(&classes[i], graph);
                        let class2 = self.serialize_class_expression(&classes[i + 1], graph);
                        graph.insert(&Triple {
                            subject: self.term_to_subject(class1),
                            predicate: owl::DISJOINT_WITH.into(),
                            object: class2.into(),
                        });
                    }
                }
            }

            Axiom::DisjointUnion { class, disjoint_classes } => {
                let class_iri = class.iri().as_ref();
                let class_terms: Vec<Term> = disjoint_classes
                    .iter()
                    .map(|c| self.serialize_class_expression(c, graph))
                    .collect();
                let union_list = self.create_rdf_list(class_terms, graph);
                graph.insert(&Triple {
                    subject: class_iri.into(),
                    predicate: owl::DISJOINT_UNION_OF.into(),
                    object: union_list.into(),
                });
            }

            // === Object Property Axioms ===
            Axiom::SubObjectPropertyOf { sub_property, super_property } => {
                let sub_term = self.serialize_object_property_expression(sub_property);
                let super_term = self.serialize_object_property_expression(super_property);
                graph.insert(&Triple {
                    subject: self.term_to_subject(sub_term),
                    predicate: rdfs::SUB_PROPERTY_OF.into(),
                    object: super_term.into(),
                });
            }

            Axiom::EquivalentObjectProperties(props) => {
                if props.len() >= 2 {
                    for i in 0..props.len() - 1 {
                        graph.insert(&Triple {
                            subject: props[i].iri().as_ref().into(),
                            predicate: owl::EQUIVALENT_PROPERTY.into(),
                            object: props[i + 1].iri().as_ref().into(),
                        });
                    }
                }
            }

            Axiom::DisjointObjectProperties(props) => {
                if props.len() >= 2 {
                    for i in 0..props.len() - 1 {
                        graph.insert(&Triple {
                            subject: props[i].iri().as_ref().into(),
                            predicate: owl::PROPERTY_DISJOINT_WITH.into(),
                            object: props[i + 1].iri().as_ref().into(),
                        });
                    }
                }
            }

            Axiom::ObjectPropertyDomain { property, domain } => {
                let domain_term = self.serialize_class_expression(domain, graph);
                graph.insert(&Triple {
                    subject: property.iri().as_ref().into(),
                    predicate: rdfs::DOMAIN.into(),
                    object: domain_term.into(),
                });
            }

            Axiom::ObjectPropertyRange { property, range } => {
                let range_term = self.serialize_class_expression(range, graph);
                graph.insert(&Triple {
                    subject: property.iri().as_ref().into(),
                    predicate: rdfs::RANGE.into(),
                    object: range_term.into(),
                });
            }

            Axiom::InverseObjectProperties(prop1, prop2) => {
                graph.insert(&Triple {
                    subject: prop1.iri().as_ref().into(),
                    predicate: owl::INVERSE_OF.into(),
                    object: prop2.iri().as_ref().into(),
                });
            }

            Axiom::FunctionalObjectProperty(prop) => {
                graph.insert(&Triple {
                    subject: prop.iri().as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::FUNCTIONAL_PROPERTY.into(),
                });
            }

            Axiom::InverseFunctionalObjectProperty(prop) => {
                graph.insert(&Triple {
                    subject: prop.iri().as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::INVERSE_FUNCTIONAL_PROPERTY.into(),
                });
            }

            Axiom::ReflexiveObjectProperty(prop) => {
                graph.insert(&Triple {
                    subject: prop.iri().as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::REFLEXIVE_PROPERTY.into(),
                });
            }

            Axiom::IrreflexiveObjectProperty(prop) => {
                graph.insert(&Triple {
                    subject: prop.iri().as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::IRREFLEXIVE_PROPERTY.into(),
                });
            }

            Axiom::SymmetricObjectProperty(prop) => {
                graph.insert(&Triple {
                    subject: prop.iri().as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::SYMMETRIC_PROPERTY.into(),
                });
            }

            Axiom::AsymmetricObjectProperty(prop) => {
                graph.insert(&Triple {
                    subject: prop.iri().as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::ASYMMETRIC_PROPERTY.into(),
                });
            }

            Axiom::TransitiveObjectProperty(prop) => {
                graph.insert(&Triple {
                    subject: prop.iri().as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::TRANSITIVE_PROPERTY.into(),
                });
            }

            // === Data Property Axioms ===
            Axiom::SubDataPropertyOf { sub_property, super_property } => {
                graph.insert(&Triple {
                    subject: sub_property.iri().as_ref().into(),
                    predicate: rdfs::SUB_PROPERTY_OF.into(),
                    object: super_property.iri().as_ref().into(),
                });
            }

            Axiom::EquivalentDataProperties(props) => {
                if props.len() >= 2 {
                    for i in 0..props.len() - 1 {
                        graph.insert(&Triple {
                            subject: props[i].iri().as_ref().into(),
                            predicate: owl::EQUIVALENT_PROPERTY.into(),
                            object: props[i + 1].iri().as_ref().into(),
                        });
                    }
                }
            }

            Axiom::DisjointDataProperties(props) => {
                if props.len() >= 2 {
                    for i in 0..props.len() - 1 {
                        graph.insert(&Triple {
                            subject: props[i].iri().as_ref().into(),
                            predicate: owl::PROPERTY_DISJOINT_WITH.into(),
                            object: props[i + 1].iri().as_ref().into(),
                        });
                    }
                }
            }

            Axiom::DataPropertyDomain { property, domain } => {
                let domain_term = self.serialize_class_expression(domain, graph);
                graph.insert(&Triple {
                    subject: property.iri().as_ref().into(),
                    predicate: rdfs::DOMAIN.into(),
                    object: domain_term.into(),
                });
            }

            Axiom::DataPropertyRange { property, range } => {
                let range_term = self.serialize_data_range(range, graph);
                graph.insert(&Triple {
                    subject: property.iri().as_ref().into(),
                    predicate: rdfs::RANGE.into(),
                    object: range_term.into(),
                });
            }

            Axiom::FunctionalDataProperty(prop) => {
                graph.insert(&Triple {
                    subject: prop.iri().as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::FUNCTIONAL_PROPERTY.into(),
                });
            }

            // === Individual Axioms (Assertions) ===
            Axiom::ClassAssertion { class, individual } => {
                let class_term = self.serialize_class_expression(class, graph);
                let ind_subject = self.individual_to_subject(individual);
                graph.insert(&Triple {
                    subject: ind_subject,
                    predicate: rdf::TYPE.into(),
                    object: class_term.into(),
                });
            }

            Axiom::ObjectPropertyAssertion { property, source, target } => {
                graph.insert(&Triple {
                    subject: self.individual_to_subject(source),
                    predicate: property.iri().clone(),
                    object: self.individual_to_term(target).into(),
                });
            }

            Axiom::NegativeObjectPropertyAssertion { property, source, target } => {
                // Use blank node for negative property assertion
                let assertion = self.fresh_blank_node();
                graph.insert(&Triple {
                    subject: assertion.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl_term_ref("NegativePropertyAssertion").into(),
                });
                graph.insert(&Triple {
                    subject: assertion.as_ref().into(),
                    predicate: owl_term_ref("sourceIndividual").into(),
                    object: self.individual_to_term(source).into(),
                });
                graph.insert(&Triple {
                    subject: assertion.as_ref().into(),
                    predicate: owl_term_ref("assertionProperty").into(),
                    object: property.iri().as_ref().into(),
                });
                graph.insert(&Triple {
                    subject: assertion.as_ref().into(),
                    predicate: owl_term_ref("targetIndividual").into(),
                    object: self.individual_to_term(target).into(),
                });
            }

            Axiom::DataPropertyAssertion { property, source, target } => {
                graph.insert(&Triple {
                    subject: self.individual_to_subject(source),
                    predicate: property.iri().clone(),
                    object: target.as_ref().into(),
                });
            }

            Axiom::NegativeDataPropertyAssertion { property, source, target } => {
                let assertion = self.fresh_blank_node();
                graph.insert(&Triple {
                    subject: assertion.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl_term_ref("NegativePropertyAssertion").into(),
                });
                graph.insert(&Triple {
                    subject: assertion.as_ref().into(),
                    predicate: owl_term_ref("sourceIndividual").into(),
                    object: self.individual_to_term(source).into(),
                });
                graph.insert(&Triple {
                    subject: assertion.as_ref().into(),
                    predicate: owl_term_ref("assertionProperty").into(),
                    object: property.iri().as_ref().into(),
                });
                graph.insert(&Triple {
                    subject: assertion.as_ref().into(),
                    predicate: owl_term_ref("targetValue").into(),
                    object: target.as_ref().into(),
                });
            }

            Axiom::SameIndividual(individuals) => {
                if individuals.len() >= 2 {
                    for i in 0..individuals.len() - 1 {
                        graph.insert(&Triple {
                            subject: self.individual_to_subject(&individuals[i]),
                            predicate: owl::SAME_AS.into(),
                            object: self.individual_to_term(&individuals[i + 1]).into(),
                        });
                    }
                }
            }

            Axiom::DifferentIndividuals(individuals) => {
                if individuals.len() >= 2 {
                    for i in 0..individuals.len() - 1 {
                        graph.insert(&Triple {
                            subject: self.individual_to_subject(&individuals[i]),
                            predicate: owl::DIFFERENT_FROM.into(),
                            object: self.individual_to_term(&individuals[i + 1]).into(),
                        });
                    }
                }
            }

            // === Keys ===
            Axiom::HasKey { class, object_properties, data_properties } => {
                let class_term = self.serialize_class_expression(class, graph);
                let mut key_props = Vec::new();
                for prop in object_properties {
                    key_props.push(Term::NamedNode(prop.iri().clone()));
                }
                for prop in data_properties {
                    key_props.push(Term::NamedNode(prop.iri().clone()));
                }
                let key_list = self.create_rdf_list(key_props, graph);
                graph.insert(&Triple {
                    subject: self.term_to_subject(class_term),
                    predicate: owl::HAS_KEY.into(),
                    object: key_list.into(),
                });
            }

            // === Declaration Axioms ===
            Axiom::DeclareClass(class) => {
                if self.config.include_declarations {
                    graph.insert(&Triple {
                        subject: class.iri().as_ref().into(),
                        predicate: rdf::TYPE.into(),
                        object: owl::CLASS.into(),
                    });
                }
            }

            Axiom::DeclareObjectProperty(prop) => {
                if self.config.include_declarations {
                    graph.insert(&Triple {
                        subject: prop.iri().as_ref().into(),
                        predicate: rdf::TYPE.into(),
                        object: owl::OBJECT_PROPERTY.into(),
                    });
                }
            }

            Axiom::DeclareDataProperty(prop) => {
                if self.config.include_declarations {
                    graph.insert(&Triple {
                        subject: prop.iri().as_ref().into(),
                        predicate: rdf::TYPE.into(),
                        object: owl::DATATYPE_PROPERTY.into(),
                    });
                }
            }

            Axiom::DeclareAnnotationProperty(prop) => {
                if self.config.include_declarations {
                    graph.insert(&Triple {
                        subject: prop.iri().as_ref().into(),
                        predicate: rdf::TYPE.into(),
                        object: owl::ANNOTATION_PROPERTY.into(),
                    });
                }
            }

            Axiom::DeclareNamedIndividual(individual) => {
                if self.config.include_declarations {
                    graph.insert(&Triple {
                        subject: self.individual_to_subject(individual),
                        predicate: rdf::TYPE.into(),
                        object: owl::NAMED_INDIVIDUAL.into(),
                    });
                }
            }

            // Property chain is not yet implemented
            Axiom::SubPropertyChainOf { .. } => {
                // TODO: Implement property chain serialization
            }
        }
    }

    /// Serializes a class expression to a term.
    fn serialize_class_expression(&mut self, expr: &ClassExpression, graph: &mut Graph) -> Term {
        match expr {
            ClassExpression::Class(class) => Term::NamedNode(class.iri().clone()),

            ClassExpression::ObjectIntersectionOf(classes) => {
                let bnode = self.fresh_blank_node();
                let class_terms: Vec<Term> = classes
                    .iter()
                    .map(|c| self.serialize_class_expression(c, graph))
                    .collect();
                let list = self.create_rdf_list(class_terms, graph);
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::INTERSECTION_OF.into(),
                    object: list.into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::ObjectUnionOf(classes) => {
                let bnode = self.fresh_blank_node();
                let class_terms: Vec<Term> = classes
                    .iter()
                    .map(|c| self.serialize_class_expression(c, graph))
                    .collect();
                let list = self.create_rdf_list(class_terms, graph);
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::UNION_OF.into(),
                    object: list.into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::ObjectComplementOf(class) => {
                let bnode = self.fresh_blank_node();
                let complement_term = self.serialize_class_expression(class, graph);
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::COMPLEMENT_OF.into(),
                    object: complement_term.into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::ObjectOneOf(individuals) => {
                let bnode = self.fresh_blank_node();
                let ind_terms: Vec<Term> = individuals
                    .iter()
                    .map(|i| self.individual_to_term(i))
                    .collect();
                let list = self.create_rdf_list(ind_terms, graph);
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ONE_OF.into(),
                    object: list.into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::ObjectSomeValuesFrom { property, filler } => {
                let bnode = self.fresh_blank_node();
                let prop_term = self.serialize_object_property_expression(property);
                let filler_term = self.serialize_class_expression(filler, graph);

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: prop_term.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::SOME_VALUES_FROM.into(),
                    object: filler_term.into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::ObjectAllValuesFrom { property, filler } => {
                let bnode = self.fresh_blank_node();
                let prop_term = self.serialize_object_property_expression(property);
                let filler_term = self.serialize_class_expression(filler, graph);

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: prop_term.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ALL_VALUES_FROM.into(),
                    object: filler_term.into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::ObjectHasValue { property, individual } => {
                let bnode = self.fresh_blank_node();
                let prop_term = self.serialize_object_property_expression(property);
                let ind_term = self.individual_to_term(individual);

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: prop_term.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::HAS_VALUE.into(),
                    object: ind_term.into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::ObjectHasSelf(property) => {
                let bnode = self.fresh_blank_node();
                let prop_term = self.serialize_object_property_expression(property);

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: prop_term.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::HAS_SELF.into(),
                    object: Literal::from(true).into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::ObjectMinCardinality { cardinality, property, filler } => {
                let bnode = self.fresh_blank_node();
                let prop_term = self.serialize_object_property_expression(property);

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: prop_term.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::MIN_CARDINALITY.into(),
                    object: Literal::new_typed_literal(cardinality.to_string(), xsd::NON_NEGATIVE_INTEGER).into(),
                });
                if let Some(filler) = filler {
                    let filler_term = self.serialize_class_expression(filler, graph);
                    graph.insert(&Triple {
                        subject: bnode.as_ref().into(),
                        predicate: owl::ON_CLASS.into(),
                        object: filler_term.into(),
                    });
                }
                Term::BlankNode(bnode)
            }

            ClassExpression::ObjectMaxCardinality { cardinality, property, filler } => {
                let bnode = self.fresh_blank_node();
                let prop_term = self.serialize_object_property_expression(property);

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: prop_term.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::MAX_CARDINALITY.into(),
                    object: Literal::new_typed_literal(cardinality.to_string(), xsd::NON_NEGATIVE_INTEGER).into(),
                });
                if let Some(filler) = filler {
                    let filler_term = self.serialize_class_expression(filler, graph);
                    graph.insert(&Triple {
                        subject: bnode.as_ref().into(),
                        predicate: owl::ON_CLASS.into(),
                        object: filler_term.into(),
                    });
                }
                Term::BlankNode(bnode)
            }

            ClassExpression::ObjectExactCardinality { cardinality, property, filler } => {
                let bnode = self.fresh_blank_node();
                let prop_term = self.serialize_object_property_expression(property);

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: prop_term.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::CARDINALITY.into(),
                    object: Literal::new_typed_literal(cardinality.to_string(), xsd::NON_NEGATIVE_INTEGER).into(),
                });
                if let Some(filler) = filler {
                    let filler_term = self.serialize_class_expression(filler, graph);
                    graph.insert(&Triple {
                        subject: bnode.as_ref().into(),
                        predicate: owl::ON_CLASS.into(),
                        object: filler_term.into(),
                    });
                }
                Term::BlankNode(bnode)
            }

            // Data property restrictions
            ClassExpression::DataSomeValuesFrom { property, filler } => {
                let bnode = self.fresh_blank_node();
                let filler_term = self.serialize_data_range(filler, graph);

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: property.iri().as_ref().into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::SOME_VALUES_FROM.into(),
                    object: filler_term.into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::DataAllValuesFrom { property, filler } => {
                let bnode = self.fresh_blank_node();
                let filler_term = self.serialize_data_range(filler, graph);

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: property.iri().as_ref().into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ALL_VALUES_FROM.into(),
                    object: filler_term.into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::DataHasValue { property, value } => {
                let bnode = self.fresh_blank_node();

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: property.iri().as_ref().into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::HAS_VALUE.into(),
                    object: value.as_ref().into(),
                });
                Term::BlankNode(bnode)
            }

            ClassExpression::DataMinCardinality { cardinality, property, filler } => {
                let bnode = self.fresh_blank_node();

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: property.iri().as_ref().into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::MIN_CARDINALITY.into(),
                    object: Literal::new_typed_literal(cardinality.to_string(), xsd::NON_NEGATIVE_INTEGER).into(),
                });
                if let Some(filler) = filler {
                    let filler_term = self.serialize_data_range(filler, graph);
                    graph.insert(&Triple {
                        subject: bnode.as_ref().into(),
                        predicate: owl::ON_DATA_RANGE.into(),
                        object: filler_term.into(),
                    });
                }
                Term::BlankNode(bnode)
            }

            ClassExpression::DataMaxCardinality { cardinality, property, filler } => {
                let bnode = self.fresh_blank_node();

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: property.iri().as_ref().into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::MAX_CARDINALITY.into(),
                    object: Literal::new_typed_literal(cardinality.to_string(), xsd::NON_NEGATIVE_INTEGER).into(),
                });
                if let Some(filler) = filler {
                    let filler_term = self.serialize_data_range(filler, graph);
                    graph.insert(&Triple {
                        subject: bnode.as_ref().into(),
                        predicate: owl::ON_DATA_RANGE.into(),
                        object: filler_term.into(),
                    });
                }
                Term::BlankNode(bnode)
            }

            ClassExpression::DataExactCardinality { cardinality, property, filler } => {
                let bnode = self.fresh_blank_node();

                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: rdf::TYPE.into(),
                    object: owl::RESTRICTION.into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ON_PROPERTY.into(),
                    object: property.iri().as_ref().into(),
                });
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::CARDINALITY.into(),
                    object: Literal::new_typed_literal(cardinality.to_string(), xsd::NON_NEGATIVE_INTEGER).into(),
                });
                if let Some(filler) = filler {
                    let filler_term = self.serialize_data_range(filler, graph);
                    graph.insert(&Triple {
                        subject: bnode.as_ref().into(),
                        predicate: owl::ON_DATA_RANGE.into(),
                        object: filler_term.into(),
                    });
                }
                Term::BlankNode(bnode)
            }
        }
    }

    /// Serializes an object property expression to a term.
    fn serialize_object_property_expression(&self, expr: &ObjectPropertyExpression) -> Term {
        match expr {
            ObjectPropertyExpression::ObjectProperty(prop) => Term::NamedNode(prop.iri().clone()),
            ObjectPropertyExpression::ObjectInverseOf(prop) => {
                // Inverse properties need blank node representation
                // For now, just use the base property
                // TODO: Properly serialize inverse properties
                Term::NamedNode(prop.iri().clone())
            }
        }
    }

    /// Serializes a data range to a term.
    fn serialize_data_range(&mut self, range: &DataRange, graph: &mut Graph) -> Term {
        match range {
            DataRange::Datatype(dt) => Term::NamedNode(dt.clone()),

            DataRange::DataIntersectionOf(ranges) => {
                let bnode = self.fresh_blank_node();
                let range_terms: Vec<Term> = ranges
                    .iter()
                    .map(|r| self.serialize_data_range(r, graph))
                    .collect();
                let list = self.create_rdf_list(range_terms, graph);
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::INTERSECTION_OF.into(),
                    object: list.into(),
                });
                Term::BlankNode(bnode)
            }

            DataRange::DataUnionOf(ranges) => {
                let bnode = self.fresh_blank_node();
                let range_terms: Vec<Term> = ranges
                    .iter()
                    .map(|r| self.serialize_data_range(r, graph))
                    .collect();
                let list = self.create_rdf_list(range_terms, graph);
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::UNION_OF.into(),
                    object: list.into(),
                });
                Term::BlankNode(bnode)
            }

            DataRange::DataComplementOf(range) => {
                let bnode = self.fresh_blank_node();
                let complement_term = self.serialize_data_range(range, graph);
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::COMPLEMENT_OF.into(),
                    object: complement_term.into(),
                });
                Term::BlankNode(bnode)
            }

            DataRange::DataOneOf(literals) => {
                let bnode = self.fresh_blank_node();
                let lit_terms: Vec<Term> = literals
                    .iter()
                    .map(|l| Term::Literal(l.clone()))
                    .collect();
                let list = self.create_rdf_list(lit_terms, graph);
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl::ONE_OF.into(),
                    object: list.into(),
                });
                Term::BlankNode(bnode)
            }

            DataRange::DatatypeRestriction { datatype, facets } => {
                let bnode = self.fresh_blank_node();
                graph.insert(&Triple {
                    subject: bnode.as_ref().into(),
                    predicate: owl_term_ref("onDatatype").into(),
                    object: datatype.as_ref().into(),
                });
                // Add facet restrictions
                for (facet, _value) in facets {
                    graph.insert(&Triple {
                        subject: bnode.as_ref().into(),
                        predicate: owl_term_ref("withRestrictions").into(),
                        object: facet.as_ref().into(),
                    });
                    // This is simplified - proper facet serialization is more complex
                }
                Term::BlankNode(bnode)
            }
        }
    }

    /// Creates an RDF list from a vector of terms.
    fn create_rdf_list(&mut self, items: Vec<Term>, graph: &mut Graph) -> Term {
        if items.is_empty() {
            return Term::NamedNode(rdf::NIL.into_owned());
        }

        let mut current = Term::NamedNode(rdf::NIL.into_owned());

        // Build list from back to front
        for item in items.into_iter().rev() {
            let list_node = self.fresh_blank_node();
            graph.insert(&Triple {
                subject: list_node.as_ref().into(),
                predicate: rdf::FIRST.into(),
                object: item.into(),
            });
            graph.insert(&Triple {
                subject: list_node.as_ref().into(),
                predicate: rdf::REST.into(),
                object: current.into(),
            });
            current = Term::BlankNode(list_node);
        }

        current
    }

    /// Converts an individual to a subject.
    fn individual_to_subject(&self, individual: &Individual) -> NamedOrBlankNode {
        match individual {
            Individual::Named(n) => n.clone().into(),
            Individual::Anonymous(b) => b.clone().into(),
        }
    }

    /// Converts an individual to a term.
    fn individual_to_term(&self, individual: &Individual) -> Term {
        match individual {
            Individual::Named(n) => Term::NamedNode(n.clone()),
            Individual::Anonymous(b) => Term::BlankNode(b.clone()),
        }
    }

    /// Converts a term to a subject (assumes it's a named node or blank node).
    fn term_to_subject(&self, term: Term) -> NamedOrBlankNode {
        match term {
            Term::NamedNode(n) => n.into(),
            Term::BlankNode(b) => b.into(),
            Term::Literal(_) => panic!("Cannot convert literal to subject"),
            _ => panic!("Unsupported term type for subject"),
        }
    }

    /// Generates a fresh blank node.
    fn fresh_blank_node(&mut self) -> BlankNode {
        self.blank_node_counter += 1;
        BlankNode::new_unchecked(format!("b{}", self.blank_node_counter))
    }
}

impl Default for OntologySerializer {
    fn default() -> Self {
        Self::new()
    }
}

/// Serializes an ontology to an RDF graph.
pub fn serialize_ontology(ontology: &Ontology) -> Graph {
    OntologySerializer::new().serialize(ontology)
}

/// Serializes an ontology to an RDF graph with custom configuration.
pub fn serialize_ontology_with_config(ontology: &Ontology, config: SerializerConfig) -> Graph {
    OntologySerializer::with_config(config).serialize(ontology)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Axiom, ClassExpression, Individual, ObjectProperty, OwlClass};

    #[test]
    fn test_serialize_simple_ontology() {
        let mut ontology = Ontology::with_iri("http://example.org/test").unwrap();

        let animal = OwlClass::new(NamedNode::new_unchecked("http://example.org/Animal"));
        let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));

        ontology.add_axiom(Axiom::subclass_of(
            ClassExpression::class(dog.clone()),
            ClassExpression::class(animal.clone()),
        ));

        let graph = serialize_ontology(&ontology);

        // Verify ontology declaration
        assert!(graph.contains(&Triple {
            subject: NamedNode::new_unchecked("http://example.org/test").into(),
            predicate: rdf::TYPE.into(),
            object: owl::ONTOLOGY.into(),
        }));

        // Verify subclass axiom
        assert!(graph.contains(&Triple {
            subject: NamedNode::new_unchecked("http://example.org/Dog").into(),
            predicate: rdfs::SUB_CLASS_OF.into(),
            object: NamedNode::new_unchecked("http://example.org/Animal").into(),
        }));
    }

    #[test]
    fn test_serialize_class_assertion() {
        let mut ontology = Ontology::new(None);

        let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));
        let fido = Individual::Named(NamedNode::new_unchecked("http://example.org/fido"));

        ontology.add_axiom(Axiom::class_assertion(
            ClassExpression::class(dog),
            fido.clone(),
        ));

        let graph = serialize_ontology(&ontology);

        assert!(graph.contains(&Triple {
            subject: NamedNode::new_unchecked("http://example.org/fido").into(),
            predicate: rdf::TYPE.into(),
            object: NamedNode::new_unchecked("http://example.org/Dog").into(),
        }));
    }

    #[test]
    fn test_serialize_property_assertion() {
        let mut ontology = Ontology::new(None);

        let owns = ObjectProperty::new(NamedNode::new_unchecked("http://example.org/owns"));
        let alice = Individual::Named(NamedNode::new_unchecked("http://example.org/Alice"));
        let fido = Individual::Named(NamedNode::new_unchecked("http://example.org/fido"));

        ontology.add_axiom(Axiom::ObjectPropertyAssertion {
            property: owns.clone(),
            source: alice.clone(),
            target: fido.clone(),
        });

        let graph = serialize_ontology(&ontology);

        assert!(graph.contains(&Triple {
            subject: NamedNode::new_unchecked("http://example.org/Alice").into(),
            predicate: NamedNode::new_unchecked("http://example.org/owns"),
            object: NamedNode::new_unchecked("http://example.org/fido").into(),
        }));
    }

    #[test]
    fn test_serialize_restriction() {
        let mut ontology = Ontology::new(None);

        let has_pet = ObjectProperty::new(NamedNode::new_unchecked("http://example.org/hasPet"));
        let dog = OwlClass::new(NamedNode::new_unchecked("http://example.org/Dog"));
        let pet_owner = OwlClass::new(NamedNode::new_unchecked("http://example.org/PetOwner"));

        // PetOwner ≡ ∃hasPet.Dog
        let restriction = ClassExpression::some_values_from(
            has_pet,
            ClassExpression::class(dog),
        );

        ontology.add_axiom(Axiom::EquivalentClasses(vec![
            ClassExpression::class(pet_owner),
            restriction,
        ]));

        let graph = serialize_ontology(&ontology);

        // Check that restriction was created (it will have a blank node)
        let mut has_restriction = false;
        for triple in graph.iter() {
            if triple.predicate == owl::SOME_VALUES_FROM {
                has_restriction = true;
                break;
            }
        }
        assert!(has_restriction, "Should have someValuesFrom restriction");
    }
}
