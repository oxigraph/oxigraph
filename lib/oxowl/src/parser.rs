//! OWL 2 parser from RDF graphs.
//!
//! This module provides functionality to parse OWL 2 ontologies from RDF
//! representations (Turtle, RDF/XML, N-Triples, etc.).

use crate::axiom::Axiom;
use crate::entity::{OwlClass, ObjectProperty, DataProperty, AnnotationProperty, Individual};
use crate::expression::{ClassExpression, ObjectPropertyExpression, DataRange};
use crate::ontology::Ontology;
use crate::error::{OwlParseError, ParseErrorKind};
use oxrdf::{
    Graph, GraphRef, NamedNode, NamedNodeRef, BlankNode, BlankNodeRef,
    Term, TermRef, Triple, TripleRef, Literal,
    vocab::{rdf, rdfs, xsd},
};
use rustc_hash::{FxHashMap, FxHashSet};

/// OWL 2 namespace.
pub const OWL_NAMESPACE: &str = "http://www.w3.org/2002/07/owl#";

// OWL vocabulary constants
mod vocab {
    use oxrdf::NamedNodeRef;

    pub const CLASS: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#Class");
    pub const THING: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#Thing");
    pub const NOTHING: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#Nothing");
    pub const ONTOLOGY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#Ontology");
    pub const OBJECT_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#ObjectProperty");
    pub const DATATYPE_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#DatatypeProperty");
    pub const ANNOTATION_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#AnnotationProperty");
    pub const NAMED_INDIVIDUAL: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#NamedIndividual");
    pub const RESTRICTION: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#Restriction");

    // Properties
    pub const IMPORTS: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#imports");
    pub const VERSION_IRI: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#versionIRI");
    pub const EQUIVALENT_CLASS: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#equivalentClass");
    pub const DISJOINT_WITH: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#disjointWith");
    pub const EQUIVALENT_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#equivalentProperty");
    pub const INVERSE_OF: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#inverseOf");
    pub const SAME_AS: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#sameAs");
    pub const DIFFERENT_FROM: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#differentFrom");

    // Restrictions
    pub const ON_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#onProperty");
    pub const SOME_VALUES_FROM: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#someValuesFrom");
    pub const ALL_VALUES_FROM: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#allValuesFrom");
    pub const HAS_VALUE: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#hasValue");
    pub const MIN_CARDINALITY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#minCardinality");
    pub const MAX_CARDINALITY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#maxCardinality");
    pub const CARDINALITY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#cardinality");
    pub const ON_CLASS: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#onClass");

    // Boolean combinations
    pub const INTERSECTION_OF: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#intersectionOf");
    pub const UNION_OF: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#unionOf");
    pub const COMPLEMENT_OF: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#complementOf");
    pub const ONE_OF: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#oneOf");

    // Property characteristics
    pub const FUNCTIONAL_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#FunctionalProperty");
    pub const INVERSE_FUNCTIONAL_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#InverseFunctionalProperty");
    pub const TRANSITIVE_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#TransitiveProperty");
    pub const SYMMETRIC_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#SymmetricProperty");
    pub const ASYMMETRIC_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#AsymmetricProperty");
    pub const REFLEXIVE_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#ReflexiveProperty");
    pub const IRREFLEXIVE_PROPERTY: NamedNodeRef<'_> = NamedNodeRef::new_unchecked("http://www.w3.org/2002/07/owl#IrreflexiveProperty");
}

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
    pub fn lenient(mut self) -> Self {
        self.lenient = true;
        self
    }
}

