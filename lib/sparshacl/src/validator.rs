//! SHACL Validator implementation.
//!
//! This module implements the core SHACL validation algorithm.

use oxrdf::{
    vocab::{rdf, shacl},
    Graph, NamedNode, NamedNodeRef, Term, TermRef,
};
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use std::cmp::Ordering;
use std::sync::Arc;

use crate::constraint::{Constraint, ConstraintComponent};
use crate::error::{ShaclError, ShaclValidationError};
use crate::model::{NodeShape, PropertyShape, Shape, ShapeId, ShapesGraph};
use crate::path::PropertyPath;
use crate::report::{Severity, ValidationReport, ValidationResult};

/// Maximum recursion depth for shape validation.
const MAX_RECURSION_DEPTH: usize = 50;

/// SHACL validator for validating RDF graphs against shapes.
#[derive(Debug, Clone)]
pub struct ShaclValidator {
    /// The shapes graph containing all shapes.
    shapes_graph: ShapesGraph,
}

impl ShaclValidator {
    /// Creates a new validator with the given shapes graph.
    pub fn new(shapes_graph: ShapesGraph) -> Self {
        Self { shapes_graph }
    }

    /// Returns a reference to the shapes graph.
    pub fn shapes_graph(&self) -> &ShapesGraph {
        &self.shapes_graph
    }

    /// Validates a data graph against the shapes graph.
    pub fn validate(&self, data_graph: &Graph) -> Result<ValidationReport, ShaclError> {
        let mut report = ValidationReport::new();
        let mut context = ValidationContext::new(self, data_graph);

        // Validate all node shapes
        for node_shape in self.shapes_graph.node_shapes() {
            if node_shape.base.deactivated {
                continue;
            }

            // Find focus nodes for this shape
            let focus_nodes = self.find_focus_nodes(&node_shape.base, data_graph);

            // Validate each focus node against the shape
            for focus_node in focus_nodes {
                self.validate_node_against_shape(
                    &mut context,
                    &mut report,
                    &focus_node,
                    node_shape,
                    0,
                )?;
            }
        }

        // Validate standalone property shapes with targets
        for prop_shape in self.shapes_graph.property_shapes() {
            if prop_shape.base.deactivated {
                continue;
            }

            // Only validate property shapes that have explicit targets
            if prop_shape.base.has_targets() {
                let focus_nodes = self.find_focus_nodes(&prop_shape.base, data_graph);

                for focus_node in focus_nodes {
                    self.validate_property_shape(
                        &mut context,
                        &mut report,
                        &focus_node,
                        prop_shape,
                        0,
                        prop_shape.base.severity,
                    )?;
                }
            }
        }

        Ok(report)
    }

    /// Finds all focus nodes for a shape based on its targets.
    fn find_focus_nodes(&self, shape: &Shape, data_graph: &Graph) -> Vec<Term> {
        let mut focus_nodes = FxHashSet::default();

        for target in &shape.targets {
            for node in target.find_focus_nodes(data_graph) {
                focus_nodes.insert(node);
            }
        }

        focus_nodes.into_iter().collect()
    }

    /// Validates a focus node against a node shape.
    fn validate_node_against_shape(
        &self,
        context: &mut ValidationContext<'_>,
        report: &mut ValidationReport,
        focus_node: &Term,
        shape: &Arc<NodeShape>,
        depth: usize,
    ) -> Result<(), ShaclError> {
        if depth > MAX_RECURSION_DEPTH {
            return Err(ShaclValidationError::max_recursion_depth(depth).into());
        }

        let parent_severity = shape.base.severity;

        // Validate constraints on the node shape itself
        for constraint in &shape.base.constraints {
            self.validate_constraint(
                context,
                report,
                focus_node,
                &[focus_node.clone()],
                constraint,
                &shape.base,
                None,
                depth,
                parent_severity,
            )?;
        }

        // Validate nested property shapes (pass parent severity for inheritance)
        for prop_shape in &shape.base.property_shapes {
            self.validate_property_shape(
                context,
                report,
                focus_node,
                prop_shape,
                depth,
                parent_severity,
            )?;
        }

        Ok(())
    }

