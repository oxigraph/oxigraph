//! SHACL shape model types.
//!
//! This module defines the core SHACL shape types:
//! - [`ShapeId`] - Identifier for shapes (IRI or blank node)
//! - [`Shape`] - Base shape type
//! - [`NodeShape`] - Node shape for validating focus nodes
//! - [`PropertyShape`] - Property shape for validating property values
//! - [`Target`] - Target declarations for selecting focus nodes
//! - [`ShapesGraph`] - Collection of shapes parsed from an RDF graph

use oxrdf::{
    vocab::{rdf, rdfs, shacl},
    BlankNode, Graph, Literal, NamedNode, NamedNodeRef, NamedOrBlankNode, Term, TermRef,
};
use rustc_hash::FxHashMap;
use std::sync::Arc;

use crate::constraint::Constraint;
use crate::error::ShaclParseError;
use crate::path::PropertyPath;
use crate::report::Severity;

/// Unique identifier for a shape.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ShapeId {
    /// Named shape (IRI).
    Named(NamedNode),
    /// Anonymous shape (blank node).
    Blank(BlankNode),
}

impl ShapeId {
    /// Creates a shape ID from a named or blank node.
    pub fn from_named_or_blank(node: NamedOrBlankNode) -> Self {
        match node {
            NamedOrBlankNode::NamedNode(n) => Self::Named(n),
            NamedOrBlankNode::BlankNode(b) => Self::Blank(b),
        }
    }

    /// Converts to a Term.
    pub fn to_term(&self) -> Term {
        match self {
            Self::Named(n) => Term::NamedNode(n.clone()),
            Self::Blank(b) => Term::BlankNode(b.clone()),
        }
    }

    /// Returns the shape ID as a named node if it is one.
    pub fn as_named(&self) -> Option<&NamedNode> {
        match self {
            Self::Named(n) => Some(n),
            Self::Blank(_) => None,
        }
    }
}

impl From<NamedNode> for ShapeId {
    fn from(n: NamedNode) -> Self {
        Self::Named(n)
    }
}

impl From<BlankNode> for ShapeId {
    fn from(b: BlankNode) -> Self {
        Self::Blank(b)
    }
}

impl std::fmt::Display for ShapeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Named(n) => write!(f, "<{}>", n.as_str()),
            Self::Blank(b) => write!(f, "_:{}", b.as_str()),
        }
    }
}

/// Target declaration for selecting focus nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Target {
    /// Target all instances of a class.
    Class(NamedNode),
    /// Target a specific node.
    Node(Term),
    /// Target all subjects of triples with the given predicate.
    SubjectsOf(NamedNode),
    /// Target all objects of triples with the given predicate.
    ObjectsOf(NamedNode),
    /// Implicit target (shape is also a class).
    Implicit(NamedNode),
}

impl Target {
    /// Finds all focus nodes matching this target in a data graph.
    pub fn find_focus_nodes(&self, graph: &Graph) -> Vec<Term> {
        match self {
            Self::Class(class) => {
                // Find all instances of the class (including subclass instances)
                let mut instances = Vec::new();
                let classes = get_class_hierarchy(graph, class);

                for cls in classes {
                    for subj in graph.subjects_for_predicate_object(rdf::TYPE, &cls) {
                        instances.push(subj.into_owned().into());
                    }
                }

                instances
            }

            Self::Node(node) => {
                // Specific node target
                vec![node.clone()]
            }

            Self::SubjectsOf(predicate) => {
                // All subjects of triples with this predicate
                let mut subjects = Vec::new();
                for triple in graph.triples_for_predicate(predicate) {
                    subjects.push(triple.subject.into_owned().into());
                }
                subjects
            }

            Self::ObjectsOf(predicate) => {
                // All objects of triples with this predicate
                let mut objects = Vec::new();
                for triple in graph.triples_for_predicate(predicate) {
                    objects.push(triple.object.into_owned());
                }
                objects
            }

            Self::Implicit(class) => {
                // Same as Class target
                Self::Class(class.clone()).find_focus_nodes(graph)
            }
        }
    }
}

