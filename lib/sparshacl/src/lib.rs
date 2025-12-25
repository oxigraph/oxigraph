#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

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
