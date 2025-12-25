//! OWL 2 Ontology - container for axioms and declarations.

use crate::axiom::Axiom;
use crate::entity::{OwlClass, ObjectProperty, DataProperty, AnnotationProperty, Individual};
use crate::expression::ClassExpression;
use oxrdf::NamedNode;
use rustc_hash::FxHashSet;

/// An OWL 2 ontology.
///
/// An ontology is a collection of axioms describing classes, properties,
/// and individuals in a domain.
#[derive(Debug, Clone, Default)]
pub struct Ontology {
    /// The ontology IRI (optional)
    iri: Option<NamedNode>,

    /// The version IRI (optional)
    version_iri: Option<NamedNode>,

    /// Imported ontology IRIs
    imports: Vec<NamedNode>,

    /// All axioms in the ontology
    axioms: Vec<Axiom>,

    /// Declared classes (for quick lookup)
    classes: FxHashSet<OwlClass>,

    /// Declared object properties
    object_properties: FxHashSet<ObjectProperty>,

    /// Declared data properties
    data_properties: FxHashSet<DataProperty>,

    /// Declared annotation properties
    annotation_properties: FxHashSet<AnnotationProperty>,

    /// Declared individuals
    individuals: FxHashSet<Individual>,
}

impl Ontology {
    /// Creates a new empty ontology.
    pub fn new(iri: Option<NamedNode>) -> Self {
        Self {
            iri,
            version_iri: None,
            imports: Vec::new(),
            axioms: Vec::new(),
            classes: FxHashSet::default(),
            object_properties: FxHashSet::default(),
            data_properties: FxHashSet::default(),
            annotation_properties: FxHashSet::default(),
            individuals: FxHashSet::default(),
        }
    }

    /// Creates a new ontology with the given IRI string.
    pub fn with_iri(iri: impl AsRef<str>) -> Result<Self, oxiri::IriParseError> {
        Ok(Self::new(Some(NamedNode::new(iri.as_ref())?)))
    }

    /// Returns the ontology IRI.
    pub fn iri(&self) -> Option<&NamedNode> {
        self.iri.as_ref()
    }

    /// Sets the ontology IRI.
    pub fn set_iri(&mut self, iri: Option<NamedNode>) {
        self.iri = iri;
    }

    /// Returns the version IRI.
    pub fn version_iri(&self) -> Option<&NamedNode> {
        self.version_iri.as_ref()
    }

    /// Sets the version IRI.
    pub fn set_version_iri(&mut self, iri: Option<NamedNode>) {
        self.version_iri = iri;
    }

    /// Returns the imported ontology IRIs.
    pub fn imports(&self) -> &[NamedNode] {
        &self.imports
    }

    /// Adds an import declaration.
    pub fn add_import(&mut self, iri: NamedNode) {
        if !self.imports.contains(&iri) {
            self.imports.push(iri);
        }
    }

    /// Adds an axiom to the ontology.
    pub fn add_axiom(&mut self, axiom: Axiom) {
        // Update declaration indexes
        match &axiom {
            Axiom::DeclareClass(c) => {
                self.classes.insert(c.clone());
            }
            Axiom::DeclareObjectProperty(p) => {
                self.object_properties.insert(p.clone());
            }
            Axiom::DeclareDataProperty(p) => {
                self.data_properties.insert(p.clone());
            }
            Axiom::DeclareAnnotationProperty(p) => {
                self.annotation_properties.insert(p.clone());
            }
            Axiom::DeclareNamedIndividual(i) => {
                self.individuals.insert(i.clone());
            }
            // For class axioms, auto-declare classes mentioned
            Axiom::SubClassOf { sub_class, super_class } => {
                self.declare_classes_in_expression(sub_class);
                self.declare_classes_in_expression(super_class);
            }
            Axiom::EquivalentClasses(classes) | Axiom::DisjointClasses(classes) => {
                for c in classes {
                    self.declare_classes_in_expression(c);
                }
            }
            Axiom::ClassAssertion { class, individual } => {
                self.declare_classes_in_expression(class);
                self.individuals.insert(individual.clone());
            }
            _ => {}
        }
        self.axioms.push(axiom);
    }