/// Gets a class and all its subclasses.
fn get_class_hierarchy(graph: &Graph, class: &NamedNode) -> Vec<Term> {
    let mut classes = vec![Term::NamedNode(class.clone())];
    let mut to_check: Vec<Term> = vec![Term::NamedNode(class.clone())];

    while let Some(current) = to_check.pop() {
        // Find subclasses
        for subclass in graph.subjects_for_predicate_object(rdfs::SUB_CLASS_OF, current.as_ref()) {
            let subclass_term: Term = subclass.into_owned().into();
            if !classes.contains(&subclass_term) {
                classes.push(subclass_term.clone());
                to_check.push(subclass_term);
            }
        }
    }

    classes
}

/// Base shape containing common properties.
#[derive(Debug, Clone)]
pub struct Shape {
    /// Shape identifier.
    pub id: ShapeId,
    /// Target declarations.
    pub targets: Vec<Target>,
    /// Constraints to apply.
    pub constraints: Vec<Constraint>,
    /// Nested property shapes.
    pub property_shapes: Vec<Arc<PropertyShape>>,
    /// Shape severity level.
    pub severity: Severity,
    /// Whether the shape is deactivated.
    pub deactivated: bool,
    /// Human-readable name.
    pub name: Option<String>,
    /// Human-readable description.
    pub description: Option<String>,
    /// Custom validation message.
    pub message: Option<String>,
}

impl Shape {
    /// Creates a new shape with the given ID.
    pub fn new(id: ShapeId) -> Self {
        Self {
            id,
            targets: Vec::new(),
            constraints: Vec::new(),
            property_shapes: Vec::new(),
            severity: Severity::Violation,
            deactivated: false,
            name: None,
            description: None,
            message: None,
        }
    }

    /// Returns true if this shape has no targets (and should use implicit targeting).
    pub fn has_targets(&self) -> bool {
        !self.targets.is_empty()
    }

    /// Adds a target to this shape.
    pub fn add_target(&mut self, target: Target) {
        self.targets.push(target);
    }

    /// Adds a constraint to this shape.
    pub fn add_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint);
    }

    /// Adds a property shape to this shape.
    pub fn add_property_shape(&mut self, property_shape: Arc<PropertyShape>) {
        self.property_shapes.push(property_shape);
    }
}

/// Node shape for validating focus nodes directly.
#[derive(Debug, Clone)]
pub struct NodeShape {
    /// Base shape properties.
    pub base: Shape,
}

impl NodeShape {
    /// Creates a new node shape with the given ID.
    pub fn new(id: ShapeId) -> Self {
        Self {
            base: Shape::new(id),
        }
    }

    /// Returns a reference to the shape ID.
    pub fn id(&self) -> &ShapeId {
        &self.base.id
    }
}

/// Property shape for validating property values.
#[derive(Debug, Clone)]
pub struct PropertyShape {
    /// Base shape properties.
    pub base: Shape,
    /// Property path (required for property shapes).
    pub path: PropertyPath,
}

impl PropertyShape {
    /// Creates a new property shape with the given ID and path.
    pub fn new(id: ShapeId, path: PropertyPath) -> Self {
        Self {
            base: Shape::new(id),
            path,
        }
    }

    /// Returns a reference to the shape ID.
    pub fn id(&self) -> &ShapeId {
        &self.base.id
    }

    /// Returns a reference to the property path.
    pub fn path(&self) -> &PropertyPath {
        &self.path
    }
}

/// Collection of shapes parsed from an RDF graph.
#[derive(Debug, Clone)]
pub struct ShapesGraph {
    /// Node shapes indexed by ID.
    node_shapes: FxHashMap<ShapeId, Arc<NodeShape>>,
    /// Property shapes indexed by ID.
    property_shapes: FxHashMap<ShapeId, Arc<PropertyShape>>,
    /// All shapes (for iteration).
    all_shape_ids: Vec<ShapeId>,
}

impl ShapesGraph {
    /// Creates a new empty shapes graph.
    pub fn new() -> Self {
        Self {
            node_shapes: FxHashMap::default(),
            property_shapes: FxHashMap::default(),
            all_shape_ids: Vec::new(),
        }
    }

