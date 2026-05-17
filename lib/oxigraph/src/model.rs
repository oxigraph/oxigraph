//! Implements data structures for [RDF 1.1 Concepts](https://www.w3.org/TR/rdf11-concepts/) using [OxRDF](https://crates.io/crates/oxrdf).
//!
//! Usage example:
//!
//! ```
//! use oxigraph::model::*;
//!
//! let mut graph = Graph::default();
//!
//! // insertion
//! let ex = NamedNode::new("http://example.com").unwrap();
//! let triple = Triple::new(ex.clone(), ex.clone(), ex.clone());
//! graph.insert(triple.clone());
//!
//! // simple filter
//! let results: Vec<_> = graph.triples_for_subject(&ex).collect();
//! assert_eq!(vec![triple], results);
//! ```

pub use oxrdf::*;
