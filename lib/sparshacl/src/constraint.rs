//! SHACL constraint definitions.
//!
//! This module defines all SHACL Core constraint components.

use oxrdf::{Literal, NamedNode, NamedNodeRef, Term};

use crate::model::ShapeId;

/// Represents a SHACL constraint.
#[derive(Debug, Clone, PartialEq)]
pub enum Constraint {
    // === Value Type Constraints ===
    /// sh:class - Value must be an instance of the specified class.
    Class(NamedNode),

    /// sh:datatype - Value must have the specified datatype.
    Datatype(NamedNode),

    /// sh:nodeKind - Value must be of the specified node kind.
    NodeKind(NamedNode),

    // === Cardinality Constraints ===
    /// sh:minCount - Minimum number of values required.
    MinCount(usize),

    /// sh:maxCount - Maximum number of values allowed.
    MaxCount(usize),

    // === Value Range Constraints ===
    /// sh:minExclusive - Value must be greater than the specified value.
    MinExclusive(Literal),

    /// sh:maxExclusive - Value must be less than the specified value.
    MaxExclusive(Literal),

    /// sh:minInclusive - Value must be greater than or equal to the specified value.
    MinInclusive(Literal),

    /// sh:maxInclusive - Value must be less than or equal to the specified value.
    MaxInclusive(Literal),

    // === String Constraints ===
    /// sh:minLength - Minimum string length.
    MinLength(usize),

    /// sh:maxLength - Maximum string length.
    MaxLength(usize),

    /// sh:pattern - Value must match the regular expression.
    Pattern {
        pattern: String,
        flags: Option<String>,
    },

    /// sh:languageIn - Language tag must be one of the specified values.
    LanguageIn(Vec<String>),

    /// sh:uniqueLang - No duplicate language tags allowed.
    UniqueLang,

    // === Property Pair Constraints ===
    /// sh:equals - Values must equal values of the specified property.
    Equals(NamedNode),

    /// sh:disjoint - Values must not overlap with values of the specified property.
    Disjoint(NamedNode),

    /// sh:lessThan - Values must be less than values of the specified property.
    LessThan(NamedNode),

    /// sh:lessThanOrEquals - Values must be less than or equal to values of the specified property.
    LessThanOrEquals(NamedNode),

    // === Logical Constraints ===
    /// sh:not - Value must NOT conform to the specified shape.
    Not(ShapeId),

    /// sh:and - Value must conform to ALL specified shapes.
    And(Vec<ShapeId>),

    /// sh:or - Value must conform to AT LEAST ONE specified shape.
    Or(Vec<ShapeId>),

    /// sh:xone - Value must conform to EXACTLY ONE specified shape.
    Xone(Vec<ShapeId>),

    // === Shape-based Constraints ===
    /// sh:node - Value must conform to the specified shape.
    Node(ShapeId),

    /// sh:qualifiedValueShape - Qualified cardinality constraint.
    QualifiedValueShape {
        shape: ShapeId,
        min_count: Option<usize>,
        max_count: Option<usize>,
        disjoint: bool,
    },

    // === Other Constraints ===
    /// sh:closed - Only specified properties are allowed.
    Closed {
        ignored_properties: Vec<NamedNode>,
    },

    /// sh:hasValue - At least one value must equal the specified value.
    HasValue(Term),

    /// sh:in - Value must be one of the specified values.
    In(Vec<Term>),
}