/// Parses an OWL ontology from an RDF graph.
pub struct OntologyParser<'a> {
    graph: &'a Graph,
    config: ParserConfig,
    visited: FxHashSet<Term>,
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
            visited: FxHashSet::default(),
        }
    }

    /// Parses the ontology from the graph.
    pub fn parse(&mut self) -> Result<Ontology, OwlParseError> {
        let mut ontology = Ontology::new(None);

        // Find ontology IRI
        for triple in self.graph.triples_for_predicate(rdf::TYPE) {
            if let TermRef::NamedNode(obj) = triple.object {
                if obj == vocab::ONTOLOGY {
                    if let Some(subject) = triple.subject.as_named_node() {
                        ontology.set_iri(Some(subject.into_owned()));

                        // Parse imports
                        for import_triple in self.graph.triples_for_subject(triple.subject) {
                            if import_triple.predicate == vocab::IMPORTS {
                                if let TermRef::NamedNode(import_iri) = import_triple.object {
                                    ontology.add_import(import_iri.into_owned());
                                }
                            } else if import_triple.predicate == vocab::VERSION_IRI {
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
                if let Some(subject) = triple.subject.as_named_node() {
                    let node = subject.into_owned();

                    if obj == vocab::CLASS {
                        ontology.add_axiom(Axiom::DeclareClass(OwlClass::new(node)));
                    } else if obj == vocab::OBJECT_PROPERTY {
                        ontology.add_axiom(Axiom::DeclareObjectProperty(ObjectProperty::new(node)));
                    } else if obj == vocab::DATATYPE_PROPERTY {
                        ontology.add_axiom(Axiom::DeclareDataProperty(DataProperty::new(node)));
                    } else if obj == vocab::ANNOTATION_PROPERTY {
                        ontology.add_axiom(Axiom::DeclareAnnotationProperty(AnnotationProperty::new(node)));
                    } else if obj == vocab::NAMED_INDIVIDUAL {
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
        for triple in self.graph.triples_for_predicate(vocab::EQUIVALENT_CLASS) {
            let class1 = self.parse_class_expression(triple.subject.into())?;
            let class2 = self.parse_class_expression(triple.object)?;
            ontology.add_axiom(Axiom::EquivalentClasses(vec![class1, class2]));
        }

        // Parse DisjointWith (owl:disjointWith)
        for triple in self.graph.triples_for_predicate(vocab::DISJOINT_WITH) {
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
        for triple in self.graph.triples_for_predicate(vocab::SAME_AS) {
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
        for triple in self.graph.triples_for_predicate(vocab::DIFFERENT_FROM) {
            let ind1 = self.term_to_individual(triple.subject.into())?;
            let ind2 = self.term_to_individual(triple.object)?;
            ontology.add_axiom(Axiom::DifferentIndividuals(vec![ind1, ind2]));
        }

        // Parse property characteristics
        for triple in self.graph.triples_for_predicate(rdf::TYPE) {
            if let Some(subject) = triple.subject.as_named_node() {
                let prop = ObjectProperty::new(subject.into_owned());
                if let TermRef::NamedNode(obj) = triple.object {
                    if obj == vocab::FUNCTIONAL_PROPERTY {
                        ontology.add_axiom(Axiom::FunctionalObjectProperty(prop));
                    } else if obj == vocab::INVERSE_FUNCTIONAL_PROPERTY {
                        ontology.add_axiom(Axiom::InverseFunctionalObjectProperty(prop));
                    } else if obj == vocab::TRANSITIVE_PROPERTY {
                        ontology.add_axiom(Axiom::TransitiveObjectProperty(prop));
                    } else if obj == vocab::SYMMETRIC_PROPERTY {
                        ontology.add_axiom(Axiom::SymmetricObjectProperty(prop));
                    } else if obj == vocab::ASYMMETRIC_PROPERTY {
                        ontology.add_axiom(Axiom::AsymmetricObjectProperty(prop));
                    } else if obj == vocab::REFLEXIVE_PROPERTY {
                        ontology.add_axiom(Axiom::ReflexiveObjectProperty(prop));
                    } else if obj == vocab::IRREFLEXIVE_PROPERTY {
                        ontology.add_axiom(Axiom::IrreflexiveObjectProperty(prop));
                    }
                }
            }
        }

        // Parse InverseOf
        for triple in self.graph.triples_for_predicate(vocab::INVERSE_OF) {
            if let (Some(sub), TermRef::NamedNode(obj)) = (triple.subject.as_named_node(), triple.object) {
                ontology.add_axiom(Axiom::InverseObjectProperties(
                    ObjectProperty::new(sub.into_owned()),
                    ObjectProperty::new(obj.into_owned()),
                ));
            }
        }

        // Parse SubPropertyOf (rdfs:subPropertyOf)
        for triple in self.graph.triples_for_predicate(rdfs::SUB_PROPERTY_OF) {
            if let (Some(sub), TermRef::NamedNode(sup)) = (triple.subject.as_named_node(), triple.object) {
                ontology.add_axiom(Axiom::SubObjectPropertyOf {
                    sub_property: ObjectPropertyExpression::ObjectProperty(ObjectProperty::new(sub.into_owned())),
                    super_property: ObjectPropertyExpression::ObjectProperty(ObjectProperty::new(sup.into_owned())),
                });
            }
        }

        // Parse domain (rdfs:domain)
        for triple in self.graph.triples_for_predicate(rdfs::DOMAIN) {
            if let Some(sub) = triple.subject.as_named_node() {
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
            if let Some(sub) = triple.subject.as_named_node() {
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
            #[cfg(feature = "rdf-12")]
            TermRef::Triple(_) => {
                Err(OwlParseError::invalid_value("Quoted triple cannot be a class expression"))
            }
        }
    }

    /// Parses an anonymous class expression (restriction or boolean).
    fn parse_anonymous_class(&mut self, bnode: BlankNodeRef<'_>) -> Result<ClassExpression, OwlParseError> {
        let subject = bnode.into();

        // Check for restriction
        for triple in self.graph.triples_for_subject(subject) {
            if triple.predicate == rdf::TYPE {
                if let TermRef::NamedNode(obj) = triple.object {
                    if obj == vocab::RESTRICTION {
                        return self.parse_restriction(bnode);
                    }
                }
            }
        }

        // Check for boolean class expressions
        for triple in self.graph.triples_for_subject(subject) {
            if triple.predicate == vocab::INTERSECTION_OF {
                let classes = self.parse_class_list(triple.object)?;
                return Ok(ClassExpression::ObjectIntersectionOf(classes));
            } else if triple.predicate == vocab::UNION_OF {
                let classes = self.parse_class_list(triple.object)?;
                return Ok(ClassExpression::ObjectUnionOf(classes));
            } else if triple.predicate == vocab::COMPLEMENT_OF {
                let class = self.parse_class_expression(triple.object)?;
                return Ok(ClassExpression::ObjectComplementOf(Box::new(class)));
            } else if triple.predicate == vocab::ONE_OF {
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
        let subject = bnode.into();

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
            if triple.predicate == vocab::ON_PROPERTY {
                if let TermRef::NamedNode(p) = triple.object {
                    property = Some(ObjectProperty::new(p.into_owned()));
                }
            } else if triple.predicate == vocab::SOME_VALUES_FROM {
                some_values = Some(self.parse_class_expression(triple.object)?);
            } else if triple.predicate == vocab::ALL_VALUES_FROM {
                all_values = Some(self.parse_class_expression(triple.object)?);
            } else if triple.predicate == vocab::HAS_VALUE {
                has_value = Some(triple.object);
            } else if triple.predicate == vocab::MIN_CARDINALITY {
                if let TermRef::Literal(lit) = triple.object {
                    min_card = lit.value().parse().ok();
                }
            } else if triple.predicate == vocab::MAX_CARDINALITY {
                if let TermRef::Literal(lit) = triple.object {
                    max_card = lit.value().parse().ok();
                }
            } else if triple.predicate == vocab::CARDINALITY {
                if let TermRef::Literal(lit) = triple.object {
                    exact_card = lit.value().parse().ok();
                }
            } else if triple.predicate == vocab::ON_CLASS {
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

            // Get rdf:first
            let first = self.graph
                .object_for_subject_predicate(current.as_ref().into(), rdf::FIRST)
                .ok_or_else(|| OwlParseError::malformed_list("Missing rdf:first"))?;
            result.push(self.parse_class_expression(first)?);

            // Get rdf:rest
            current = self.graph
                .object_for_subject_predicate(current.as_ref().into(), rdf::REST)
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

            let first = self.graph
                .object_for_subject_predicate(current.as_ref().into(), rdf::FIRST)
                .ok_or_else(|| OwlParseError::malformed_list("Missing rdf:first"))?;
            result.push(self.term_to_individual(first)?);

            current = self.graph
                .object_for_subject_predicate(current.as_ref().into(), rdf::REST)
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

use oxrdf::NamedOrBlankNodeRef;

/// Parses an ontology from an RDF graph.
pub fn parse_ontology(graph: &Graph) -> Result<Ontology, OwlParseError> {
    OntologyParser::new(graph).parse()
}

/// Parses an ontology with custom configuration.
pub fn parse_ontology_with_config(graph: &Graph, config: ParserConfig) -> Result<Ontology, OwlParseError> {
    OntologyParser::with_config(graph, config).parse()
}
