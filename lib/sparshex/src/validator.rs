//! ShEx validator implementation.
//!
//! This module implements the core ShEx validation algorithm based on the
//! ShEx specification: https://shex.io/shex-semantics/

use crate::error::ShexValidationError;
use crate::model::{
    NodeConstraint, NumericFacet, Shape, ShapeExpression, ShapeLabel, ShapesSchema, StringFacet,
    TripleConstraint, ValueSetValue,
};
use crate::result::ValidationResult;
use oxrdf::{Graph, Literal, NamedNode, Term};
use regex::Regex;
use rustc_hash::{FxHashMap, FxHashSet};
use std::cmp::Ordering;

/// Maximum recursion depth for shape validation to prevent infinite loops.
const MAX_RECURSION_DEPTH: usize = 100;

/// ShEx validator for validating RDF graphs against ShEx shapes.
#[derive(Debug)]
pub struct ShexValidator {
    schema: ShapesSchema,
}

impl ShexValidator {
    /// Creates a new validator with the given shapes schema.
    pub fn new(schema: ShapesSchema) -> Self {
        Self { schema }
    }

    /// Returns a reference to the shapes schema.
    pub fn schema(&self) -> &ShapesSchema {
        &self.schema
    }

    /// Validates a node against a specific shape in the schema.
    ///
    /// # Arguments
    /// * `graph` - The RDF graph to validate
    /// * `node` - The focus node to validate
    /// * `shape_label` - The label of the shape to validate against
    ///
    /// # Returns
    /// A ValidationResult indicating success or failure
    pub fn validate(
        &self,
        graph: &Graph,
        node: &Term,
        shape_label: &ShapeLabel,
    ) -> Result<ValidationResult, ShexValidationError> {
        let mut context = ValidationContext::new(graph);
        self.validate_node_against_shape(&mut context, node, shape_label, 0)
    }

    /// Validates a node against a shape expression.
    fn validate_node_against_shape(
        &self,
        context: &mut ValidationContext<'_>,
        node: &Term,
        shape_label: &ShapeLabel,
        depth: usize,
    ) -> Result<ValidationResult, ShexValidationError> {
        if depth > MAX_RECURSION_DEPTH {
            return Err(ShexValidationError::max_recursion_depth(depth));
        }

        // Check if we've already validated this (node, shape) pair to detect cycles
        let key = (node.clone(), shape_label.clone());
        if context.visited.contains(&key) {
            // Already validating this pair - assume it succeeds to break cycle
            return Ok(ValidationResult::valid());
        }

        context.visited.insert(key.clone());

        let shape = self
            .schema
            .get_shape(shape_label)
            .ok_or_else(|| ShexValidationError::shape_not_found(shape_label.to_string()))?;

        let result = self.validate_shape_expression(context, node, shape, depth)?;

        context.visited.remove(&key);

        Ok(result)
    }

    /// Validates a node against a shape expression.
    fn validate_shape_expression(
        &self,
        context: &mut ValidationContext<'_>,
        node: &Term,
        shape_expr: &ShapeExpression,
        depth: usize,
    ) -> Result<ValidationResult, ShexValidationError> {
        match shape_expr {
            ShapeExpression::NodeConstraint(nc) => {
                self.validate_node_constraint(context, node, nc)
            }
            ShapeExpression::Shape(shape) => self.validate_shape(context, node, shape, depth),
            ShapeExpression::ShapeRef(label) => {
                self.validate_node_against_shape(context, node, label, depth + 1)
            }
            ShapeExpression::ShapeAnd(shapes) => {
                self.validate_shape_and(context, node, shapes, depth)
            }
            ShapeExpression::ShapeOr(shapes) => {
                self.validate_shape_or(context, node, shapes, depth)
            }
            ShapeExpression::ShapeNot(shape) => {
                self.validate_shape_not(context, node, shape, depth)
            }
            ShapeExpression::ShapeExternal => {
                // External shapes are not validated locally
                Ok(ValidationResult::valid())
            }
        }
    }