    /// Parses shapes from an RDF graph.
    pub fn from_graph(graph: &Graph) -> Result<Self, ShaclParseError> {
        let mut shapes_graph = Self::new();

        // Find all node shapes
        for shape_node in graph.subjects_for_predicate_object(rdf::TYPE, shacl::NODE_SHAPE) {
            let id = ShapeId::from_named_or_blank(shape_node.into_owned());
            let node_shape = parse_node_shape(graph, &id)?;
            shapes_graph.add_node_shape(Arc::new(node_shape));
        }

        // Find shapes that are both sh:NodeShape and rdfs:Class (implicit targets)
        for shape_node in graph.subjects_for_predicate_object(rdf::TYPE, shacl::SHAPE) {
            let id = ShapeId::from_named_or_blank(shape_node.into_owned());
            if !shapes_graph.node_shapes.contains_key(&id) {
                // Check if it's also a class (implicit target)
                let is_class = match &id {
                    ShapeId::Named(n) => graph
                        .objects_for_subject_predicate(n, rdf::TYPE)
                        .any(|t| t == TermRef::NamedNode(rdfs::CLASS)),
                    ShapeId::Blank(b) => graph
                        .objects_for_subject_predicate(b, rdf::TYPE)
                        .any(|t| t == TermRef::NamedNode(rdfs::CLASS)),
                };

                if is_class {
                    let mut node_shape = parse_node_shape(graph, &id)?;
                    if let ShapeId::Named(n) = &id {
                        node_shape.base.targets.push(Target::Implicit(n.clone()));
                    }
                    shapes_graph.add_node_shape(Arc::new(node_shape));
                }
            }
        }

        // Find standalone property shapes (not nested)
        for shape_node in graph.subjects_for_predicate_object(rdf::TYPE, shacl::PROPERTY_SHAPE) {
            let id = ShapeId::from_named_or_blank(shape_node.into_owned());
            if !shapes_graph.property_shapes.contains_key(&id) {
                if let Some(property_shape) = parse_property_shape(graph, &id)? {
                    shapes_graph.add_property_shape(Arc::new(property_shape));
                }
            }
        }

        Ok(shapes_graph)
    }

    /// Adds a node shape to the graph.
    pub fn add_node_shape(&mut self, shape: Arc<NodeShape>) {
        let id = shape.id().clone();
        if !self.node_shapes.contains_key(&id) {
            self.all_shape_ids.push(id.clone());
        }
        self.node_shapes.insert(id, shape);
    }

    /// Adds a property shape to the graph.
    pub fn add_property_shape(&mut self, shape: Arc<PropertyShape>) {
        let id = shape.id().clone();
        if !self.property_shapes.contains_key(&id) {
            self.all_shape_ids.push(id.clone());
        }
        self.property_shapes.insert(id, shape);
    }

    /// Gets a node shape by ID.
    pub fn get_node_shape(&self, id: &ShapeId) -> Option<&Arc<NodeShape>> {
        self.node_shapes.get(id)
    }

    /// Gets a property shape by ID.
    pub fn get_property_shape(&self, id: &ShapeId) -> Option<&Arc<PropertyShape>> {
        self.property_shapes.get(id)
    }

    /// Returns an iterator over all node shapes.
    pub fn node_shapes(&self) -> impl Iterator<Item = &Arc<NodeShape>> {
        self.node_shapes.values()
    }

    /// Returns an iterator over all property shapes.
    pub fn property_shapes(&self) -> impl Iterator<Item = &Arc<PropertyShape>> {
        self.property_shapes.values()
    }

    /// Returns true if the shapes graph is empty.
    pub fn is_empty(&self) -> bool {
        self.node_shapes.is_empty() && self.property_shapes.is_empty()
    }

    /// Returns the number of shapes.
    pub fn len(&self) -> usize {
        self.node_shapes.len() + self.property_shapes.len()
    }
}

impl Default for ShapesGraph {
    fn default() -> Self {
        Self::new()
    }
}

// Parsing helpers

fn parse_node_shape(graph: &Graph, id: &ShapeId) -> Result<NodeShape, ShaclParseError> {
    let mut shape = NodeShape::new(id.clone());
    let term = id.to_term();

    // Parse targets
    parse_targets(graph, &term, &mut shape.base)?;

    // Parse constraints
    parse_constraints(graph, &term, &mut shape.base)?;

    // Parse property shapes
    parse_property_shapes(graph, &term, &mut shape.base)?;

    // Parse metadata
    parse_metadata(graph, &term, &mut shape.base)?;

    Ok(shape)
}