    /// Helper to declare classes mentioned in an expression.
    fn declare_classes_in_expression(&mut self, expr: &ClassExpression) {
        if let ClassExpression::Class(c) = expr {
            self.classes.insert(c.clone());
        }
        // Could recursively handle nested expressions if needed
    }

    /// Returns all axioms in the ontology.
    pub fn axioms(&self) -> &[Axiom] {
        &self.axioms
    }

    /// Returns an iterator over axioms.
    pub fn iter_axioms(&self) -> impl Iterator<Item = &Axiom> {
        self.axioms.iter()
    }

    /// Returns the number of axioms.
    pub fn axiom_count(&self) -> usize {
        self.axioms.len()
    }

    /// Returns all declared classes.
    pub fn classes(&self) -> impl Iterator<Item = &OwlClass> {
        self.classes.iter()
    }

    /// Returns all declared object properties.
    pub fn object_properties(&self) -> impl Iterator<Item = &ObjectProperty> {
        self.object_properties.iter()
    }

    /// Returns all declared data properties.
    pub fn data_properties(&self) -> impl Iterator<Item = &DataProperty> {
        self.data_properties.iter()
    }

    /// Returns all declared individuals.
    pub fn individuals(&self) -> impl Iterator<Item = &Individual> {
        self.individuals.iter()
    }

    /// Checks if a class is declared in this ontology.
    pub fn contains_class(&self, class: &OwlClass) -> bool {
        self.classes.contains(class)
    }

    /// Checks if an individual is declared.
    pub fn contains_individual(&self, individual: &Individual) -> bool {
        self.individuals.contains(individual)
    }

    /// Returns all SubClassOf axioms.
    pub fn subclass_axioms(&self) -> impl Iterator<Item = &Axiom> {
        self.axioms.iter().filter(|a| matches!(a, Axiom::SubClassOf { .. }))
    }

    /// Returns all EquivalentClasses axioms.
    pub fn equivalent_class_axioms(&self) -> impl Iterator<Item = &Axiom> {
        self.axioms.iter().filter(|a| matches!(a, Axiom::EquivalentClasses(_)))
    }

    /// Returns all class assertion axioms for a given individual.
    pub fn types_of(&self, individual: &Individual) -> impl Iterator<Item = &ClassExpression> {
        self.axioms.iter().filter_map(move |a| {
            match a {
                Axiom::ClassAssertion { class, individual: i } if i == individual => Some(class),
                _ => None,
            }
        })
    }

    /// Returns direct subclasses of a given class.
    pub fn direct_subclasses_of(&self, class: &OwlClass) -> Vec<&ClassExpression> {
        self.axioms.iter().filter_map(|a| {
            match a {
                Axiom::SubClassOf { sub_class, super_class } => {
                    if let ClassExpression::Class(c) = super_class {
                        if c == class {
                            return Some(sub_class);
                        }
                    }
                    None
                }
                _ => None,
            }
        }).collect()
    }

    /// Returns direct superclasses of a given class.
    pub fn direct_superclasses_of(&self, class: &OwlClass) -> Vec<&ClassExpression> {
        self.axioms.iter().filter_map(|a| {
            match a {
                Axiom::SubClassOf { sub_class, super_class } => {
                    if let ClassExpression::Class(c) = sub_class {
                        if c == class {
                            return Some(super_class);
                        }
                    }
                    None
                }
                _ => None,
            }
        }).collect()
    }

    /// Clears all axioms from the ontology.
    pub fn clear(&mut self) {
        self.axioms.clear();
        self.classes.clear();
        self.object_properties.clear();
        self.data_properties.clear();
        self.annotation_properties.clear();
        self.individuals.clear();
    }

    /// Merges another ontology into this one.
    pub fn merge(&mut self, other: Ontology) {
        for import in other.imports {
            self.add_import(import);
        }
        for axiom in other.axioms {
            self.add_axiom(axiom);
        }
    }
}

impl std::fmt::Display for Ontology {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(iri) = &self.iri {
            write!(f, "Ontology({iri})")?;
        } else {
            write!(f, "Ontology(anonymous)")?;
        }
        write!(f, " [{} axioms]", self.axioms.len())
    }
}