    /// Validates a node against node constraints.
    fn validate_node_constraint(
        &self,
        context: &mut ValidationContext<'_>,
        node: &Term,
        constraint: &NodeConstraint,
    ) -> Result<ValidationResult, ShexValidationError> {
        let mut errors = Vec::new();

        // Check node kind
        if let Some(node_kind) = &constraint.node_kind {
            if !node_kind.matches(node) {
                errors.push(format!(
                    "Node {} does not match node kind {}",
                    node, node_kind
                ));
            }
        }

        // Check datatype
        if let Some(datatype) = &constraint.datatype {
            match node {
                Term::Literal(lit) => {
                    if lit.datatype() != datatype.as_ref() {
                        errors.push(format!(
                            "Literal datatype {} does not match expected {}",
                            lit.datatype(),
                            datatype
                        ));
                    }
                }
                _ => {
                    errors.push("Datatype constraint requires a literal".to_string());
                }
            }
        }

        // Check string facets
        for facet in &constraint.string_facets {
            match facet {
                StringFacet::MinLength(min) => {
                    let len = get_string_length(node);
                    if len < *min {
                        errors.push(format!(
                            "String length {} is less than minimum {}",
                            len, min
                        ));
                    }
                }
                StringFacet::MaxLength(max) => {
                    let len = get_string_length(node);
                    if len > *max {
                        errors.push(format!("String length {} exceeds maximum {}", len, max));
                    }
                }
                StringFacet::Pattern { pattern, flags } => {
                    let regex = context.get_or_compile_regex(pattern, flags.as_deref())?;
                    let string_value = get_string_value(node);
                    if !regex.is_match(&string_value) {
                        errors.push(format!("Value does not match pattern '{}'", pattern));
                    }
                }
            }
        }

        // Check numeric facets
        for facet in &constraint.numeric_facets {
            match facet {
                NumericFacet::MinInclusive(min) => {
                    if let Some(cmp) = compare_values(node, &min.value) {
                        if cmp == Ordering::Less {
                            errors.push(format!(
                                "Value is less than minimum inclusive {}",
                                min.value.value()
                            ));
                        }
                    }
                }
                NumericFacet::MinExclusive(min) => {
                    if let Some(cmp) = compare_values(node, &min.value) {
                        if cmp != Ordering::Greater {
                            errors.push(format!(
                                "Value is not greater than minimum exclusive {}",
                                min.value.value()
                            ));
                        }
                    }
                }
                NumericFacet::MaxInclusive(max) => {
                    if let Some(cmp) = compare_values(node, &max.value) {
                        if cmp == Ordering::Greater {
                            errors.push(format!(
                                "Value exceeds maximum inclusive {}",
                                max.value.value()
                            ));
                        }
                    }
                }
                NumericFacet::MaxExclusive(max) => {
                    if let Some(cmp) = compare_values(node, &max.value) {
                        if cmp != Ordering::Less {
                            errors.push(format!(
                                "Value is not less than maximum exclusive {}",
                                max.value.value()
                            ));
                        }
                    }
                }
                NumericFacet::TotalDigits(_) | NumericFacet::FractionDigits(_) => {
                    // TODO: Implement digit facets
                }
            }
        }

        // Check value set
        if !constraint.values.is_empty() {
            let matches = constraint.values.iter().any(|v| matches_value_set(node, v));
            if !matches {
                errors.push("Value is not in the allowed value set".to_string());
            }
        }

        if errors.is_empty() {
            Ok(ValidationResult::valid())
        } else {
            Ok(ValidationResult::invalid(errors))
        }
    }

