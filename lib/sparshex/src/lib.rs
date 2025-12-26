//! ShEx (Shape Expressions) validation for RDF graphs.
//!
//! This crate provides a complete implementation of [ShEx](https://shex.io/) for validating
//! RDF graphs against shape constraints. ShEx is a concise, human-readable language for
//! describing RDF graph structures.
//!
//! # Core Concepts
//!
//! - **Schema**: A collection of shape definitions ([`ShapesSchema`])
//! - **Shape Expression**: Constraints on RDF nodes ([`ShapeExpression`])
//! - **Validation**: Checking if nodes conform to shapes ([`ShexValidator`])
//! - **Report**: Results of validation ([`ValidationResult`])
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use sparshex::{ShapesSchema, ShexValidator};
//! use oxrdf::Graph;
//!
//! // Create a ShEx schema
//! let schema = ShapesSchema::new();
//!
//! // Create a validator
//! let validator = ShexValidator::new(schema);
//!
//! // Validate data against the schema
//! let graph = Graph::new();
//! // let result = validator.validate(&graph, focus_node)?;
//!
//! // if result.is_valid() {
//! //     println!("Data is valid!");
//! // } else {
//! //     for error in result.errors() {
//! //         println!("Validation error: {}", error);
//! //     }
//! // }
//! ```
//!
//! # Comparison with SHACL
//!
//! ShEx and SHACL are both RDF validation languages with different philosophies:
//!
//! | Feature | ShEx | SHACL |
//! |---------|------|-------|
//! | **Syntax** | Compact, human-friendly | RDF-based (Turtle/SPARQL) |
//! | **Expressiveness** | Pattern matching focus | Constraint checking focus |
//! | **Closed shapes** | Native support | Via `sh:closed` |
//! | **Recursion** | First-class support | Limited |
//!
//! The API intentionally mirrors SHACL for consistency across Oxigraph validation libraries.

#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![deny(unsafe_code)]
#![warn(missing_docs)]

mod error;
mod limits;
mod model;
mod parser;
mod result;
mod validator;

#[cfg(test)]
mod tests;

// Public API exports - following SHACL pattern for consistency
pub use error::{ShexError, ShexParseError, ShexValidationError};
pub use model::{
    Annotation, Cardinality, NodeConstraint, NodeKind, NumericFacet, NumericLiteral, Shape,
    ShapeExpression, ShapeLabel, ShapesSchema, StringFacet, TripleConstraint, ValueSetValue,
};
pub use result::ValidationResult;
pub use validator::ShexValidator;
