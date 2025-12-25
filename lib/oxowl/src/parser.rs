//! OWL 2 parser from RDF graphs.
//!
//! This module provides functionality to parse OWL 2 ontologies from RDF
//! representations (Turtle, RDF/XML, N-Triples, etc.).

use crate::axiom::Axiom;
use crate::entity::{OwlClass, ObjectProperty, DataProperty, AnnotationProperty, Individual};
use crate::expression::{ClassExpression, ObjectPropertyExpression};
use crate::ontology::Ontology;
use crate::error::{OwlParseError, ParseErrorKind};
use oxrdf::{
    Graph, BlankNodeRef, NamedOrBlankNodeRef,
    Term, TermRef,
    vocab::{rdf, rdfs, owl},
};
use rustc_hash::{FxHashMap, FxHashSet};

/// OWL 2 namespace.
pub const OWL_NAMESPACE: &str = "http://www.w3.org/2002/07/owl#";


/// Parser configuration.
#[derive(Debug, Clone, Default)]
pub struct ParserConfig {
    /// Maximum depth for parsing nested expressions.
    pub max_depth: usize,
    /// Maximum length for RDF lists.
    pub max_list_length: usize,
    /// Whether to be lenient about missing declarations.
    pub lenient: bool,
}

impl ParserConfig {
    /// Creates a new parser configuration with default values.
    pub fn new() -> Self {
        Self {
            max_depth: 100,
            max_list_length: 10000,
            lenient: false,
        }
    }

    /// Sets lenient mode.
    #[must_use]
    pub fn lenient(mut self) -> Self {
        self.lenient = true;
        self
    }
}

/// Parses an OWL ontology from an RDF graph.
pub struct OntologyParser<'a> {
    graph: &'a Graph,
    config: ParserConfig,
}

impl<'a> OntologyParser<'a> {
    /// Creates a new parser for the given graph.
    pub fn new(graph: &'a Graph) -> Self {
        Self::with_config(graph, ParserConfig::new())
    }

    /// Creates a new parser with custom configuration.
    pub fn with_config(graph: &'a Graph, config: ParserConfig) -> Self {
        Self {
            graph,
            config,
        }
    }