    /// Validates a node against a shape (triple constraints).
    fn validate_shape(
        &self,
        context: &mut ValidationContext<'_>,
        node: &Term,
        shape: &Shape,
        depth: usize,
    ) -> Result<ValidationResult, ShexValidationError> {
        let mut errors = Vec::new();

        // Get all triples where node is the subject
        let triples = get_triples_for_subject(context.graph, node);

        // Validate each triple constraint
        for tc in &shape.triple_constraints {
            let tc_errors =
                self.validate_triple_constraint(context, node, &triples, tc, depth + 1)?;
            errors.extend(tc_errors);
        }

        // Check closed shape constraint
        if shape.closed {
            let allowed_predicates: FxHashSet<_> = shape
                .triple_constraints
                .iter()
                .map(|tc| tc.predicate.clone())
                .chain(shape.extra.iter().cloned())
                .collect();

            for triple in &triples {
                if !allowed_predicates.contains(&triple.predicate) {
                    errors.push(format!(
                        "Closed shape violation: unexpected predicate {}",
                        triple.predicate
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(ValidationResult::valid())
        } else {
            Ok(ValidationResult::invalid(errors))
        }
    }

    /// Validates a triple constraint against a set of triples.
    fn validate_triple_constraint(
        &self,
        context: &mut ValidationContext<'_>,
        focus_node: &Term,
        all_triples: &[TriplePattern],
        tc: &TripleConstraint,
        depth: usize,
    ) -> Result<Vec<String>, ShexValidationError> {
        let mut errors = Vec::new();

        // Get matching triples (subject matches focus node, predicate matches constraint)
        let matching_triples: Vec<_> = all_triples
            .iter()
            .filter(|t| {
                if tc.inverse {
                    &t.object == focus_node && &t.predicate == &tc.predicate
                } else {
                    &t.subject == focus_node && &t.predicate == &tc.predicate
                }
            })
            .collect();

        let count = matching_triples.len() as u32;

        // Check cardinality
        if !tc.cardinality.allows(count) {
            errors.push(format!(
                "Cardinality violation for predicate {}: expected {}, found {}",
                tc.predicate, tc.cardinality, count
            ));
        }

        // Validate values against value expression if present
        if let Some(value_expr) = &tc.value_expr {
            for triple in &matching_triples {
                let value = if tc.inverse {
                    &triple.subject
                } else {
                    &triple.object
                };

                let result = self.validate_shape_expression(context, value, value_expr, depth)?;
                if !result.is_valid() {
                    for error in result.errors() {
                        errors.push(format!(
                            "Value {} for predicate {} failed validation: {}",
                            value, tc.predicate, error
                        ));
                    }
                }
            }
        }

        Ok(errors)
    }

    /// Validates ShapeAnd (all shapes must match).
    fn validate_shape_and(
        &self,
        context: &mut ValidationContext<'_>,
        node: &Term,
        shapes: &[ShapeExpression],
        depth: usize,
    ) -> Result<ValidationResult, ShexValidationError> {
        let mut all_errors = Vec::new();

        for shape in shapes {
            let result = self.validate_shape_expression(context, node, shape, depth + 1)?;
            if !result.is_valid() {
                all_errors.extend(result.errors().to_vec());
            }
        }

        if all_errors.is_empty() {
            Ok(ValidationResult::valid())
        } else {
            Ok(ValidationResult::invalid(all_errors))
        }
    }

    /// Validates ShapeOr (at least one shape must match).
    fn validate_shape_or(
        &self,
        context: &mut ValidationContext<'_>,
        node: &Term,
        shapes: &[ShapeExpression],
        depth: usize,
    ) -> Result<ValidationResult, ShexValidationError> {
        for shape in shapes {
            let result = self.validate_shape_expression(context, node, shape, depth + 1)?;
            if result.is_valid() {
                return Ok(ValidationResult::valid());
            }
        }

        Ok(ValidationResult::invalid(vec![format!(
            "ShapeOr violation: node {} does not match any of the {} shapes",
            node,
            shapes.len()
        )]))
    }

    /// Validates ShapeNot (shape must not match).
    fn validate_shape_not(
        &self,
        context: &mut ValidationContext<'_>,
        node: &Term,
        shape: &ShapeExpression,
        depth: usize,
    ) -> Result<ValidationResult, ShexValidationError> {
        let result = self.validate_shape_expression(context, node, shape, depth + 1)?;
        if result.is_valid() {
            Ok(ValidationResult::invalid(vec![format!(
                "ShapeNot violation: node {} matches the negated shape",
                node
            )]))
        } else {
            Ok(ValidationResult::valid())
        }
    }
}

/// Validation context for tracking state during validation.
struct ValidationContext<'a> {
    /// The data graph being validated.
    graph: &'a Graph,
    /// Set of (node, shape_label) pairs currently being validated (for cycle detection).
    visited: FxHashSet<(Term, ShapeLabel)>,
    /// Cache of compiled regular expressions.
    regex_cache: FxHashMap<String, Regex>,
}

impl<'a> ValidationContext<'a> {
    /// Creates a new validation context.
    fn new(graph: &'a Graph) -> Self {
        Self {
            graph,
            visited: FxHashSet::default(),
            regex_cache: FxHashMap::default(),
        }
    }

    /// Gets or compiles a regular expression with optional flags.
    fn get_or_compile_regex(
        &mut self,
        pattern: &str,
        flags: Option<&str>,
    ) -> Result<&Regex, ShexValidationError> {
        let key = format!(
            "{}{}",
            pattern,
            flags.map_or(String::new(), |f| format!("/{}", f))
        );

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
                ShexValidationError::Internal {
                    message: format!("Invalid regex pattern '{}': {}", pattern, e),
                }
            })?;

            self.regex_cache.insert(key.clone(), regex);
        }

        self.regex_cache.get(&key).ok_or_else(|| {
            ShexValidationError::Internal {
                message: "regex cache miss".to_string(),
            }
        })
    }
}

