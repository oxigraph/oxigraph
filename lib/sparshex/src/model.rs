//! ShEx shape model types.
//!
//! This module defines the core ShEx shape expression types:
//! - [`ShapeLabel`] - Identifier for shapes (IRI or blank node)
//! - [`ShapeExpression`] - Main shape expression type (union of all shape types)
//! - [`TripleConstraint`] - Constraint on triples with predicate and cardinality
//! - [`NodeConstraint`] - Constraints on node values (datatype, pattern, value set, etc.)
//! - [`Cardinality`] - Min/max occurrences for triple constraints
//! - [`ShapesSchema`] - Collection of shapes with namespace handling

use oxrdf::{BlankNode, Literal, NamedNode, NamedOrBlankNode, Term};
use rustc_hash::{FxHashMap, FxHashSet};
use std::sync::Arc;

use crate::error::ShexParseError;

/// Unique identifier for a shape (shape label in ShEx terminology).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShapeLabel {
    /// Named shape (IRI).
    Iri(NamedNode),
    /// Anonymous shape (blank node).
    BNode(BlankNode),
}

impl ShapeLabel {
    /// Creates a shape label from a named or blank node.
    pub fn from_named_or_blank(node: NamedOrBlankNode) -> Self {
        match node {
            NamedOrBlankNode::NamedNode(n) => Self::Iri(n),
            NamedOrBlankNode::BlankNode(b) => Self::BNode(b),
        }
    }

    /// Converts to a Term.
    pub fn to_term(&self) -> Term {
        match self {
            Self::Iri(n) => Term::NamedNode(n.clone()),
            Self::BNode(b) => Term::BlankNode(b.clone()),
        }
    }

    /// Returns the shape label as a named node if it is one.
    pub fn as_iri(&self) -> Option<&NamedNode> {
        match self {
            Self::Iri(n) => Some(n),
            Self::BNode(_) => None,
        }
    }

    /// Returns the shape label as a blank node if it is one.
    pub fn as_bnode(&self) -> Option<&BlankNode> {
        match self {
            Self::Iri(_) => None,
            Self::BNode(b) => Some(b),
        }
    }
}

impl From<NamedNode> for ShapeLabel {
    fn from(n: NamedNode) -> Self {
        Self::Iri(n)
    }
}

impl From<BlankNode> for ShapeLabel {
    fn from(b: BlankNode) -> Self {
        Self::BNode(b)
    }
}

impl std::fmt::Display for ShapeLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Iri(n) => write!(f, "<{}>", n.as_str()),
            Self::BNode(b) => write!(f, "_:{}", b.as_str()),
        }
    }
}

/// Main shape expression type.
///
/// ShEx shapes can be combined and composed using various operators.
/// This enum represents all possible shape expression types.
#[derive(Debug, Clone, PartialEq)]
pub enum ShapeExpression {
    /// Conjunction of shape expressions (AND).
    ShapeAnd(Vec<ShapeExpression>),

    /// Disjunction of shape expressions (OR).
    ShapeOr(Vec<ShapeExpression>),

    /// Negation of a shape expression (NOT).
    ShapeNot(Box<ShapeExpression>),

    /// Node constraint - validates properties of the focus node itself.
    NodeConstraint(NodeConstraint),

    /// Shape with triple constraints - validates triples where focus node is subject.
    Shape(Shape),

    /// Reference to an external shape schema.
    ShapeExternal,

    /// Reference to another shape by label.
    ShapeRef(ShapeLabel),
}

impl ShapeExpression {
    /// Returns true if this is a shape reference.
    pub fn is_ref(&self) -> bool {
        matches!(self, Self::ShapeRef(_))
    }

    /// Returns the shape label if this is a reference.
    pub fn as_ref(&self) -> Option<&ShapeLabel> {
        match self {
            Self::ShapeRef(label) => Some(label),
            _ => None,
        }
    }

    /// Collects all shape references in this expression (recursive).
    pub fn collect_refs(&self) -> Vec<&ShapeLabel> {
        let mut refs = Vec::new();
        self.collect_refs_impl(&mut refs);
        refs
    }