    /// Validates a focus node against a property shape.
    fn validate_property_shape(
        &self,
        context: &mut ValidationContext<'_>,
        report: &mut ValidationReport,
        focus_node: &Term,
        shape: &Arc<PropertyShape>,
        depth: usize,
        parent_severity: Severity,
    ) -> Result<(), ShaclError> {
        if depth > MAX_RECURSION_DEPTH {
            return Err(ShaclValidationError::max_recursion_depth(depth).into());
        }

        // Use shape's own severity if non-default, otherwise inherit from parent
        let effective_severity = if shape.base.severity != Severity::Violation {
            shape.base.severity
        } else {
            parent_severity
        };

        // Get value nodes via property path
        let value_nodes = shape.path.evaluate(context.data_graph, focus_node.as_ref());

        // Validate constraints against value nodes
        for constraint in &shape.base.constraints {
            self.validate_constraint(
                context,
                report,
                focus_node,
                &value_nodes,
                constraint,
                &shape.base,
                Some(&shape.path),
                depth,
                effective_severity,
            )?;
        }

        // Validate nested property shapes on value nodes
        for nested_prop_shape in &shape.base.property_shapes {
            for value_node in &value_nodes {
                self.validate_property_shape(
                    context,
                    report,
                    value_node,
                    nested_prop_shape,
                    depth + 1,
                    effective_severity,
                )?;
            }
        }

        Ok(())
    }