/// A simple triple pattern for validation.
#[derive(Debug, Clone, PartialEq)]
struct TriplePattern {
    subject: Term,
    predicate: NamedNode,
    object: Term,
}

// Helper functions

/// Gets string value from any RDF term.
#[allow(unreachable_patterns)]
fn get_string_value(term: &Term) -> String {
    match term {
        Term::NamedNode(n) => n.as_str().to_owned(),
        Term::BlankNode(b) => b.as_str().to_owned(),
        Term::Literal(l) => l.value().to_owned(),
        #[cfg(feature = "rdf-12")]
        Term::Triple(_) => String::new(),
        #[cfg(not(feature = "rdf-12"))]
        _ => String::new(), // Catch-all for any other term types
    }
}

/// Gets string length (character count) from any RDF term.
fn get_string_length(term: &Term) -> usize {
    get_string_value(term).chars().count()
}

/// Compares two values for ordering (for numeric/string comparisons).
fn compare_values(a: &Term, b: &Literal) -> Option<Ordering> {
    match a {
        Term::Literal(la) => {
            // Try numeric comparison first
            if let (Ok(na), Ok(nb)) = (la.value().parse::<f64>(), b.value().parse::<f64>()) {
                return na.partial_cmp(&nb);
            }
            // Fall back to string comparison
            Some(la.value().cmp(b.value()))
        }
        _ => None,
    }
}