    fn collect_refs_impl<'a>(&'a self, refs: &mut Vec<&'a ShapeLabel>) {
        match self {
            Self::ShapeAnd(shapes) | Self::ShapeOr(shapes) => {
                for shape in shapes {
                    shape.collect_refs_impl(refs);
                }
            }
            Self::ShapeNot(shape) => shape.collect_refs_impl(refs),
            Self::ShapeRef(label) => refs.push(label),
            Self::Shape(shape) => {
                for tc in &shape.triple_constraints {
                    if let Some(value_expr) = &tc.value_expr {
                        value_expr.collect_refs_impl(refs);
                    }
                }
            }
            Self::NodeConstraint(_) | Self::ShapeExternal => {}
        }
    }
}

/// Shape with triple constraints.
///
/// Validates triples where the focus node is the subject, matching
/// against a set of triple constraints with cardinalities.
#[derive(Debug, Clone, PartialEq)]
pub struct Shape {
    /// Optional label for this shape.
    pub label: Option<ShapeLabel>,

    /// Whether this is a closed shape (no extra properties allowed).
    pub closed: bool,

    /// Additional properties allowed in closed shapes.
    pub extra: Vec<NamedNode>,

    /// Triple constraints that must be satisfied.
    pub triple_constraints: Vec<TripleConstraint>,

    /// Annotations for this shape.
    pub annotations: Vec<Annotation>,
}

impl Shape {
    /// Creates a new empty shape.
    pub fn new() -> Self {
        Self {
            label: None,
            closed: false,
            extra: Vec::new(),
            triple_constraints: Vec::new(),
            annotations: Vec::new(),
        }
    }

    /// Creates a new shape with the given label.
    pub fn with_label(label: ShapeLabel) -> Self {
        Self {
            label: Some(label),
            closed: false,
            extra: Vec::new(),
            triple_constraints: Vec::new(),
            annotations: Vec::new(),
        }
    }

    /// Adds a triple constraint to this shape.
    pub fn add_triple_constraint(&mut self, constraint: TripleConstraint) {
        self.triple_constraints.push(constraint);
    }

    /// Returns true if this shape has no constraints.
    pub fn is_empty(&self) -> bool {
        self.triple_constraints.is_empty()
    }
}

impl Default for Shape {
    fn default() -> Self {
        Self::new()
    }
}

/// Constraint on a triple pattern.
///
/// Specifies a predicate and optional value expression that values must match,
/// along with cardinality constraints (min/max occurrences).
#[derive(Debug, Clone, PartialEq)]
pub struct TripleConstraint {
    /// Predicate IRI for this constraint.
    pub predicate: NamedNode,

    /// Optional shape expression that values must satisfy.
    pub value_expr: Option<Box<ShapeExpression>>,

    /// Cardinality constraint (min/max occurrences).
    pub cardinality: Cardinality,

    /// Whether this constraint is inverse (focus node is object).
    pub inverse: bool,

    /// Annotations for this constraint.
    pub annotations: Vec<Annotation>,
}

impl TripleConstraint {
    /// Creates a new triple constraint with the given predicate.
    pub fn new(predicate: NamedNode) -> Self {
        Self {
            predicate,
            value_expr: None,
            cardinality: Cardinality::default(),
            inverse: false,
            annotations: Vec::new(),
        }
    }

    /// Creates a new triple constraint with predicate and value expression.
    pub fn with_value_expr(predicate: NamedNode, value_expr: ShapeExpression) -> Self {
        Self {
            predicate,
            value_expr: Some(Box::new(value_expr)),
            cardinality: Cardinality::default(),
            inverse: false,
            annotations: Vec::new(),
        }
    }

    /// Sets the cardinality for this constraint.
    pub fn with_cardinality(mut self, cardinality: Cardinality) -> Self {
        self.cardinality = cardinality;
        self
    }

    /// Sets whether this constraint is inverse.
    pub fn with_inverse(mut self, inverse: bool) -> Self {
        self.inverse = inverse;
        self
    }
}