    /// Parses the ontology from the graph.
    pub fn parse(&mut self) -> Result<Ontology, OwlParseError> {
        let mut ontology = Ontology::new(None);

        // Find ontology IRI
        for triple in self.graph.triples_for_predicate(rdf::TYPE) {
            if let TermRef::NamedNode(obj) = triple.object {
                if obj == owl::ONTOLOGY {
                    if let NamedOrBlankNodeRef::NamedNode(subject) = triple.subject {
                        ontology.set_iri(Some(subject.into_owned()));

                        // Parse imports
                        for import_triple in self.graph.triples_for_subject(triple.subject) {
                            if import_triple.predicate == owl::IMPORTS {
                                if let TermRef::NamedNode(import_iri) = import_triple.object {
                                    ontology.add_import(import_iri.into_owned());
                                }
                            } else if import_triple.predicate == owl::VERSION_IRI {
                                if let TermRef::NamedNode(version) = import_triple.object {
                                    ontology.set_version_iri(Some(version.into_owned()));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Parse declarations
        self.parse_declarations(&mut ontology)?;

        // Parse axioms
        self.parse_axioms(&mut ontology)?;

        Ok(ontology)
    }

    /// Parses entity declarations.
    fn parse_declarations(&self, ontology: &mut Ontology) -> Result<(), OwlParseError> {
        for triple in self.graph.triples_for_predicate(rdf::TYPE) {
            if let TermRef::NamedNode(obj) = triple.object {
                if let NamedOrBlankNodeRef::NamedNode(subject) = triple.subject {
                    let node = subject.into_owned();

                    if obj == owl::CLASS {
                        ontology.add_axiom(Axiom::DeclareClass(OwlClass::new(node)));
                    } else if obj == owl::OBJECT_PROPERTY {
                        ontology.add_axiom(Axiom::DeclareObjectProperty(ObjectProperty::new(node)));
                    } else if obj == owl::DATATYPE_PROPERTY {
                        ontology.add_axiom(Axiom::DeclareDataProperty(DataProperty::new(node)));
                    } else if obj == owl::ANNOTATION_PROPERTY {
                        ontology.add_axiom(Axiom::DeclareAnnotationProperty(AnnotationProperty::new(node)));
                    } else if obj == owl::NAMED_INDIVIDUAL {
                        ontology.add_axiom(Axiom::DeclareNamedIndividual(Individual::Named(node)));
                    }
                }
            }
        }
        Ok(())
    }

    /// Parses axioms from the graph.
    fn parse_axioms(&mut self, ontology: &mut Ontology) -> Result<(), OwlParseError> {
        // Parse SubClassOf (rdfs:subClassOf)
        for triple in self.graph.triples_for_predicate(rdfs::SUB_CLASS_OF) {
            let sub_class = self.parse_class_expression(triple.subject.into())?;
            let super_class = self.parse_class_expression(triple.object)?;
            ontology.add_axiom(Axiom::SubClassOf { sub_class, super_class });
        }

        // Parse EquivalentClass (owl:equivalentClass)
        for triple in self.graph.triples_for_predicate(owl::EQUIVALENT_CLASS) {
            let class1 = self.parse_class_expression(triple.subject.into())?;
            let class2 = self.parse_class_expression(triple.object)?;
            ontology.add_axiom(Axiom::EquivalentClasses(vec![class1, class2]));
        }

        // Parse DisjointWith (owl:disjointWith)
        for triple in self.graph.triples_for_predicate(owl::DISJOINT_WITH) {
            let class1 = self.parse_class_expression(triple.subject.into())?;
            let class2 = self.parse_class_expression(triple.object)?;
            ontology.add_axiom(Axiom::DisjointClasses(vec![class1, class2]));
        }

        // Parse ClassAssertion (individual rdf:type class)
        for triple in self.graph.triples_for_predicate(rdf::TYPE) {
            // Skip OWL vocabulary types
            if let TermRef::NamedNode(obj) = triple.object {
                let obj_str = obj.as_str();
                if obj_str.starts_with(OWL_NAMESPACE) || obj_str.starts_with("http://www.w3.org/2000/01/rdf-schema#") {
                    continue;
                }
            }

            let individual = match triple.subject {
                NamedOrBlankNodeRef::NamedNode(n) => Individual::Named(n.into_owned()),
                NamedOrBlankNodeRef::BlankNode(b) => Individual::Anonymous(b.into_owned()),
            };

            if let Ok(class) = self.parse_class_expression(triple.object) {
                ontology.add_axiom(Axiom::ClassAssertion { class, individual });
            }
        }

        // Parse SameAs (owl:sameAs)
        let mut same_as_pairs: FxHashMap<Term, Vec<Term>> = FxHashMap::default();
        for triple in self.graph.triples_for_predicate(owl::SAME_AS) {
            let subject: Term = triple.subject.into();
            let object: Term = triple.object.into();
            same_as_pairs.entry(subject.clone()).or_default().push(object.clone());
            same_as_pairs.entry(object).or_default().push(subject);
        }
        // Group into equivalence classes
        let mut processed = FxHashSet::default();
        for (term, related) in &same_as_pairs {
            if !processed.contains(term) {
                let mut group = vec![self.term_to_individual(term.as_ref())?];
                for r in related {
                    if !processed.contains(r) {
                        group.push(self.term_to_individual(r.as_ref())?);
                        processed.insert(r.clone());
                    }
                }
                processed.insert(term.clone());
                if group.len() > 1 {
                    ontology.add_axiom(Axiom::SameIndividual(group));
                }
            }
        }

        // Parse DifferentFrom (owl:differentFrom)
        for triple in self.graph.triples_for_predicate(owl::DIFFERENT_FROM) {
            let ind1 = self.term_to_individual(triple.subject.into())?;
            let ind2 = self.term_to_individual(triple.object)?;
            ontology.add_axiom(Axiom::DifferentIndividuals(vec![ind1, ind2]));
        }

        // Parse property characteristics
        for triple in self.graph.triples_for_predicate(rdf::TYPE) {
            if let NamedOrBlankNodeRef::NamedNode(subject) = triple.subject {
                let prop = ObjectProperty::new(subject.into_owned());
                if let TermRef::NamedNode(obj) = triple.object {
                    if obj == owl::FUNCTIONAL_PROPERTY {
                        ontology.add_axiom(Axiom::FunctionalObjectProperty(prop));
                    } else if obj == owl::INVERSE_FUNCTIONAL_PROPERTY {
                        ontology.add_axiom(Axiom::InverseFunctionalObjectProperty(prop));
                    } else if obj == owl::TRANSITIVE_PROPERTY {
                        ontology.add_axiom(Axiom::TransitiveObjectProperty(prop));
                    } else if obj == owl::SYMMETRIC_PROPERTY {
                        ontology.add_axiom(Axiom::SymmetricObjectProperty(prop));
                    } else if obj == owl::ASYMMETRIC_PROPERTY {
                        ontology.add_axiom(Axiom::AsymmetricObjectProperty(prop));
                    } else if obj == owl::REFLEXIVE_PROPERTY {
                        ontology.add_axiom(Axiom::ReflexiveObjectProperty(prop));
                    } else if obj == owl::IRREFLEXIVE_PROPERTY {
                        ontology.add_axiom(Axiom::IrreflexiveObjectProperty(prop));
                    }
                }
            }
        }

        // Parse InverseOf
        for triple in self.graph.triples_for_predicate(owl::INVERSE_OF) {
            if let (Some(sub), TermRef::NamedNode(obj)) = (match triple.subject { NamedOrBlankNodeRef::NamedNode(n) => Some(n), _ => None }, triple.object) {
                ontology.add_axiom(Axiom::InverseObjectProperties(
                    ObjectProperty::new(sub.into_owned()),
                    ObjectProperty::new(obj.into_owned()),
                ));
            }
        }

        // Parse SubPropertyOf (rdfs:subPropertyOf)
        for triple in self.graph.triples_for_predicate(rdfs::SUB_PROPERTY_OF) {
            if let (Some(sub), TermRef::NamedNode(sup)) = (match triple.subject { NamedOrBlankNodeRef::NamedNode(n) => Some(n), _ => None }, triple.object) {
                ontology.add_axiom(Axiom::SubObjectPropertyOf {
                    sub_property: ObjectPropertyExpression::ObjectProperty(ObjectProperty::new(sub.into_owned())),
                    super_property: ObjectPropertyExpression::ObjectProperty(ObjectProperty::new(sup.into_owned())),
                });
            }
        }

        // Parse domain (rdfs:domain)
        for triple in self.graph.triples_for_predicate(rdfs::DOMAIN) {
            if let NamedOrBlankNodeRef::NamedNode(sub) = triple.subject {
                if let Ok(domain) = self.parse_class_expression(triple.object) {
                    ontology.add_axiom(Axiom::ObjectPropertyDomain {
                        property: ObjectProperty::new(sub.into_owned()),
                        domain,
                    });
                }
            }
        }

        // Parse range (rdfs:range)
        for triple in self.graph.triples_for_predicate(rdfs::RANGE) {
            if let NamedOrBlankNodeRef::NamedNode(sub) = triple.subject {
                if let Ok(range) = self.parse_class_expression(triple.object) {
                    ontology.add_axiom(Axiom::ObjectPropertyRange {
                        property: ObjectProperty::new(sub.into_owned()),
                        range,
                    });
                }
            }
        }

        Ok(())
    }

    /// Parses a class expression from a term.
    fn parse_class_expression(&mut self, term: TermRef<'_>) -> Result<ClassExpression, OwlParseError> {
        match term {
            TermRef::NamedNode(n) => {
                Ok(ClassExpression::Class(OwlClass::new(n.into_owned())))
            }
            TermRef::BlankNode(b) => {
                // Could be a restriction or boolean class expression
                self.parse_anonymous_class(b)
            }
            TermRef::Literal(_) => {
                Err(OwlParseError::invalid_value("Literal cannot be a class expression"))
            }
            _ => {
                Err(OwlParseError::invalid_value("Unsupported term type for class expression"))
            }
        }
    }

    /// Parses an anonymous class expression (restriction or boolean).
    fn parse_anonymous_class(&mut self, bnode: BlankNodeRef<'_>) -> Result<ClassExpression, OwlParseError> {
        let subject: NamedOrBlankNodeRef<'_> = bnode.into();

        // Check for restriction
        for triple in self.graph.triples_for_subject(subject) {
            if triple.predicate == rdf::TYPE {
                if let TermRef::NamedNode(obj) = triple.object {
                    if obj == owl::RESTRICTION {
                        return self.parse_restriction(bnode);
                    }
                }
            }
        }

        // Check for boolean class expressions
        for triple in self.graph.triples_for_subject(subject) {
            if triple.predicate == owl::INTERSECTION_OF {
                let classes = self.parse_class_list(triple.object)?;
                return Ok(ClassExpression::ObjectIntersectionOf(classes));
            } else if triple.predicate == owl::UNION_OF {
                let classes = self.parse_class_list(triple.object)?;
                return Ok(ClassExpression::ObjectUnionOf(classes));
            } else if triple.predicate == owl::COMPLEMENT_OF {
                let class = self.parse_class_expression(triple.object)?;
                return Ok(ClassExpression::ObjectComplementOf(Box::new(class)));
            } else if triple.predicate == owl::ONE_OF {
                let individuals = self.parse_individual_list(triple.object)?;
                return Ok(ClassExpression::ObjectOneOf(individuals));
            }
        }

        Err(OwlParseError::new(
            ParseErrorKind::UnknownConstruct,
            format!("Cannot parse anonymous class: {}", bnode),
        ))
    }

    /// Parses an OWL restriction.
    fn parse_restriction(&mut self, bnode: BlankNodeRef<'_>) -> Result<ClassExpression, OwlParseError> {
        let subject: NamedOrBlankNodeRef<'_> = bnode.into();

        // Get the property
        let mut property = None;
        let mut some_values = None;
        let mut all_values = None;
        let mut has_value = None;
        let mut min_card = None;
        let mut max_card = None;
        let mut exact_card = None;
        let mut on_class = None;

        for triple in self.graph.triples_for_subject(subject) {
            if triple.predicate == owl::ON_PROPERTY {
                if let TermRef::NamedNode(p) = triple.object {
                    property = Some(ObjectProperty::new(p.into_owned()));
                }
            } else if triple.predicate == owl::SOME_VALUES_FROM {
                some_values = Some(self.parse_class_expression(triple.object)?);
            } else if triple.predicate == owl::ALL_VALUES_FROM {
                all_values = Some(self.parse_class_expression(triple.object)?);
            } else if triple.predicate == owl::HAS_VALUE {
                has_value = Some(triple.object);
            } else if triple.predicate == owl::MIN_CARDINALITY {
                if let TermRef::Literal(lit) = triple.object {
                    min_card = lit.value().parse().ok();
                }
            } else if triple.predicate == owl::MAX_CARDINALITY {
                if let TermRef::Literal(lit) = triple.object {
                    max_card = lit.value().parse().ok();
                }
            } else if triple.predicate == owl::CARDINALITY {
                if let TermRef::Literal(lit) = triple.object {
                    exact_card = lit.value().parse().ok();
                }
            } else if triple.predicate == owl::ON_CLASS {
                on_class = Some(self.parse_class_expression(triple.object)?);
            }
        }

        let property = property.ok_or_else(|| OwlParseError::missing_property("owl:onProperty"))?;
        let prop_expr = ObjectPropertyExpression::ObjectProperty(property);

        if let Some(filler) = some_values {
            return Ok(ClassExpression::ObjectSomeValuesFrom {
                property: prop_expr,
                filler: Box::new(filler),
            });
        }

        if let Some(filler) = all_values {
            return Ok(ClassExpression::ObjectAllValuesFrom {
                property: prop_expr,
                filler: Box::new(filler),
            });
        }

        if let Some(value) = has_value {
            let individual = self.term_to_individual(value)?;
            return Ok(ClassExpression::ObjectHasValue {
                property: prop_expr,
                individual,
            });
        }

        if let Some(n) = min_card {
            return Ok(ClassExpression::ObjectMinCardinality {
                cardinality: n,
                property: prop_expr,
                filler: on_class.map(Box::new),
            });
        }

        if let Some(n) = max_card {
            return Ok(ClassExpression::ObjectMaxCardinality {
                cardinality: n,
                property: prop_expr,
                filler: on_class.map(Box::new),
            });
        }

        if let Some(n) = exact_card {
            return Ok(ClassExpression::ObjectExactCardinality {
                cardinality: n,
                property: prop_expr,
                filler: on_class.map(Box::new),
            });
        }

        Err(OwlParseError::new(
            ParseErrorKind::UnknownConstruct,
            "Unknown restriction type",
        ))
    }

    /// Parses an RDF list of class expressions.
    fn parse_class_list(&mut self, head: TermRef<'_>) -> Result<Vec<ClassExpression>, OwlParseError> {
        let mut result = Vec::new();
        let mut current = head.into_owned();
        let mut count = 0;

        while current != Term::NamedNode(rdf::NIL.into_owned()) {
            count += 1;
            if count > self.config.max_list_length {
                return Err(OwlParseError::malformed_list("List too long"));
            }

            // Convert current to NamedOrBlankNodeRef
            let current_ref = match current.as_ref() {
                TermRef::NamedNode(n) => NamedOrBlankNodeRef::NamedNode(n),
                TermRef::BlankNode(b) => NamedOrBlankNodeRef::BlankNode(b),
                _ => return Err(OwlParseError::malformed_list("List node must be named or blank node")),
            };

            // Get rdf:first
            let first = self.graph
                .object_for_subject_predicate(current_ref, rdf::FIRST)
                .ok_or_else(|| OwlParseError::malformed_list("Missing rdf:first"))?;
            result.push(self.parse_class_expression(first)?);

            // Get rdf:rest
            current = self.graph
                .object_for_subject_predicate(current_ref, rdf::REST)
                .ok_or_else(|| OwlParseError::malformed_list("Missing rdf:rest"))?
                .into_owned();
        }

        Ok(result)
    }

    /// Parses an RDF list of individuals.
    fn parse_individual_list(&mut self, head: TermRef<'_>) -> Result<Vec<Individual>, OwlParseError> {
        let mut result = Vec::new();
        let mut current = head.into_owned();
        let mut count = 0;

        while current != Term::NamedNode(rdf::NIL.into_owned()) {
            count += 1;
            if count > self.config.max_list_length {
                return Err(OwlParseError::malformed_list("List too long"));
            }

            // Convert current to NamedOrBlankNodeRef
            let current_ref = match current.as_ref() {
                TermRef::NamedNode(n) => NamedOrBlankNodeRef::NamedNode(n),
                TermRef::BlankNode(b) => NamedOrBlankNodeRef::BlankNode(b),
                _ => return Err(OwlParseError::malformed_list("List node must be named or blank node")),
            };

            let first = self.graph
                .object_for_subject_predicate(current_ref, rdf::FIRST)
                .ok_or_else(|| OwlParseError::malformed_list("Missing rdf:first"))?;
            result.push(self.term_to_individual(first)?);

            current = self.graph
                .object_for_subject_predicate(current_ref, rdf::REST)
                .ok_or_else(|| OwlParseError::malformed_list("Missing rdf:rest"))?
                .into_owned();
        }

        Ok(result)
    }

    /// Converts a term to an individual.
    fn term_to_individual(&self, term: TermRef<'_>) -> Result<Individual, OwlParseError> {
        match term {
            TermRef::NamedNode(n) => Ok(Individual::Named(n.into_owned())),
            TermRef::BlankNode(b) => Ok(Individual::Anonymous(b.into_owned())),
            _ => Err(OwlParseError::invalid_value("Expected individual")),
        }
    }
}


/// Parses an ontology from an RDF graph.
pub fn parse_ontology(graph: &Graph) -> Result<Ontology, OwlParseError> {
    OntologyParser::new(graph).parse()
}

/// Parses an ontology with custom configuration.
pub fn parse_ontology_with_config(graph: &Graph, config: ParserConfig) -> Result<Ontology, OwlParseError> {
    OntologyParser::with_config(graph, config).parse()
}