impl Constraint {
    /// Returns the constraint component IRI for this constraint.
    pub fn component(&self) -> NamedNodeRef<'_> {
        use oxrdf::vocab::shacl;
        match self {
            Self::Class(_) => shacl::CLASS_CONSTRAINT_COMPONENT,
            Self::Datatype(_) => shacl::DATATYPE_CONSTRAINT_COMPONENT,
            Self::NodeKind(_) => shacl::NODE_KIND_CONSTRAINT_COMPONENT,
            Self::MinCount(_) => shacl::MIN_COUNT_CONSTRAINT_COMPONENT,
            Self::MaxCount(_) => shacl::MAX_COUNT_CONSTRAINT_COMPONENT,
            Self::MinExclusive(_) => shacl::MIN_EXCLUSIVE_CONSTRAINT_COMPONENT,
            Self::MaxExclusive(_) => shacl::MAX_EXCLUSIVE_CONSTRAINT_COMPONENT,
            Self::MinInclusive(_) => shacl::MIN_INCLUSIVE_CONSTRAINT_COMPONENT,
            Self::MaxInclusive(_) => shacl::MAX_INCLUSIVE_CONSTRAINT_COMPONENT,
            Self::MinLength(_) => shacl::MIN_LENGTH_CONSTRAINT_COMPONENT,
            Self::MaxLength(_) => shacl::MAX_LENGTH_CONSTRAINT_COMPONENT,
            Self::Pattern { .. } => shacl::PATTERN_CONSTRAINT_COMPONENT,
            Self::LanguageIn(_) => shacl::LANGUAGE_IN_CONSTRAINT_COMPONENT,
            Self::UniqueLang => shacl::UNIQUE_LANG_CONSTRAINT_COMPONENT,
            Self::Equals(_) => shacl::EQUALS_CONSTRAINT_COMPONENT,
            Self::Disjoint(_) => shacl::DISJOINT_CONSTRAINT_COMPONENT,
            Self::LessThan(_) => shacl::LESS_THAN_CONSTRAINT_COMPONENT,
            Self::LessThanOrEquals(_) => shacl::LESS_THAN_OR_EQUALS_CONSTRAINT_COMPONENT,
            Self::Not(_) => shacl::NOT_CONSTRAINT_COMPONENT,
            Self::And(_) => shacl::AND_CONSTRAINT_COMPONENT,
            Self::Or(_) => shacl::OR_CONSTRAINT_COMPONENT,
            Self::Xone(_) => shacl::XONE_CONSTRAINT_COMPONENT,
            Self::Node(_) => shacl::NODE_CONSTRAINT_COMPONENT,
            Self::QualifiedValueShape { .. } => shacl::QUALIFIED_VALUE_SHAPE_CONSTRAINT_COMPONENT,
            Self::Closed { .. } => shacl::CLOSED_CONSTRAINT_COMPONENT,
            Self::HasValue(_) => shacl::HAS_VALUE_CONSTRAINT_COMPONENT,
            Self::In(_) => shacl::IN_CONSTRAINT_COMPONENT,
        }
    }

    /// Returns a human-readable name for this constraint type.
    pub fn name(&self) -> &'static str {
        match self {
            Self::Class(_) => "class",
            Self::Datatype(_) => "datatype",
            Self::NodeKind(_) => "nodeKind",
            Self::MinCount(_) => "minCount",
            Self::MaxCount(_) => "maxCount",
            Self::MinExclusive(_) => "minExclusive",
            Self::MaxExclusive(_) => "maxExclusive",
            Self::MinInclusive(_) => "minInclusive",
            Self::MaxInclusive(_) => "maxInclusive",
            Self::MinLength(_) => "minLength",
            Self::MaxLength(_) => "maxLength",
            Self::Pattern { .. } => "pattern",
            Self::LanguageIn(_) => "languageIn",
            Self::UniqueLang => "uniqueLang",
            Self::Equals(_) => "equals",
            Self::Disjoint(_) => "disjoint",
            Self::LessThan(_) => "lessThan",
            Self::LessThanOrEquals(_) => "lessThanOrEquals",
            Self::Not(_) => "not",
            Self::And(_) => "and",
            Self::Or(_) => "or",
            Self::Xone(_) => "xone",
            Self::Node(_) => "node",
            Self::QualifiedValueShape { .. } => "qualifiedValueShape",
            Self::Closed { .. } => "closed",
            Self::HasValue(_) => "hasValue",
            Self::In(_) => "in",
        }
    }
}

/// Represents a constraint component (the type of constraint).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConstraintComponent {
    Class,
    Datatype,
    NodeKind,
    MinCount,
    MaxCount,
    MinExclusive,
    MaxExclusive,
    MinInclusive,
    MaxInclusive,
    MinLength,
    MaxLength,
    Pattern,
    LanguageIn,
    UniqueLang,
    Equals,
    Disjoint,
    LessThan,
    LessThanOrEquals,
    Not,
    And,
    Or,
    Xone,
    Node,
    QualifiedValueShape,
    Closed,
    HasValue,
    In,
}