/// Cardinality constraint (min/max occurrences).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cardinality {
    /// Minimum number of occurrences (default: 1).
    pub min: u32,

    /// Maximum number of occurrences (None = unbounded, default: 1).
    pub max: Option<u32>,
}

impl Cardinality {
    /// Creates a cardinality constraint with min and max.
    pub fn new(min: u32, max: Option<u32>) -> Result<Self, ShexParseError> {
        if let Some(max_val) = max {
            if max_val < min {
                return Err(ShexParseError::invalid_cardinality(
                    Term::Literal(Literal::new_simple_literal("cardinality")),
                    min,
                    max,
                ));
            }
        }
        Ok(Self { min, max })
    }

    /// Creates a cardinality constraint for exactly n occurrences.
    pub fn exactly(n: u32) -> Self {
        Self {
            min: n,
            max: Some(n),
        }
    }

    /// Creates a cardinality constraint for 0 or 1 occurrence.
    pub fn optional() -> Self {
        Self {
            min: 0,
            max: Some(1),
        }
    }

    /// Creates a cardinality constraint for 0 or more occurrences (*).
    pub fn zero_or_more() -> Self {
        Self { min: 0, max: None }
    }

    /// Creates a cardinality constraint for 1 or more occurrences (+).
    pub fn one_or_more() -> Self {
        Self { min: 1, max: None }
    }

    /// Returns true if this cardinality allows the given count.
    pub fn allows(&self, count: u32) -> bool {
        count >= self.min && self.max.map_or(true, |max| count <= max)
    }

    /// Returns true if this is the default cardinality (exactly 1).
    pub fn is_default(&self) -> bool {
        self.min == 1 && self.max == Some(1)
    }
}

impl Default for Cardinality {
    fn default() -> Self {
        Self::exactly(1)
    }
}

impl std::fmt::Display for Cardinality {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.min, self.max) {
            (0, None) => write!(f, "*"),
            (1, None) => write!(f, "+"),
            (0, Some(1)) => write!(f, "?"),
            (min, None) => write!(f, "{{{},}}", min),
            (min, Some(max)) if min == max => write!(f, "{{{}}}", min),
            (min, Some(max)) => write!(f, "{{{},{}}}", min, max),
        }
    }
}

/// Node constraint - validates properties of nodes.
///
/// Can constrain node kind, datatype, string facets (length, pattern),
/// numeric facets (min/max), and value sets.
#[derive(Debug, Clone, PartialEq)]
pub struct NodeConstraint {
    /// Required node kind (IRI, BlankNode, Literal, etc.).
    pub node_kind: Option<NodeKind>,

    /// Required datatype for literals.
    pub datatype: Option<NamedNode>,

    /// String facets (length, pattern).
    pub string_facets: Vec<StringFacet>,

    /// Numeric facets (min/max values).
    pub numeric_facets: Vec<NumericFacet>,

    /// Value set constraint.
    pub values: Vec<ValueSetValue>,
}

