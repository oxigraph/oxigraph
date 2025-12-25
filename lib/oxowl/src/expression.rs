//! OWL 2 class expressions, object property expressions, and data ranges.

use crate::entity::{OwlClass, ObjectProperty, DataProperty, Individual};
use oxrdf::{NamedNode, Literal};

/// An OWL 2 class expression.
///
/// Class expressions describe sets of individuals through various constructors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassExpression {
    /// A named class (atomic class)
    Class(OwlClass),

    /// ObjectIntersectionOf(C1, ..., Cn) - intersection of classes
    ObjectIntersectionOf(Vec<ClassExpression>),

    /// ObjectUnionOf(C1, ..., Cn) - union of classes
    ObjectUnionOf(Vec<ClassExpression>),

    /// ObjectComplementOf(C) - complement of a class
    ObjectComplementOf(Box<ClassExpression>),

    /// ObjectOneOf(a1, ..., an) - enumeration of individuals
    ObjectOneOf(Vec<Individual>),

    /// ObjectSomeValuesFrom(P, C) - existential restriction
    ObjectSomeValuesFrom {
        property: ObjectPropertyExpression,
        filler: Box<ClassExpression>,
    },

    /// ObjectAllValuesFrom(P, C) - universal restriction
    ObjectAllValuesFrom {
        property: ObjectPropertyExpression,
        filler: Box<ClassExpression>,
    },

    /// ObjectHasValue(P, a) - has-value restriction
    ObjectHasValue {
        property: ObjectPropertyExpression,
        individual: Individual,
    },

    /// ObjectHasSelf(P) - self restriction (reflexive on P)
    ObjectHasSelf(ObjectPropertyExpression),

    /// ObjectMinCardinality(n, P) or ObjectMinCardinality(n, P, C)
    ObjectMinCardinality {
        cardinality: u32,
        property: ObjectPropertyExpression,
        filler: Option<Box<ClassExpression>>,
    },

    /// ObjectMaxCardinality(n, P) or ObjectMaxCardinality(n, P, C)
    ObjectMaxCardinality {
        cardinality: u32,
        property: ObjectPropertyExpression,
        filler: Option<Box<ClassExpression>>,
    },

    /// ObjectExactCardinality(n, P) or ObjectExactCardinality(n, P, C)
    ObjectExactCardinality {
        cardinality: u32,
        property: ObjectPropertyExpression,
        filler: Option<Box<ClassExpression>>,
    },

    /// DataSomeValuesFrom(P, D) - existential data restriction
    DataSomeValuesFrom {
        property: DataProperty,
        filler: DataRange,
    },

    /// DataAllValuesFrom(P, D) - universal data restriction
    DataAllValuesFrom {
        property: DataProperty,
        filler: DataRange,
    },

    /// DataHasValue(P, v) - has-value data restriction
    DataHasValue {
        property: DataProperty,
        value: Literal,
    },

    /// DataMinCardinality(n, P) or DataMinCardinality(n, P, D)
    DataMinCardinality {
        cardinality: u32,
        property: DataProperty,
        filler: Option<DataRange>,
    },

    /// DataMaxCardinality(n, P) or DataMaxCardinality(n, P, D)
    DataMaxCardinality {
        cardinality: u32,
        property: DataProperty,
        filler: Option<DataRange>,
    },

    /// DataExactCardinality(n, P) or DataExactCardinality(n, P, D)
    DataExactCardinality {
        cardinality: u32,
        property: DataProperty,
        filler: Option<DataRange>,
    },
}

impl ClassExpression {
    /// Creates a named class expression.
    pub fn class(c: impl Into<OwlClass>) -> Self {
        Self::Class(c.into())
    }

    /// Creates an intersection of classes.
    pub fn intersection(classes: Vec<ClassExpression>) -> Self {
        Self::ObjectIntersectionOf(classes)
    }

    /// Creates a union of classes.
    pub fn union(classes: Vec<ClassExpression>) -> Self {
        Self::ObjectUnionOf(classes)
    }

    /// Creates the complement of a class expression.
    pub fn complement(c: ClassExpression) -> Self {
        Self::ObjectComplementOf(Box::new(c))
    }

    /// Creates an existential restriction.
    pub fn some_values_from(property: impl Into<ObjectPropertyExpression>, filler: ClassExpression) -> Self {
        Self::ObjectSomeValuesFrom {
            property: property.into(),
            filler: Box::new(filler),
        }
    }

    /// Creates a universal restriction.
    pub fn all_values_from(property: impl Into<ObjectPropertyExpression>, filler: ClassExpression) -> Self {
        Self::ObjectAllValuesFrom {
            property: property.into(),
            filler: Box::new(filler),
        }
    }

    /// Returns true if this is a named class.
    pub fn is_named(&self) -> bool {
        matches!(self, Self::Class(_))
    }

    /// Returns the named class if this is one.
    pub fn as_class(&self) -> Option<&OwlClass> {
        match self {
            Self::Class(c) => Some(c),
            _ => None,
        }
    }
}

impl From<OwlClass> for ClassExpression {
    fn from(c: OwlClass) -> Self {
        Self::Class(c)
    }
}

/// An OWL 2 object property expression.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ObjectPropertyExpression {
    /// A named object property
    ObjectProperty(ObjectProperty),

    /// ObjectInverseOf(P) - inverse of a property
    ObjectInverseOf(Box<ObjectProperty>),
}

impl ObjectPropertyExpression {
    /// Creates an inverse property expression.
    pub fn inverse(property: ObjectProperty) -> Self {
        Self::ObjectInverseOf(Box::new(property))
    }

    /// Returns true if this is a named property.
    pub fn is_named(&self) -> bool {
        matches!(self, Self::ObjectProperty(_))
    }

    /// Returns the named property if this is one.
    pub fn as_property(&self) -> &ObjectProperty {
        match self {
            Self::ObjectProperty(p) => p,
            Self::ObjectInverseOf(p) => p.as_ref(),
        }
    }

    /// Returns the base property (removing inverse if present).
    pub fn base_property(&self) -> &ObjectProperty {
        match self {
            Self::ObjectProperty(p) => p,
            Self::ObjectInverseOf(p) => p.as_ref(),
        }
    }
}

impl From<ObjectProperty> for ObjectPropertyExpression {
    fn from(p: ObjectProperty) -> Self {
        Self::ObjectProperty(p)
    }
}

/// An OWL 2 data range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DataRange {
    /// A named datatype (e.g., xsd:string)
    Datatype(NamedNode),

    /// DataIntersectionOf(D1, ..., Dn)
    DataIntersectionOf(Vec<DataRange>),

    /// DataUnionOf(D1, ..., Dn)
    DataUnionOf(Vec<DataRange>),

    /// DataComplementOf(D)
    DataComplementOf(Box<DataRange>),

    /// DataOneOf(v1, ..., vn) - enumeration of literals
    DataOneOf(Vec<Literal>),

    /// DatatypeRestriction(D, facet1 value1, ...)
    DatatypeRestriction {
        datatype: NamedNode,
        facets: Vec<(NamedNode, Literal)>,
    },
}

impl DataRange {
    /// Creates a datatype data range.
    pub fn datatype(dt: impl Into<NamedNode>) -> Self {
        Self::Datatype(dt.into())
    }

    /// Returns the datatype if this is a simple datatype.
    pub fn as_datatype(&self) -> Option<&NamedNode> {
        match self {
            Self::Datatype(dt) => Some(dt),
            _ => None,
        }
    }
}

impl From<NamedNode> for DataRange {
    fn from(node: NamedNode) -> Self {
        Self::Datatype(node)
    }
}
