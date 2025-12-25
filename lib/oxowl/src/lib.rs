//! OWL 2 ontology support for Oxigraph.
//!
//! This crate provides OWL 2 Web Ontology Language support including:
//! - Ontology data model (classes, properties, individuals, axioms)
//! - OWL 2 profiles (EL, QL, RL) validation
//! - Rule-based reasoning (OWL 2 RL)
//! - RDF parsing and serialization
//!
//! # Example
//! ```
//! use oxowl::{Ontology, Axiom, ClassExpression};
//! use oxrdf::NamedNode;
//!
//! let mut ontology = Ontology::new(Some(
//!     NamedNode::new("http://example.org/animals").unwrap()
//! ));
//!
//! // Ontologies can be built programmatically or parsed from RDF
//! ```

mod entity;
mod axiom;
mod expression;
mod ontology;
mod error;

pub use entity::{OwlClass, ObjectProperty, DataProperty, AnnotationProperty, Individual};
pub use axiom::Axiom;
pub use expression::{ClassExpression, ObjectPropertyExpression, DataRange};
pub use ontology::Ontology;
pub use error::{OwlError, OwlParseError};

#[cfg(feature = "reasoner-rl")]
mod reasoner;

#[cfg(feature = "reasoner-rl")]
pub use reasoner::{Reasoner, RlReasoner, ReasonerConfig};