fn parse_property_shape(
    graph: &Graph,
    id: &ShapeId,
) -> Result<Option<PropertyShape>, ShaclParseError> {
    let term = id.to_term();

    // Property shapes must have sh:path
    let path_term = get_object(graph, &term, shacl::PATH);
    let path = match path_term {
        Some(p) => PropertyPath::parse(graph, p.as_ref())?,
        None => return Ok(None), // Not a valid property shape without path
    };

    let mut shape = PropertyShape::new(id.clone(), path);

    // Parse targets
    parse_targets(graph, &term, &mut shape.base)?;

    // Parse constraints
    parse_constraints(graph, &term, &mut shape.base)?;

    // Parse nested property shapes
    parse_property_shapes(graph, &term, &mut shape.base)?;

    // Parse metadata
    parse_metadata(graph, &term, &mut shape.base)?;

    Ok(Some(shape))
}

fn parse_targets(graph: &Graph, shape_term: &Term, shape: &mut Shape) -> Result<(), ShaclParseError> {
    // sh:targetClass
    for obj in get_objects(graph, shape_term, shacl::TARGET_CLASS) {
        if let Term::NamedNode(class) = obj {
            shape.targets.push(Target::Class(class));
        }
    }

    // sh:targetNode
    for obj in get_objects(graph, shape_term, shacl::TARGET_NODE) {
        shape.targets.push(Target::Node(obj));
    }

    // sh:targetSubjectsOf
    for obj in get_objects(graph, shape_term, shacl::TARGET_SUBJECTS_OF) {
        if let Term::NamedNode(pred) = obj {
            shape.targets.push(Target::SubjectsOf(pred));
        }
    }

    // sh:targetObjectsOf
    for obj in get_objects(graph, shape_term, shacl::TARGET_OBJECTS_OF) {
        if let Term::NamedNode(pred) = obj {
            shape.targets.push(Target::ObjectsOf(pred));
        }
    }

    Ok(())
}