impl NodeConstraint {
    /// Creates a new empty node constraint.
    pub fn new() -> Self {
        Self {
            node_kind: None,
            datatype: None,
            string_facets: Vec::new(),
            numeric_facets: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Creates a node constraint with the given node kind.
    pub fn with_node_kind(node_kind: NodeKind) -> Self {
        Self {
            node_kind: Some(node_kind),
            datatype: None,
            string_facets: Vec::new(),
            numeric_facets: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Creates a node constraint with the given datatype.
    pub fn with_datatype(datatype: NamedNode) -> Self {
        Self {
            node_kind: None,
            datatype: Some(datatype),
            string_facets: Vec::new(),
            numeric_facets: Vec::new(),
            values: Vec::new(),
        }
    }

    /// Adds a value to the value set.
    pub fn add_value(&mut self, value: ValueSetValue) {
        self.values.push(value);
    }

    /// Returns true if this constraint is empty.
    pub fn is_empty(&self) -> bool {
        self.node_kind.is_none()
            && self.datatype.is_none()
            && self.string_facets.is_empty()
            && self.numeric_facets.is_empty()
            && self.values.is_empty()
    }
}

impl Default for NodeConstraint {
    fn default() -> Self {
        Self::new()
    }
}

/// Node kind constraint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NodeKind {
    /// IRI node.
    Iri,
    /// Blank node.
    BNode,
    /// Literal value.
    Literal,
    /// Non-literal (IRI or blank node).
    NonLiteral,
}

impl NodeKind {
    /// Returns true if the given term matches this node kind.
    pub fn matches(&self, term: &Term) -> bool {
        match self {
            Self::Iri => matches!(term, Term::NamedNode(_)),
            Self::BNode => matches!(term, Term::BlankNode(_)),
            Self::Literal => matches!(term, Term::Literal(_)),
            Self::NonLiteral => matches!(term, Term::NamedNode(_) | Term::BlankNode(_)),
        }
    }
}

impl std::fmt::Display for NodeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Iri => write!(f, "IRI"),
            Self::BNode => write!(f, "BNODE"),
            Self::Literal => write!(f, "LITERAL"),
            Self::NonLiteral => write!(f, "NONLITERAL"),
        }
    }
}

/// String facet constraint (length, pattern).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StringFacet {
    /// Minimum string length.
    MinLength(usize),
    /// Maximum string length.
    MaxLength(usize),
    /// Regular expression pattern.
    Pattern {
        /// Regex pattern.
        pattern: String,
        /// Optional regex flags.
        flags: Option<String>,
    },
}

/// Numeric facet constraint (min/max values).
#[derive(Debug, Clone, PartialEq)]
pub enum NumericFacet {
    /// Minimum inclusive value.
    MinInclusive(NumericLiteral),
    /// Minimum exclusive value.
    MinExclusive(NumericLiteral),
    /// Maximum inclusive value.
    MaxInclusive(NumericLiteral),
    /// Maximum exclusive value.
    MaxExclusive(NumericLiteral),
    /// Total number of digits.
    TotalDigits(u32),
    /// Number of fractional digits.
    FractionDigits(u32),
}

/// Numeric literal value for comparisons.
#[derive(Debug, Clone, PartialEq)]
pub struct NumericLiteral {
    /// The literal value.
    pub value: Literal,
}

impl NumericLiteral {
    /// Creates a new numeric literal.
    pub fn new(value: Literal) -> Self {
        Self { value }
    }
}

/// Value in a value set constraint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueSetValue {
    /// Exact RDF term.
    ObjectValue(Term),

    /// IRI stem (prefix match).
    IriStem(String),

    /// IRI stem exclusion.
    IriStemRange {
        /// Base stem to match.
        stem: String,
        /// Values to exclude.
        exclusions: Vec<ValueSetValue>,
    },

    /// Literal stem (lexical form prefix match).
    LiteralStem(String),

    /// Literal stem exclusion.
    LiteralStemRange {
        /// Base stem to match.
        stem: String,
        /// Values to exclude.
        exclusions: Vec<ValueSetValue>,
    },

    /// Language stem (language tag prefix match).
    LanguageStem(String),

    /// Language stem exclusion.
    LanguageStemRange {
        /// Base stem to match.
        stem: String,
        /// Values to exclude.
        exclusions: Vec<ValueSetValue>,
    },
}

impl ValueSetValue {
    /// Creates an IRI stem value.
    pub fn iri_stem(stem: impl Into<String>) -> Self {
        Self::IriStem(stem.into())
    }

    /// Creates a literal stem value.
    pub fn literal_stem(stem: impl Into<String>) -> Self {
        Self::LiteralStem(stem.into())
    }

    /// Creates a language stem value.
    pub fn language_stem(stem: impl Into<String>) -> Self {
        Self::LanguageStem(stem.into())
    }
}

/// Annotation on shapes or triple constraints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Annotation {
    /// Annotation predicate.
    pub predicate: NamedNode,
    /// Annotation value.
    pub object: Term,
}

impl Annotation {
    /// Creates a new annotation.
    pub fn new(predicate: NamedNode, object: Term) -> Self {
        Self { predicate, object }
    }
}

