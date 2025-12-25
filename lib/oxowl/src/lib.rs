//! # OxOWL - OWL 2 Ontology Support for Oxigraph
//!
//! This crate provides comprehensive OWL 2 Web Ontology Language support including:
//!
//! - **Ontology Data Model**: Classes, properties, individuals, and axioms
//! - **Class Expressions**: Union, intersection, complement, restrictions
//! - **OWL 2 Profiles**: Support for OWL 2 RL reasoning profile
//! - **Reasoning**: Forward-chaining inference engine
//! - **Parsing**: Load OWL ontologies from RDF graphs
//!
//! ## Quick Start
//!
//! ```rust
//! use oxowl::{Ontology, Axiom, ClassExpression, OwlClass, Individual};
//! use oxrdf::NamedNode;
//!
//! // Create an ontology
//! let mut ontology = Ontology::with_iri("http://example.org/animals").unwrap();
//!
//! // Define classes
//! let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());
//! let dog = OwlClass::new(NamedNode::new("http://example.org/Dog").unwrap());
//!
//! // Add subclass axiom: Dog ⊑ Animal
//! ontology.add_axiom(Axiom::subclass_of(
//!     ClassExpression::class(dog.clone()),
//!     ClassExpression::class(animal.clone()),
//! ));
//!
//! // Create an individual
//! let fido = Individual::Named(NamedNode::new("http://example.org/fido").unwrap());
//!
//! // Assert fido is a Dog
//! ontology.add_axiom(Axiom::class_assertion(
//!     ClassExpression::class(dog),
//!     fido,
//! ));
//!
//! println!("Ontology has {} axioms", ontology.axiom_count());
//! ```
//!
//! ## Reasoning
//!
//! The crate includes an OWL 2 RL reasoner for computing inferences:
//!
//! ```rust
//! # use oxowl::{Ontology, Axiom, ClassExpression, OwlClass, Individual};
//! # use oxrdf::NamedNode;
//! # let mut ontology = Ontology::new(None);
//! # let animal = OwlClass::new(NamedNode::new("http://example.org/Animal").unwrap());
//! # let dog = OwlClass::new(NamedNode::new("http://example.org/Dog").unwrap());
//! # ontology.add_axiom(Axiom::subclass_of(ClassExpression::class(dog.clone()), ClassExpression::class(animal.clone())));
//! # let fido = Individual::Named(NamedNode::new("http://example.org/fido").unwrap());
//! # ontology.add_axiom(Axiom::class_assertion(ClassExpression::class(dog), fido.clone()));
//! use oxowl::{Reasoner, RlReasoner};
//!
//! let mut reasoner = RlReasoner::new(&ontology);
//! reasoner.classify().unwrap();
//!
//! // fido is inferred to be an Animal (through Dog ⊑ Animal)
//! let types = reasoner.get_types(&fido);
//! assert!(types.iter().any(|c| c.iri().as_str() == "http://example.org/Animal"));
//! ```
//!
//! ## Features
//!
//! - `reasoner-rl` (default): OWL 2 RL reasoning support
//! - `reasoner-el`: OWL 2 EL profile support (planned)
//! - `reasoner-rdfs`: Pure RDFS reasoning (planned)
//! - `rdf-12`: RDF 1.2 features

mod entity;
mod axiom;
mod expression;
mod ontology;
mod error;
mod parser;

pub use entity::{OwlClass, ObjectProperty, DataProperty, AnnotationProperty, Individual};
pub use axiom::Axiom;
pub use expression::{ClassExpression, ObjectPropertyExpression, DataRange};
pub use ontology::Ontology;
pub use error::{OwlError, OwlParseError};
pub use parser::{parse_ontology, parse_ontology_with_config, OntologyParser, ParserConfig};

#[cfg(feature = "reasoner-rl")]
mod reasoner;

#[cfg(feature = "reasoner-rl")]
pub use reasoner::{Reasoner, RlReasoner, ReasonerConfig};