fn parse_constraints(
    graph: &Graph,
    shape_term: &Term,
    shape: &mut Shape,
) -> Result<(), ShaclParseError> {
    // Parse all constraint types

    // sh:class
    for obj in get_objects(graph, shape_term, shacl::CLASS) {
        if let Term::NamedNode(class) = obj {
            shape.constraints.push(Constraint::Class(class));
        }
    }

    // sh:datatype
    if let Some(Term::NamedNode(dt)) = get_object(graph, shape_term, shacl::DATATYPE) {
        shape.constraints.push(Constraint::Datatype(dt));
    }

    // sh:nodeKind
    if let Some(Term::NamedNode(nk)) = get_object(graph, shape_term, shacl::NODE_KIND) {
        shape.constraints.push(Constraint::NodeKind(nk));
    }

    // sh:minCount
    if let Some(n) = get_integer(graph, shape_term, shacl::MIN_COUNT) {
        shape.constraints.push(Constraint::MinCount(n as usize));
    }

    // sh:maxCount
    if let Some(n) = get_integer(graph, shape_term, shacl::MAX_COUNT) {
        shape.constraints.push(Constraint::MaxCount(n as usize));
    }

    // sh:minExclusive
    if let Some(lit) = get_literal(graph, shape_term, shacl::MIN_EXCLUSIVE) {
        shape.constraints.push(Constraint::MinExclusive(lit));
    }

    // sh:maxExclusive
    if let Some(lit) = get_literal(graph, shape_term, shacl::MAX_EXCLUSIVE) {
        shape.constraints.push(Constraint::MaxExclusive(lit));
    }

    // sh:minInclusive
    if let Some(lit) = get_literal(graph, shape_term, shacl::MIN_INCLUSIVE) {
        shape.constraints.push(Constraint::MinInclusive(lit));
    }

    // sh:maxInclusive
    if let Some(lit) = get_literal(graph, shape_term, shacl::MAX_INCLUSIVE) {
        shape.constraints.push(Constraint::MaxInclusive(lit));
    }

    // sh:minLength
    if let Some(n) = get_integer(graph, shape_term, shacl::MIN_LENGTH) {
        shape.constraints.push(Constraint::MinLength(n as usize));
    }

    // sh:maxLength
    if let Some(n) = get_integer(graph, shape_term, shacl::MAX_LENGTH) {
        shape.constraints.push(Constraint::MaxLength(n as usize));
    }

    // sh:pattern
    if let Some(pattern) = get_string(graph, shape_term, shacl::PATTERN) {
        let flags = get_string(graph, shape_term, shacl::FLAGS);
        shape.constraints.push(Constraint::Pattern { pattern, flags });
    }

    // sh:languageIn
    if let Some(list_head) = get_object(graph, shape_term, shacl::LANGUAGE_IN) {
        let languages = parse_string_list(graph, list_head, shape_term)?;
        shape.constraints.push(Constraint::LanguageIn(languages));
    }

    // sh:uniqueLang
    if let Some(b) = get_boolean(graph, shape_term, shacl::UNIQUE_LANG) {
        if b {
            shape.constraints.push(Constraint::UniqueLang);
        }
    }

    // sh:equals
    for obj in get_objects(graph, shape_term, shacl::EQUALS) {
        if let Term::NamedNode(prop) = obj {
            shape.constraints.push(Constraint::Equals(prop));
        }
    }

    // sh:disjoint
    for obj in get_objects(graph, shape_term, shacl::DISJOINT) {
        if let Term::NamedNode(prop) = obj {
            shape.constraints.push(Constraint::Disjoint(prop));
        }
    }

    // sh:lessThan
    for obj in get_objects(graph, shape_term, shacl::LESS_THAN) {
        if let Term::NamedNode(prop) = obj {
            shape.constraints.push(Constraint::LessThan(prop));
        }
    }

    // sh:lessThanOrEquals
    for obj in get_objects(graph, shape_term, shacl::LESS_THAN_OR_EQUALS) {
        if let Term::NamedNode(prop) = obj {
            shape.constraints.push(Constraint::LessThanOrEquals(prop));
        }
    }

    // sh:not
    for obj in get_objects(graph, shape_term, shacl::NOT) {
        let shape_id = term_to_shape_id(obj)?;
        shape.constraints.push(Constraint::Not(shape_id));
    }

    // sh:and
    if let Some(list_head) = get_object(graph, shape_term, shacl::AND) {
        let shape_ids = parse_shape_list(graph, list_head, shape_term)?;
        shape.constraints.push(Constraint::And(shape_ids));
    }

    // sh:or
    if let Some(list_head) = get_object(graph, shape_term, shacl::OR) {
        let shape_ids = parse_shape_list(graph, list_head, shape_term)?;
        shape.constraints.push(Constraint::Or(shape_ids));
    }

    // sh:xone
    if let Some(list_head) = get_object(graph, shape_term, shacl::XONE) {
        let shape_ids = parse_shape_list(graph, list_head, shape_term)?;
        shape.constraints.push(Constraint::Xone(shape_ids));
    }

    // sh:node
    for obj in get_objects(graph, shape_term, shacl::NODE) {
        let shape_id = term_to_shape_id(obj)?;
        shape.constraints.push(Constraint::Node(shape_id));
    }

    // sh:hasValue
    for obj in get_objects(graph, shape_term, shacl::HAS_VALUE) {
        shape.constraints.push(Constraint::HasValue(obj));
    }

    // sh:in
    if let Some(list_head) = get_object(graph, shape_term, shacl::IN) {
        let values = parse_term_list(graph, list_head, shape_term)?;
        shape.constraints.push(Constraint::In(values));
    }

    // sh:closed
    if let Some(b) = get_boolean(graph, shape_term, shacl::CLOSED) {
        if b {
            let ignored = if let Some(list_head) = get_object(graph, shape_term, shacl::IGNORED_PROPERTIES) {
                parse_named_node_list(graph, list_head, shape_term)?
            } else {
                Vec::new()
            };
            shape.constraints.push(Constraint::Closed { ignored_properties: ignored });
        }
    }

    // sh:qualifiedValueShape
    if let Some(qvs) = get_object(graph, shape_term, shacl::QUALIFIED_VALUE_SHAPE) {
        let shape_id = term_to_shape_id(qvs)?;
        let min = get_integer(graph, shape_term, shacl::QUALIFIED_MIN_COUNT).map(|n| n as usize);
        let max = get_integer(graph, shape_term, shacl::QUALIFIED_MAX_COUNT).map(|n| n as usize);
        let disjoint = get_boolean(graph, shape_term, shacl::QUALIFIED_VALUE_SHAPES_DISJOINT).unwrap_or(false);
        shape.constraints.push(Constraint::QualifiedValueShape {
            shape: shape_id,
            min_count: min,
            max_count: max,
            disjoint,
        });
    }

    Ok(())
}