/// Collection of shapes (shapes schema in ShEx terminology).
///
/// Contains shape definitions indexed by label, with support for
/// imports, namespace declarations, and start shape.
#[derive(Debug, Clone)]
pub struct ShapesSchema {
    /// Shape expressions indexed by label.
    shapes: FxHashMap<ShapeLabel, Arc<ShapeExpression>>,

    /// Optional start shape (default entry point for validation).
    start: Option<ShapeLabel>,

    /// Imported schemas.
    imports: Vec<NamedNode>,

    /// All shape labels (for iteration).
    all_labels: Vec<ShapeLabel>,
}

impl ShapesSchema {
    /// Creates a new empty shapes schema.
    pub fn new() -> Self {
        Self {
            shapes: FxHashMap::default(),
            start: None,
            imports: Vec::new(),
            all_labels: Vec::new(),
        }
    }

    /// Adds a shape expression with the given label.
    pub fn add_shape(&mut self, label: ShapeLabel, expr: ShapeExpression) {
        if !self.shapes.contains_key(&label) {
            self.all_labels.push(label.clone());
        }
        self.shapes.insert(label, Arc::new(expr));
    }

    /// Gets a shape expression by label.
    pub fn get_shape(&self, label: &ShapeLabel) -> Option<&Arc<ShapeExpression>> {
        self.shapes.get(label)
    }

    /// Returns an iterator over all shape labels.
    pub fn labels(&self) -> impl Iterator<Item = &ShapeLabel> {
        self.all_labels.iter()
    }

    /// Returns an iterator over all shapes.
    pub fn shapes(&self) -> impl Iterator<Item = (&ShapeLabel, &Arc<ShapeExpression>)> {
        self.shapes.iter()
    }

    /// Sets the start shape.
    pub fn set_start(&mut self, label: ShapeLabel) {
        self.start = Some(label);
    }

    /// Gets the start shape label.
    pub fn start(&self) -> Option<&ShapeLabel> {
        self.start.as_ref()
    }

    /// Adds an import.
    pub fn add_import(&mut self, import: NamedNode) {
        self.imports.push(import);
    }

    /// Returns an iterator over imports.
    pub fn imports(&self) -> impl Iterator<Item = &NamedNode> {
        self.imports.iter()
    }

    /// Returns true if the schema is empty.
    pub fn is_empty(&self) -> bool {
        self.shapes.is_empty()
    }

    /// Returns the number of shapes.
    pub fn len(&self) -> usize {
        self.shapes.len()
    }

    /// Validates that all shape references are defined.
    ///
    /// Returns an error if any referenced shape is not in the schema.
    pub fn validate_refs(&self) -> Result<(), ShexParseError> {
        for (_label, expr) in &self.shapes {
            let refs = expr.collect_refs();
            for ref_label in refs {
                if !self.shapes.contains_key(ref_label) {
                    return Err(ShexParseError::undefined_shape_ref(ref_label.to_string()));
                }
            }
        }
        Ok(())
    }

    /// Detects cycles in shape references.
    ///
    /// Returns an error if a cyclic reference is detected.
    pub fn detect_cycles(&self) -> Result<(), ShexParseError> {
        let mut visited = FxHashSet::default();
        let mut rec_stack = FxHashSet::default();

        for label in &self.all_labels {
            if !visited.contains(label) {
                self.detect_cycles_impl(label, &mut visited, &mut rec_stack)?;
            }
        }

        Ok(())
    }

    fn detect_cycles_impl(
        &self,
        label: &ShapeLabel,
        visited: &mut FxHashSet<ShapeLabel>,
        rec_stack: &mut FxHashSet<ShapeLabel>,
    ) -> Result<(), ShexParseError> {
        visited.insert(label.clone());
        rec_stack.insert(label.clone());

        if let Some(expr) = self.shapes.get(label) {
            for ref_label in expr.collect_refs() {
                if !visited.contains(ref_label) {
                    self.detect_cycles_impl(ref_label, visited, rec_stack)?;
                } else if rec_stack.contains(ref_label) {
                    return Err(ShexParseError::cyclic_reference(format!(
                        "Cycle detected: {} -> {}",
                        label, ref_label
                    )));
                }
            }
        }

        rec_stack.remove(label);
        Ok(())
    }
}

