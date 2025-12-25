//! OWL 2 axiom types.
//!
//! Axioms are the basic statements in an OWL 2 ontology.

use crate::entity::{OwlClass, ObjectProperty, DataProperty, AnnotationProperty, Individual};
use crate::expression::{ClassExpression, ObjectPropertyExpression, DataRange};
use oxrdf::Literal;

/// An OWL 2 axiom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Axiom {
    // === Class Axioms ===

    /// SubClassOf(sub, super) - sub is a subclass of super
    SubClassOf {
        sub_class: ClassExpression,
        super_class: ClassExpression,
    },

    /// EquivalentClasses(C1, C2, ...) - all classes are equivalent
    EquivalentClasses(Vec<ClassExpression>),

    /// DisjointClasses(C1, C2, ...) - classes have no common instances
    DisjointClasses(Vec<ClassExpression>),

    /// DisjointUnion(C, C1, ..., Cn) - C is the disjoint union of C1...Cn
    DisjointUnion {
        class: OwlClass,
        disjoint_classes: Vec<ClassExpression>,
    },

    // === Object Property Axioms ===

    /// SubObjectPropertyOf(sub, super)
    SubObjectPropertyOf {
        sub_property: ObjectPropertyExpression,
        super_property: ObjectPropertyExpression,
    },

    /// SubObjectPropertyOf(PropertyChain(P1...Pn), P) - property chain axiom
    SubPropertyChainOf {
        property_chain: Vec<ObjectPropertyExpression>,
        super_property: ObjectProperty,
    },

    /// EquivalentObjectProperties(P1, P2, ...)
    EquivalentObjectProperties(Vec<ObjectProperty>),

    /// DisjointObjectProperties(P1, P2, ...)
    DisjointObjectProperties(Vec<ObjectProperty>),

    /// ObjectPropertyDomain(P, C) - domain of P is C
    ObjectPropertyDomain {
        property: ObjectProperty,
        domain: ClassExpression,
    },

    /// ObjectPropertyRange(P, C) - range of P is C
    ObjectPropertyRange {
        property: ObjectProperty,
        range: ClassExpression,
    },

    /// InverseObjectProperties(P1, P2)
    InverseObjectProperties(ObjectProperty, ObjectProperty),

    /// FunctionalObjectProperty(P)
    FunctionalObjectProperty(ObjectProperty),

    /// InverseFunctionalObjectProperty(P)
    InverseFunctionalObjectProperty(ObjectProperty),

    /// ReflexiveObjectProperty(P)
    ReflexiveObjectProperty(ObjectProperty),

    /// IrreflexiveObjectProperty(P)
    IrreflexiveObjectProperty(ObjectProperty),

    /// SymmetricObjectProperty(P)
    SymmetricObjectProperty(ObjectProperty),

    /// AsymmetricObjectProperty(P)
    AsymmetricObjectProperty(ObjectProperty),

    /// TransitiveObjectProperty(P)
    TransitiveObjectProperty(ObjectProperty),

    // === Data Property Axioms ===

    /// SubDataPropertyOf(sub, super)
    SubDataPropertyOf {
        sub_property: DataProperty,
        super_property: DataProperty,
    },

    /// EquivalentDataProperties(P1, P2, ...)
    EquivalentDataProperties(Vec<DataProperty>),

    /// DisjointDataProperties(P1, P2, ...)
    DisjointDataProperties(Vec<DataProperty>),

    /// DataPropertyDomain(P, C)
    DataPropertyDomain {
        property: DataProperty,
        domain: ClassExpression,
    },

    /// DataPropertyRange(P, D)
    DataPropertyRange {
        property: DataProperty,
        range: DataRange,
    },

    /// FunctionalDataProperty(P)
    FunctionalDataProperty(DataProperty),

    // === Individual Axioms (Assertions) ===

    /// ClassAssertion(C, a) - a is an instance of C
    ClassAssertion {
        class: ClassExpression,
        individual: Individual,
    },

    /// ObjectPropertyAssertion(P, a, b) - (a, b) is in P
    ObjectPropertyAssertion {
        property: ObjectProperty,
        source: Individual,
        target: Individual,
    },

    /// NegativeObjectPropertyAssertion(P, a, b) - (a, b) is NOT in P
    NegativeObjectPropertyAssertion {
        property: ObjectProperty,
        source: Individual,
        target: Individual,
    },

    /// DataPropertyAssertion(P, a, v) - (a, v) is in P
    DataPropertyAssertion {
        property: DataProperty,
        source: Individual,
        target: Literal,
    },

    /// NegativeDataPropertyAssertion(P, a, v)
    NegativeDataPropertyAssertion {
        property: DataProperty,
        source: Individual,
        target: Literal,
    },

    /// SameIndividual(a1, a2, ...)
    SameIndividual(Vec<Individual>),

    /// DifferentIndividuals(a1, a2, ...)
    DifferentIndividuals(Vec<Individual>),

    // === Keys ===

    /// HasKey(C, (P1...Pm), (D1...Dn))
    HasKey {
        class: ClassExpression,
        object_properties: Vec<ObjectProperty>,
        data_properties: Vec<DataProperty>,
    },

    // === Declaration Axioms ===

    /// Declaration(Class(C))
    DeclareClass(OwlClass),

    /// Declaration(ObjectProperty(P))
    DeclareObjectProperty(ObjectProperty),

    /// Declaration(DataProperty(P))
    DeclareDataProperty(DataProperty),

    /// Declaration(AnnotationProperty(P))
    DeclareAnnotationProperty(AnnotationProperty),

    /// Declaration(NamedIndividual(a))
    DeclareNamedIndividual(Individual),
}

impl Axiom {
    /// Creates a SubClassOf axiom.
    pub fn subclass_of(sub: impl Into<ClassExpression>, sup: impl Into<ClassExpression>) -> Self {
        Self::SubClassOf {
            sub_class: sub.into(),
            super_class: sup.into(),
        }
    }

    /// Creates a ClassAssertion axiom.
    pub fn class_assertion(class: impl Into<ClassExpression>, individual: impl Into<Individual>) -> Self {
        Self::ClassAssertion {
            class: class.into(),
            individual: individual.into(),
        }
    }

    /// Creates an EquivalentClasses axiom.
    pub fn equivalent_classes(classes: Vec<ClassExpression>) -> Self {
        Self::EquivalentClasses(classes)
    }

    /// Creates a DisjointClasses axiom.
    pub fn disjoint_classes(classes: Vec<ClassExpression>) -> Self {
        Self::DisjointClasses(classes)
    }

    // Add more constructor methods as needed...
}