fn parse_property_shapes(
    graph: &Graph,
    shape_term: &Term,
    shape: &mut Shape,
) -> Result<(), ShaclParseError> {
    for obj in get_objects(graph, shape_term, shacl::PROPERTY) {
        let prop_id = term_to_shape_id(obj)?;
        if let Some(prop_shape) = parse_property_shape(graph, &prop_id)? {
            shape.property_shapes.push(Arc::new(prop_shape));
        }
    }
    Ok(())
}

fn parse_metadata(
    graph: &Graph,
    shape_term: &Term,
    shape: &mut Shape,
) -> Result<(), ShaclParseError> {
    // sh:deactivated
    if let Some(b) = get_boolean(graph, shape_term, shacl::DEACTIVATED) {
        shape.deactivated = b;
    }

    // sh:severity
    if let Some(Term::NamedNode(sev)) = get_object(graph, shape_term, shacl::SEVERITY) {
        shape.severity = match sev.as_ref() {
            s if s == shacl::VIOLATION => Severity::Violation,
            s if s == shacl::WARNING => Severity::Warning,
            s if s == shacl::INFO => Severity::Info,
            _ => Severity::Violation,
        };
    }

    // sh:name
    shape.name = get_string(graph, shape_term, shacl::NAME);

    // sh:description
    shape.description = get_string(graph, shape_term, shacl::DESCRIPTION);

    // sh:message
    shape.message = get_string(graph, shape_term, shacl::MESSAGE);

    Ok(())
}

// Helper functions

fn get_object(graph: &Graph, subject: &Term, predicate: NamedNodeRef<'_>) -> Option<Term> {
    match subject {
        Term::NamedNode(n) => graph
            .object_for_subject_predicate(n, predicate)
            .map(|t| t.into_owned()),
        Term::BlankNode(b) => graph
            .object_for_subject_predicate(b, predicate)
            .map(|t| t.into_owned()),
        _ => None,
    }
}

fn get_objects(graph: &Graph, subject: &Term, predicate: NamedNodeRef<'_>) -> Vec<Term> {
    match subject {
        Term::NamedNode(n) => graph
            .objects_for_subject_predicate(n, predicate)
            .map(|t| t.into_owned())
            .collect(),
        Term::BlankNode(b) => graph
            .objects_for_subject_predicate(b, predicate)
            .map(|t| t.into_owned())
            .collect(),
        _ => Vec::new(),
    }
}

fn get_string(graph: &Graph, subject: &Term, predicate: NamedNodeRef<'_>) -> Option<String> {
    get_object(graph, subject, predicate).and_then(|t| {
        if let Term::Literal(lit) = t {
            Some(lit.value().to_string())
        } else {
            None
        }
    })
}

fn get_integer(graph: &Graph, subject: &Term, predicate: NamedNodeRef<'_>) -> Option<i64> {
    get_object(graph, subject, predicate).and_then(|t| {
        if let Term::Literal(lit) = t {
            lit.value().parse().ok()
        } else {
            None
        }
    })
}

fn get_boolean(graph: &Graph, subject: &Term, predicate: NamedNodeRef<'_>) -> Option<bool> {
    get_object(graph, subject, predicate).and_then(|t| {
        if let Term::Literal(lit) = t {
            match lit.value() {
                "true" | "1" => Some(true),
                "false" | "0" => Some(false),
                _ => None,
            }
        } else {
            None
        }
    })
}