/// Checks if a term matches a value set value.
fn matches_value_set(term: &Term, value_set: &ValueSetValue) -> bool {
    match value_set {
        ValueSetValue::ObjectValue(val) => term == val,
        ValueSetValue::IriStem(stem) => {
            if let Term::NamedNode(n) = term {
                n.as_str().starts_with(stem)
            } else {
                false
            }
        }
        ValueSetValue::IriStemRange { stem, exclusions } => {
            if let Term::NamedNode(n) = term {
                n.as_str().starts_with(stem)
                    && !exclusions.iter().any(|ex| matches_value_set(term, ex))
            } else {
                false
            }
        }
        ValueSetValue::LiteralStem(stem) => {
            if let Term::Literal(l) = term {
                l.value().starts_with(stem)
            } else {
                false
            }
        }
        ValueSetValue::LiteralStemRange { stem, exclusions } => {
            if let Term::Literal(l) = term {
                l.value().starts_with(stem)
                    && !exclusions.iter().any(|ex| matches_value_set(term, ex))
            } else {
                false
            }
        }
        ValueSetValue::LanguageStem(stem) => {
            if let Term::Literal(l) = term {
                if let Some(lang) = l.language() {
                    // LanguageTag doesn't expose as_str(), so convert to string
                    lang.to_string().starts_with(stem)
                } else {
                    false
                }
            } else {
                false
            }
        }
        ValueSetValue::LanguageStemRange { stem, exclusions } => {
            if let Term::Literal(l) = term {
                if let Some(lang) = l.language() {
                    lang.to_string().starts_with(stem)
                        && !exclusions.iter().any(|ex| matches_value_set(term, ex))
                } else {
                    false
                }
            } else {
                false
            }
        }
    }
}

/// Gets all triples where the given term is the subject.
#[allow(unreachable_patterns)]
fn get_triples_for_subject(graph: &Graph, subject: &Term) -> Vec<TriplePattern> {
    match subject {
        Term::NamedNode(n) => graph
            .triples_for_subject(n)
            .map(|t| TriplePattern {
                subject: subject.clone(),
                predicate: t.predicate.into_owned(),
                object: t.object.into_owned(),
            })
            .collect(),
        Term::BlankNode(b) => graph
            .triples_for_subject(b)
            .map(|t| TriplePattern {
                subject: subject.clone(),
                predicate: t.predicate.into_owned(),
                object: t.object.into_owned(),
            })
            .collect(),
        Term::Literal(_) => Vec::new(),
        #[cfg(feature = "rdf-12")]
        Term::Triple(_) => Vec::new(),
        #[cfg(not(feature = "rdf-12"))]
        _ => Vec::new(), // Catch-all for any other term types
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxrdf::vocab::xsd;

    #[test]
    fn test_validator_creation() {
        let schema = ShapesSchema::new();
        let validator = ShexValidator::new(schema);
        assert!(validator.schema().is_empty());
    }

    #[test]
    fn test_node_kind_validation() {
        let iri = Term::NamedNode(NamedNode::new("http://example.org/x").unwrap());
        assert!(NodeKind::Iri.matches(&iri));
        assert!(!NodeKind::Literal.matches(&iri));
    }

    #[test]
    fn test_get_string_value() {
        let iri = Term::NamedNode(NamedNode::new("http://example.org/x").unwrap());
        assert_eq!(get_string_value(&iri), "http://example.org/x");

        let literal = Term::Literal(Literal::new_simple_literal("hello"));
        assert_eq!(get_string_value(&literal), "hello");
    }

    #[test]
    fn test_get_string_length() {
        let literal = Term::Literal(Literal::new_simple_literal("hello"));
        assert_eq!(get_string_length(&literal), 5);

        let literal = Term::Literal(Literal::new_simple_literal(""));
        assert_eq!(get_string_length(&literal), 0);
    }

    #[test]
    fn test_matches_value_set_object_value() {
        let value = Term::Literal(Literal::new_simple_literal("test"));
        let value_set = ValueSetValue::ObjectValue(value.clone());
        assert!(matches_value_set(&value, &value_set));

        let other = Term::Literal(Literal::new_simple_literal("other"));
        assert!(!matches_value_set(&other, &value_set));
    }

    #[test]
    fn test_matches_value_set_iri_stem() {
        let iri = Term::NamedNode(NamedNode::new("http://example.org/person/1").unwrap());
        let stem = ValueSetValue::iri_stem("http://example.org/person/");
        assert!(matches_value_set(&iri, &stem));

        let other = Term::NamedNode(NamedNode::new("http://other.org/x").unwrap());
        assert!(!matches_value_set(&other, &stem));
    }
}