impl Default for ShapesSchema {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cardinality_default() {
        let card = Cardinality::default();
        assert_eq!(card.min, 1);
        assert_eq!(card.max, Some(1));
        assert!(card.is_default());
    }

    #[test]
    fn test_cardinality_optional() {
        let card = Cardinality::optional();
        assert_eq!(card.min, 0);
        assert_eq!(card.max, Some(1));
        assert!(card.allows(0));
        assert!(card.allows(1));
        assert!(!card.allows(2));
    }

    #[test]
    fn test_cardinality_star() {
        let card = Cardinality::zero_or_more();
        assert_eq!(card.min, 0);
        assert_eq!(card.max, None);
        assert!(card.allows(0));
        assert!(card.allows(100));
    }

    #[test]
    fn test_cardinality_plus() {
        let card = Cardinality::one_or_more();
        assert_eq!(card.min, 1);
        assert_eq!(card.max, None);
        assert!(!card.allows(0));
        assert!(card.allows(1));
        assert!(card.allows(100));
    }

    #[test]
    fn test_shape_label_display() {
        let iri = ShapeLabel::Iri(NamedNode::new("http://example.org/PersonShape").unwrap());
        assert_eq!(iri.to_string(), "<http://example.org/PersonShape>");
    }

    #[test]
    fn test_shapes_schema_add_get() {
        let mut schema = ShapesSchema::new();
        let label = ShapeLabel::Iri(NamedNode::new("http://example.org/PersonShape").unwrap());
        let expr = ShapeExpression::NodeConstraint(NodeConstraint::new());

        schema.add_shape(label.clone(), expr);
        assert_eq!(schema.len(), 1);
        assert!(schema.get_shape(&label).is_some());
    }

    #[test]
    fn test_shapes_schema_validate_refs_ok() {
        let mut schema = ShapesSchema::new();
        let label1 = ShapeLabel::Iri(NamedNode::new("http://example.org/Shape1").unwrap());
        let label2 = ShapeLabel::Iri(NamedNode::new("http://example.org/Shape2").unwrap());

        schema.add_shape(label1.clone(), ShapeExpression::NodeConstraint(NodeConstraint::new()));
        schema.add_shape(label2.clone(), ShapeExpression::ShapeRef(label1.clone()));

        assert!(schema.validate_refs().is_ok());
    }

    #[test]
    fn test_shapes_schema_validate_refs_undefined() {
        let mut schema = ShapesSchema::new();
        let label1 = ShapeLabel::Iri(NamedNode::new("http://example.org/Shape1").unwrap());
        let label2 = ShapeLabel::Iri(NamedNode::new("http://example.org/Shape2").unwrap());

        // Add reference to undefined shape
        schema.add_shape(label1, ShapeExpression::ShapeRef(label2));

        assert!(schema.validate_refs().is_err());
    }

    #[test]
    fn test_node_kind_matches() {
        let iri = Term::NamedNode(NamedNode::new("http://example.org/").unwrap());
        let bnode = Term::BlankNode(BlankNode::default());
        let literal = Term::Literal(Literal::new_simple_literal("test"));

        assert!(NodeKind::Iri.matches(&iri));
        assert!(!NodeKind::Iri.matches(&bnode));
        assert!(!NodeKind::Iri.matches(&literal));

        assert!(NodeKind::BNode.matches(&bnode));
        assert!(!NodeKind::BNode.matches(&iri));

        assert!(NodeKind::Literal.matches(&literal));
        assert!(!NodeKind::Literal.matches(&iri));

        assert!(NodeKind::NonLiteral.matches(&iri));
        assert!(NodeKind::NonLiteral.matches(&bnode));
        assert!(!NodeKind::NonLiteral.matches(&literal));
    }
}