fn get_literal(graph: &Graph, subject: &Term, predicate: NamedNodeRef<'_>) -> Option<Literal> {
    get_object(graph, subject, predicate).and_then(|t| {
        if let Term::Literal(lit) = t {
            Some(lit)
        } else {
            None
        }
    })
}

fn term_to_shape_id(term: Term) -> Result<ShapeId, ShaclParseError> {
    match term {
        Term::NamedNode(n) => Ok(ShapeId::Named(n)),
        Term::BlankNode(b) => Ok(ShapeId::Blank(b)),
        _ => Err(ShaclParseError::invalid_shape(
            term,
            "Shape reference must be an IRI or blank node",
        )),
    }
}

fn parse_string_list(
    graph: &Graph,
    list_head: Term,
    shape: &Term,
) -> Result<Vec<String>, ShaclParseError> {
    use oxrdf::vocab::rdf;
    let mut strings = Vec::new();
    let mut current = list_head;

    loop {
        if let Term::NamedNode(n) = &current {
            if n.as_ref() == rdf::NIL {
                break;
            }
        }

        let first = get_object(graph, &current, rdf::FIRST).ok_or_else(|| {
            ShaclParseError::invalid_rdf_list(shape.clone(), "Missing rdf:first")
        })?;

        if let Term::Literal(lit) = first {
            strings.push(lit.value().to_string());
        }

        let rest = get_object(graph, &current, rdf::REST).ok_or_else(|| {
            ShaclParseError::invalid_rdf_list(shape.clone(), "Missing rdf:rest")
        })?;

        current = rest;
    }

    Ok(strings)
}

fn parse_term_list(
    graph: &Graph,
    list_head: Term,
    shape: &Term,
) -> Result<Vec<Term>, ShaclParseError> {
    use oxrdf::vocab::rdf;
    let mut terms = Vec::new();
    let mut current = list_head;

    loop {
        if let Term::NamedNode(n) = &current {
            if n.as_ref() == rdf::NIL {
                break;
            }
        }

        let first = get_object(graph, &current, rdf::FIRST).ok_or_else(|| {
            ShaclParseError::invalid_rdf_list(shape.clone(), "Missing rdf:first")
        })?;

        terms.push(first);

        let rest = get_object(graph, &current, rdf::REST).ok_or_else(|| {
            ShaclParseError::invalid_rdf_list(shape.clone(), "Missing rdf:rest")
        })?;

        current = rest;
    }

    Ok(terms)
}

fn parse_named_node_list(
    graph: &Graph,
    list_head: Term,
    shape: &Term,
) -> Result<Vec<NamedNode>, ShaclParseError> {
    let terms = parse_term_list(graph, list_head, shape)?;
    let mut nodes = Vec::new();
    for term in terms {
        if let Term::NamedNode(n) = term {
            nodes.push(n);
        }
    }
    Ok(nodes)
}

fn parse_shape_list(
    graph: &Graph,
    list_head: Term,
    shape: &Term,
) -> Result<Vec<ShapeId>, ShaclParseError> {
    let terms = parse_term_list(graph, list_head, shape)?;
    terms.into_iter().map(term_to_shape_id).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::Triple;

    #[test]
    fn test_parse_empty_shapes_graph() {
        let graph = Graph::new();
        let shapes = ShapesGraph::from_graph(&graph).unwrap();
        assert!(shapes.is_empty());
    }

    #[test]
    fn test_parse_simple_node_shape() {
        let mut graph = Graph::new();
        let shape_node = NamedNode::new("http://example.org/PersonShape").unwrap();
        let person_class = NamedNode::new("http://example.org/Person").unwrap();

        // Add shape type
        graph.insert(&Triple::new(
            shape_node.clone(),
            rdf::TYPE,
            shacl::NODE_SHAPE,
        ));

        // Add target class
        graph.insert(&Triple::new(
            shape_node.clone(),
            shacl::TARGET_CLASS,
            person_class.clone(),
        ));

        let shapes = ShapesGraph::from_graph(&graph).unwrap();
        assert_eq!(shapes.len(), 1);

        let shape = shapes.get_node_shape(&ShapeId::Named(shape_node)).unwrap();
        assert_eq!(shape.base.targets.len(), 1);
    }
}