impl ConstraintComponent {
    /// Returns the IRI for this constraint component.
    pub fn iri(&self) -> NamedNodeRef<'_> {
        use oxrdf::vocab::shacl;
        match self {
            Self::Class => shacl::CLASS_CONSTRAINT_COMPONENT,
            Self::Datatype => shacl::DATATYPE_CONSTRAINT_COMPONENT,
            Self::NodeKind => shacl::NODE_KIND_CONSTRAINT_COMPONENT,
            Self::MinCount => shacl::MIN_COUNT_CONSTRAINT_COMPONENT,
            Self::MaxCount => shacl::MAX_COUNT_CONSTRAINT_COMPONENT,
            Self::MinExclusive => shacl::MIN_EXCLUSIVE_CONSTRAINT_COMPONENT,
            Self::MaxExclusive => shacl::MAX_EXCLUSIVE_CONSTRAINT_COMPONENT,
            Self::MinInclusive => shacl::MIN_INCLUSIVE_CONSTRAINT_COMPONENT,
            Self::MaxInclusive => shacl::MAX_INCLUSIVE_CONSTRAINT_COMPONENT,
            Self::MinLength => shacl::MIN_LENGTH_CONSTRAINT_COMPONENT,
            Self::MaxLength => shacl::MAX_LENGTH_CONSTRAINT_COMPONENT,
            Self::Pattern => shacl::PATTERN_CONSTRAINT_COMPONENT,
            Self::LanguageIn => shacl::LANGUAGE_IN_CONSTRAINT_COMPONENT,
            Self::UniqueLang => shacl::UNIQUE_LANG_CONSTRAINT_COMPONENT,
            Self::Equals => shacl::EQUALS_CONSTRAINT_COMPONENT,
            Self::Disjoint => shacl::DISJOINT_CONSTRAINT_COMPONENT,
            Self::LessThan => shacl::LESS_THAN_CONSTRAINT_COMPONENT,
            Self::LessThanOrEquals => shacl::LESS_THAN_OR_EQUALS_CONSTRAINT_COMPONENT,
            Self::Not => shacl::NOT_CONSTRAINT_COMPONENT,
            Self::And => shacl::AND_CONSTRAINT_COMPONENT,
            Self::Or => shacl::OR_CONSTRAINT_COMPONENT,
            Self::Xone => shacl::XONE_CONSTRAINT_COMPONENT,
            Self::Node => shacl::NODE_CONSTRAINT_COMPONENT,
            Self::QualifiedValueShape => shacl::QUALIFIED_VALUE_SHAPE_CONSTRAINT_COMPONENT,
            Self::Closed => shacl::CLOSED_CONSTRAINT_COMPONENT,
            Self::HasValue => shacl::HAS_VALUE_CONSTRAINT_COMPONENT,
            Self::In => shacl::IN_CONSTRAINT_COMPONENT,
        }
    }
}

impl From<&Constraint> for ConstraintComponent {
    fn from(constraint: &Constraint) -> Self {
        match constraint {
            Constraint::Class(_) => Self::Class,
            Constraint::Datatype(_) => Self::Datatype,
            Constraint::NodeKind(_) => Self::NodeKind,
            Constraint::MinCount(_) => Self::MinCount,
            Constraint::MaxCount(_) => Self::MaxCount,
            Constraint::MinExclusive(_) => Self::MinExclusive,
            Constraint::MaxExclusive(_) => Self::MaxExclusive,
            Constraint::MinInclusive(_) => Self::MinInclusive,
            Constraint::MaxInclusive(_) => Self::MaxInclusive,
            Constraint::MinLength(_) => Self::MinLength,
            Constraint::MaxLength(_) => Self::MaxLength,
            Constraint::Pattern { .. } => Self::Pattern,
            Constraint::LanguageIn(_) => Self::LanguageIn,
            Constraint::UniqueLang => Self::UniqueLang,
            Constraint::Equals(_) => Self::Equals,
            Constraint::Disjoint(_) => Self::Disjoint,
            Constraint::LessThan(_) => Self::LessThan,
            Constraint::LessThanOrEquals(_) => Self::LessThanOrEquals,
            Constraint::Not(_) => Self::Not,
            Constraint::And(_) => Self::And,
            Constraint::Or(_) => Self::Or,
            Constraint::Xone(_) => Self::Xone,
            Constraint::Node(_) => Self::Node,
            Constraint::QualifiedValueShape { .. } => Self::QualifiedValueShape,
            Constraint::Closed { .. } => Self::Closed,
            Constraint::HasValue(_) => Self::HasValue,
            Constraint::In(_) => Self::In,
        }
    }
}
