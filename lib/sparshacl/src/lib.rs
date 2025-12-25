//! sparshacl is a Rust implementation of the [W3C SHACL (Shapes Constraint Language)](https://www.w3.org/TR/shacl/)
//! specification for validating RDF graphs against a set of conditions called "shapes".
//!
//! # Example
//!
//! ```
//! use sparshacl::{ShaclValidator, ShapesGraph, ValidationReport};
//! use oxrdf::Graph;
//!
//! // Create a shapes graph (would normally be parsed from RDF)
//! let shapes = Graph::new();
//! let shapes_graph = ShapesGraph::from_graph(&shapes).unwrap();
//!
//! // Create a data graph to validate
//! let data = Graph::new();
//!
//! // Create validator and validate
//! let validator = ShaclValidator::new(shapes_graph);
//! let report = validator.validate(&data).unwrap();
//!
//! assert!(report.conforms());
//! ```

mod constraint;
mod error;
mod model;
mod path;
mod report;
mod validator;

pub use constraint::{Constraint, ConstraintComponent};
pub use error::{ShaclError, ShaclParseError, ShaclValidationError};
pub use model::{NodeShape, PropertyShape, Shape, ShapeId, ShapesGraph, Target};
pub use path::PropertyPath;
pub use report::{Severity, ValidationReport, ValidationResult};
pub use validator::ShaclValidator;

// Re-export vocabulary for convenience
pub use oxrdf::vocab::shacl;