    /// Validates a single constraint against value nodes.
    #[allow(clippy::too_many_arguments)]
    fn validate_constraint(
        &self,
        context: &mut ValidationContext<'_>,
        report: &mut ValidationReport,
        focus_node: &Term,
        value_nodes: &[Term],
        constraint: &Constraint,
        shape: &Shape,
        path: Option<&PropertyPath>,
        depth: usize,
        effective_severity: Severity,
    ) -> Result<(), ShaclError> {
        let severity = effective_severity;
        let shape_id = shape.id.clone();

        match constraint {
            // === Cardinality Constraints ===
            Constraint::MinCount(min) => {
                if value_nodes.len() < *min {
                    let mut result = ValidationResult::new(
                        focus_node.clone(),
                        shape_id,
                        ConstraintComponent::MinCount,
                    )
                    .with_severity(severity)
                    .with_message(format!(
                        "Expected at least {} value(s), got {}",
                        min,
                        value_nodes.len()
                    ));

                    if let Some(p) = path {
                        result = result.with_path(p.clone());
                    }
                    if let Some(msg) = &shape.message {
                        result = result.with_message(msg.clone());
                    }

                    report.add_result(result);
                }
            }

            Constraint::MaxCount(max) => {
                if value_nodes.len() > *max {
                    let mut result = ValidationResult::new(
                        focus_node.clone(),
                        shape_id,
                        ConstraintComponent::MaxCount,
                    )
                    .with_severity(severity)
                    .with_message(format!(
                        "Expected at most {} value(s), got {}",
                        max,
                        value_nodes.len()
                    ));

                    if let Some(p) = path {
                        result = result.with_path(p.clone());
                    }
                    if let Some(msg) = &shape.message {
                        result = result.with_message(msg.clone());
                    }

                    report.add_result(result);
                }
            }

            // === Value Type Constraints ===
            Constraint::Class(class) => {
                for value in value_nodes {
                    if !is_instance_of(context.data_graph, value, class) {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::Class,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message(format!("Value is not an instance of <{}>", class.as_str()));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::Datatype(datatype) => {
                for value in value_nodes {
                    let valid = match value {
                        Term::Literal(lit) => lit.datatype() == datatype.as_ref(),
                        _ => false,
                    };

                    if !valid {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::Datatype,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message(format!("Value does not have datatype <{}>", datatype.as_str()));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::NodeKind(node_kind) => {
                for value in value_nodes {
                    if !matches_node_kind(value, node_kind.as_ref()) {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::NodeKind,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message(format!("Value does not match node kind <{}>", node_kind.as_str()));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            // === String Constraints ===
            Constraint::MinLength(min) => {
                for value in value_nodes {
                    let len = get_string_length(value);
                    if len < *min {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::MinLength,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message(format!(
                            "String length {} is less than minimum {}",
                            len, min
                        ));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::MaxLength(max) => {
                for value in value_nodes {
                    let len = get_string_length(value);
                    if len > *max {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::MaxLength,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message(format!(
                            "String length {} exceeds maximum {}",
                            len, max
                        ));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::Pattern { pattern, flags } => {
                let regex = context.get_or_compile_regex(pattern, flags.as_deref())?;

                for value in value_nodes {
                    let str_value = get_string_value(value);
                    if !regex.is_match(&str_value) {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::Pattern,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message(format!("Value does not match pattern '{}'", pattern));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::LanguageIn(languages) => {
                for value in value_nodes {
                    let valid = match value {
                        Term::Literal(lit) => {
                            if let Some(lang) = lit.language() {
                                languages.iter().any(|l| lang.starts_with(l.as_str()))
                            } else {
                                false
                            }
                        }
                        _ => false,
                    };

                    if !valid {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::LanguageIn,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message("Language tag not in allowed list");

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::UniqueLang => {
                let mut seen_langs = FxHashSet::default();
                for value in value_nodes {
                    if let Term::Literal(lit) = value {
                        if let Some(lang) = lit.language() {
                            if !seen_langs.insert(lang.to_string()) {
                                let mut result = ValidationResult::new(
                                    focus_node.clone(),
                                    shape_id.clone(),
                                    ConstraintComponent::UniqueLang,
                                )
                                .with_value(value.clone())
                                .with_severity(severity)
                                .with_message(format!("Duplicate language tag: {}", lang));

                                if let Some(p) = path {
                                    result = result.with_path(p.clone());
                                }

                                report.add_result(result);
                            }
                        }
                    }
                }
            }

            // === Value Range Constraints ===
            Constraint::MinExclusive(min) => {
                for value in value_nodes {
                    if let Some(cmp) = compare_values(value, &Term::Literal(min.clone())) {
                        if cmp != Ordering::Greater {
                            let mut result = ValidationResult::new(
                                focus_node.clone(),
                                shape_id.clone(),
                                ConstraintComponent::MinExclusive,
                            )
                            .with_value(value.clone())
                            .with_severity(severity)
                            .with_message(format!("Value must be greater than {}", min.value()));

                            if let Some(p) = path {
                                result = result.with_path(p.clone());
                            }

                            report.add_result(result);
                        }
                    }
                }
            }

            Constraint::MaxExclusive(max) => {
                for value in value_nodes {
                    if let Some(cmp) = compare_values(value, &Term::Literal(max.clone())) {
                        if cmp != Ordering::Less {
                            let mut result = ValidationResult::new(
                                focus_node.clone(),
                                shape_id.clone(),
                                ConstraintComponent::MaxExclusive,
                            )
                            .with_value(value.clone())
                            .with_severity(severity)
                            .with_message(format!("Value must be less than {}", max.value()));

                            if let Some(p) = path {
                                result = result.with_path(p.clone());
                            }

                            report.add_result(result);
                        }
                    }
                }
            }

            Constraint::MinInclusive(min) => {
                for value in value_nodes {
                    if let Some(cmp) = compare_values(value, &Term::Literal(min.clone())) {
                        if cmp == Ordering::Less {
                            let mut result = ValidationResult::new(
                                focus_node.clone(),
                                shape_id.clone(),
                                ConstraintComponent::MinInclusive,
                            )
                            .with_value(value.clone())
                            .with_severity(severity)
                            .with_message(format!(
                                "Value must be greater than or equal to {}",
                                min.value()
                            ));

                            if let Some(p) = path {
                                result = result.with_path(p.clone());
                            }

                            report.add_result(result);
                        }
                    }
                }
            }

            Constraint::MaxInclusive(max) => {
                for value in value_nodes {
                    if let Some(cmp) = compare_values(value, &Term::Literal(max.clone())) {
                        if cmp == Ordering::Greater {
                            let mut result = ValidationResult::new(
                                focus_node.clone(),
                                shape_id.clone(),
                                ConstraintComponent::MaxInclusive,
                            )
                            .with_value(value.clone())
                            .with_severity(severity)
                            .with_message(format!(
                                "Value must be less than or equal to {}",
                                max.value()
                            ));

                            if let Some(p) = path {
                                result = result.with_path(p.clone());
                            }

                            report.add_result(result);
                        }
                    }
                }
            }

            // === Property Pair Constraints ===
            Constraint::Equals(property) => {
                let other_values: FxHashSet<_> = get_property_values(context.data_graph, focus_node, property)
                    .into_iter()
                    .collect();
                let value_set: FxHashSet<_> = value_nodes.iter().cloned().collect();

                if value_set != other_values {
                    let mut result = ValidationResult::new(
                        focus_node.clone(),
                        shape_id.clone(),
                        ConstraintComponent::Equals,
                    )
                    .with_severity(severity)
                    .with_message(format!(
                        "Values do not equal values of property <{}>",
                        property.as_str()
                    ));

                    if let Some(p) = path {
                        result = result.with_path(p.clone());
                    }

                    report.add_result(result);
                }
            }

            Constraint::Disjoint(property) => {
                let other_values: FxHashSet<_> = get_property_values(context.data_graph, focus_node, property)
                    .into_iter()
                    .collect();

                for value in value_nodes {
                    if other_values.contains(value) {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::Disjoint,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message(format!(
                            "Value appears in property <{}> which should be disjoint",
                            property.as_str()
                        ));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::LessThan(property) => {
                let other_values = get_property_values(context.data_graph, focus_node, property);

                for value in value_nodes {
                    for other in &other_values {
                        if let Some(cmp) = compare_values(value, other) {
                            if cmp != Ordering::Less {
                                let mut result = ValidationResult::new(
                                    focus_node.clone(),
                                    shape_id.clone(),
                                    ConstraintComponent::LessThan,
                                )
                                .with_value(value.clone())
                                .with_severity(severity)
                                .with_message(format!(
                                    "Value is not less than values of <{}>",
                                    property.as_str()
                                ));

                                if let Some(p) = path {
                                    result = result.with_path(p.clone());
                                }

                                report.add_result(result);
                            }
                        }
                    }
                }
            }

            Constraint::LessThanOrEquals(property) => {
                let other_values = get_property_values(context.data_graph, focus_node, property);

                for value in value_nodes {
                    for other in &other_values {
                        if let Some(cmp) = compare_values(value, other) {
                            if cmp == Ordering::Greater {
                                let mut result = ValidationResult::new(
                                    focus_node.clone(),
                                    shape_id.clone(),
                                    ConstraintComponent::LessThanOrEquals,
                                )
                                .with_value(value.clone())
                                .with_severity(severity)
                                .with_message(format!(
                                    "Value is not less than or equal to values of <{}>",
                                    property.as_str()
                                ));

                                if let Some(p) = path {
                                    result = result.with_path(p.clone());
                                }

                                report.add_result(result);
                            }
                        }
                    }
                }
            }

            // === Logical Constraints ===
            Constraint::Not(ref_shape_id) => {
                for value in value_nodes {
                    if self.node_conforms_to_shape(context, value, ref_shape_id, depth + 1)? {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::Not,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message("Value conforms to negated shape");

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::And(shape_ids) => {
                for value in value_nodes {
                    for ref_shape_id in shape_ids {
                        if !self.node_conforms_to_shape(context, value, ref_shape_id, depth + 1)? {
                            let mut result = ValidationResult::new(
                                focus_node.clone(),
                                shape_id.clone(),
                                ConstraintComponent::And,
                            )
                            .with_value(value.clone())
                            .with_severity(severity)
                            .with_message("Value does not conform to all shapes in sh:and");

                            if let Some(p) = path {
                                result = result.with_path(p.clone());
                            }

                            report.add_result(result);
                            break;
                        }
                    }
                }
            }

            Constraint::Or(shape_ids) => {
                for value in value_nodes {
                    let conforms = shape_ids
                        .iter()
                        .any(|ref_shape_id| {
                            self.node_conforms_to_shape(context, value, ref_shape_id, depth + 1)
                                .unwrap_or(false)
                        });

                    if !conforms {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::Or,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message("Value does not conform to any shape in sh:or");

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::Xone(shape_ids) => {
                for value in value_nodes {
                    let conforming_count = shape_ids
                        .iter()
                        .filter(|ref_shape_id| {
                            self.node_conforms_to_shape(context, value, ref_shape_id, depth + 1)
                                .unwrap_or(false)
                        })
                        .count();

                    if conforming_count != 1 {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::Xone,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message(format!(
                            "Value conforms to {} shapes, expected exactly 1",
                            conforming_count
                        ));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::Node(ref_shape_id) => {
                for value in value_nodes {
                    if !self.node_conforms_to_shape(context, value, ref_shape_id, depth + 1)? {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::Node,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message("Value does not conform to referenced shape");

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            // === Other Constraints ===
            Constraint::HasValue(required_value) => {
                if !value_nodes.contains(required_value) {
                    let mut result = ValidationResult::new(
                        focus_node.clone(),
                        shape_id.clone(),
                        ConstraintComponent::HasValue,
                    )
                    .with_severity(severity)
                    .with_message("Required value not found");

                    if let Some(p) = path {
                        result = result.with_path(p.clone());
                    }

                    report.add_result(result);
                }
            }

            Constraint::In(allowed_values) => {
                for value in value_nodes {
                    if !allowed_values.contains(value) {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::In,
                        )
                        .with_value(value.clone())
                        .with_severity(severity)
                        .with_message("Value is not in the allowed list");

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::Closed { ignored_properties } => {
                // Get all properties of the focus node
                let allowed_properties: FxHashSet<_> = shape
                    .property_shapes
                    .iter()
                    .filter_map(|ps| ps.path.as_predicate())
                    .cloned()
                    .chain(ignored_properties.iter().cloned())
                    .collect();

                // Check for unexpected properties
                for triple in get_triples_for_subject(context.data_graph, focus_node) {
                    if !allowed_properties.contains(&triple.predicate) {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::Closed,
                        )
                        .with_value(triple.object.clone())
                        .with_severity(severity)
                        .with_message(format!(
                            "Unexpected property <{}>",
                            triple.predicate.as_str()
                        ));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }

            Constraint::QualifiedValueShape {
                shape: ref_shape_id,
                min_count,
                max_count,
                disjoint: _,
            } => {
                let conforming_count = value_nodes
                    .iter()
                    .filter(|v| {
                        self.node_conforms_to_shape(context, v, ref_shape_id, depth + 1)
                            .unwrap_or(false)
                    })
                    .count();

                if let Some(min) = min_count {
                    if conforming_count < *min {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::QualifiedValueShape,
                        )
                        .with_severity(severity)
                        .with_message(format!(
                            "Expected at least {} value(s) conforming to qualified shape, got {}",
                            min, conforming_count
                        ));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }

                if let Some(max) = max_count {
                    if conforming_count > *max {
                        let mut result = ValidationResult::new(
                            focus_node.clone(),
                            shape_id.clone(),
                            ConstraintComponent::QualifiedValueShape,
                        )
                        .with_severity(severity)
                        .with_message(format!(
                            "Expected at most {} value(s) conforming to qualified shape, got {}",
                            max, conforming_count
                        ));

                        if let Some(p) = path {
                            result = result.with_path(p.clone());
                        }

                        report.add_result(result);
                    }
                }
            }
        }

        Ok(())
    }

    /// Checks if a node conforms to a shape (used for logical constraints).
    fn node_conforms_to_shape(
        &self,
        context: &mut ValidationContext<'_>,
        node: &Term,
        shape_id: &ShapeId,
        depth: usize,
    ) -> Result<bool, ShaclError> {
        if depth > MAX_RECURSION_DEPTH {
            return Err(ShaclValidationError::max_recursion_depth(depth).into());
        }

        // Try to find the shape
        if let Some(node_shape) = self.shapes_graph.get_node_shape(shape_id) {
            let mut temp_report = ValidationReport::new();
            self.validate_node_against_shape(context, &mut temp_report, node, node_shape, depth)?;
            return Ok(temp_report.conforms());
        }

        if let Some(prop_shape) = self.shapes_graph.get_property_shape(shape_id) {
            let mut temp_report = ValidationReport::new();
            self.validate_property_shape(
                context,
                &mut temp_report,
                node,
                prop_shape,
                depth,
                prop_shape.base.severity,
            )?;
            return Ok(temp_report.conforms());
        }

        // Shape not found - treat as conforming (or could return error)
        Ok(true)
    }
}

/// Internal validation context.
struct ValidationContext<'a> {
    #[allow(dead_code)]
    validator: &'a ShaclValidator,
    data_graph: &'a Graph,
    regex_cache: FxHashMap<String, Regex>,
}

impl<'a> ValidationContext<'a> {
    fn new(validator: &'a ShaclValidator, data_graph: &'a Graph) -> Self {
        Self {
            validator,
            data_graph,
            regex_cache: FxHashMap::default(),
        }
    }

    fn get_or_compile_regex(
        &mut self,
        pattern: &str,
        flags: Option<&str>,
    ) -> Result<&Regex, ShaclError> {
        let key = format!("{}{}", pattern, flags.unwrap_or(""));

        if !self.regex_cache.contains_key(&key) {
            let mut regex_pattern = String::new();

            // Handle flags
            if let Some(f) = flags {
                if f.contains('i') {
                    regex_pattern.push_str("(?i)");
                }
                if f.contains('m') {
                    regex_pattern.push_str("(?m)");
                }
                if f.contains('s') {
                    regex_pattern.push_str("(?s)");
                }
            }

            regex_pattern.push_str(pattern);

            let regex = Regex::new(&regex_pattern).map_err(|e| {
                ShaclError::Parse(crate::error::ShaclParseError::invalid_regex(
                    pattern,
                    e.to_string(),
                ))
            })?;

            self.regex_cache.insert(key.clone(), regex);
        }

        Ok(self.regex_cache.get(&key).unwrap())
    }
}

// Helper functions

fn is_instance_of(graph: &Graph, term: &Term, class: &NamedNode) -> bool {
    match term {
        Term::NamedNode(n) => graph
            .objects_for_subject_predicate(n, rdf::TYPE)
            .any(|t| match t {
                TermRef::NamedNode(type_node) => type_node == class.as_ref(),
                _ => false,
            }),
        Term::BlankNode(b) => graph
            .objects_for_subject_predicate(b, rdf::TYPE)
            .any(|t| match t {
                TermRef::NamedNode(type_node) => type_node == class.as_ref(),
                _ => false,
            }),
        _ => false,
    }
}

fn matches_node_kind(term: &Term, node_kind: NamedNodeRef<'_>) -> bool {
    match node_kind {
        k if k == shacl::IRI => matches!(term, Term::NamedNode(_)),
        k if k == shacl::LITERAL => matches!(term, Term::Literal(_)),
        k if k == shacl::BLANK_NODE => matches!(term, Term::BlankNode(_)),
        k if k == shacl::BLANK_NODE_OR_IRI => {
            matches!(term, Term::NamedNode(_) | Term::BlankNode(_))
        }
        k if k == shacl::BLANK_NODE_OR_LITERAL => {
            matches!(term, Term::BlankNode(_) | Term::Literal(_))
        }
        k if k == shacl::IRI_OR_LITERAL => {
            matches!(term, Term::NamedNode(_) | Term::Literal(_))
        }
        _ => false,
    }
}

fn get_string_value(term: &Term) -> String {
    match term {
        Term::NamedNode(n) => n.as_str().to_string(),
        Term::BlankNode(b) => b.as_str().to_string(),
        Term::Literal(l) => l.value().to_string(),
        #[cfg(feature = "rdf-12")]
        Term::Triple(_) => String::new(),
    }
}

fn get_string_length(term: &Term) -> usize {
    get_string_value(term).chars().count()
}

fn compare_values(a: &Term, b: &Term) -> Option<Ordering> {
    match (a, b) {
        (Term::Literal(la), Term::Literal(lb)) => {
            // Try numeric comparison first
            if let (Ok(na), Ok(nb)) = (la.value().parse::<f64>(), lb.value().parse::<f64>()) {
                return na.partial_cmp(&nb);
            }
            // Fall back to string comparison
            Some(la.value().cmp(lb.value()))
        }
        (Term::NamedNode(na), Term::NamedNode(nb)) => Some(na.as_str().cmp(nb.as_str())),
        _ => None,
    }
}

fn get_property_values(graph: &Graph, subject: &Term, predicate: &NamedNode) -> Vec<Term> {
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

struct SimpleTriple {
    predicate: NamedNode,
    object: Term,
}

fn get_triples_for_subject(graph: &Graph, subject: &Term) -> Vec<SimpleTriple> {
    match subject {
        Term::NamedNode(n) => graph
            .triples_for_subject(n)
            .map(|t| SimpleTriple {
                predicate: t.predicate.into_owned(),
                object: t.object.into_owned(),
            })
            .collect(),
        Term::BlankNode(b) => graph
            .triples_for_subject(b)
            .map(|t| SimpleTriple {
                predicate: t.predicate.into_owned(),
                object: t.object.into_owned(),
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::{vocab::xsd, Literal, Triple};

    #[test]
    fn test_empty_shapes_validation() {
        let shapes = ShapesGraph::new();
        let validator = ShaclValidator::new(shapes);
        let data = Graph::new();

        let report = validator.validate(&data).unwrap();
        assert!(report.conforms());
    }

    #[test]
    fn test_min_count_constraint() {
        // Create shapes graph
        let mut shapes_graph = Graph::new();
        let shape = NamedNode::new("http://example.org/PersonShape").unwrap();
        let person = NamedNode::new("http://example.org/Person").unwrap();
        let name_prop = NamedNode::new("http://example.org/name").unwrap();
        let prop_shape = oxrdf::BlankNode::default();

        // Shape is a NodeShape
        shapes_graph.insert(&Triple::new(shape.clone(), rdf::TYPE, shacl::NODE_SHAPE));
        // Target class
        shapes_graph.insert(&Triple::new(
            shape.clone(),
            shacl::TARGET_CLASS,
            person.clone(),
        ));
        // Property shape
        shapes_graph.insert(&Triple::new(shape.clone(), shacl::PROPERTY, prop_shape.clone()));
        shapes_graph.insert(&Triple::new(prop_shape.clone(), shacl::PATH, name_prop.clone()));
        shapes_graph.insert(&Triple::new(
            prop_shape.clone(),
            shacl::MIN_COUNT,
            Literal::new_typed_literal("1", xsd::INTEGER),
        ));

        let shapes = ShapesGraph::from_graph(&shapes_graph).unwrap();
        let validator = ShaclValidator::new(shapes);

        // Data graph with person without name
        let mut data = Graph::new();
        let alice = NamedNode::new("http://example.org/alice").unwrap();
        data.insert(&Triple::new(alice.clone(), rdf::TYPE, person.clone()));

        let report = validator.validate(&data).unwrap();
        assert!(!report.conforms());
        assert_eq!(report.violation_count(), 1);
    }

    #[test]
    fn test_datatype_constraint() {
        // Create shapes graph
        let mut shapes_graph = Graph::new();
        let shape = NamedNode::new("http://example.org/PersonShape").unwrap();
        let person = NamedNode::new("http://example.org/Person").unwrap();
        let age_prop = NamedNode::new("http://example.org/age").unwrap();
        let prop_shape = oxrdf::BlankNode::default();

        shapes_graph.insert(&Triple::new(shape.clone(), rdf::TYPE, shacl::NODE_SHAPE));
        shapes_graph.insert(&Triple::new(
            shape.clone(),
            shacl::TARGET_CLASS,
            person.clone(),
        ));
        shapes_graph.insert(&Triple::new(shape.clone(), shacl::PROPERTY, prop_shape.clone()));
        shapes_graph.insert(&Triple::new(prop_shape.clone(), shacl::PATH, age_prop.clone()));
        shapes_graph.insert(&Triple::new(prop_shape.clone(), shacl::DATATYPE, xsd::INTEGER));

        let shapes = ShapesGraph::from_graph(&shapes_graph).unwrap();
        let validator = ShaclValidator::new(shapes);

        // Data graph with wrong datatype
        let mut data = Graph::new();
        let alice = NamedNode::new("http://example.org/alice").unwrap();
        data.insert(&Triple::new(alice.clone(), rdf::TYPE, person.clone()));
        data.insert(&Triple::new(
            alice.clone(),
            age_prop,
            Literal::new_simple_literal("thirty"),
        ));

        let report = validator.validate(&data).unwrap();
        assert!(!report.conforms());
        assert_eq!(report.violation_count(), 1);
    }
}
