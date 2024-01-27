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
//! let ex = NamedNodeRef::new("http://example.com").unwrap();
//! let triple = TripleRef::new(ex, ex, ex);
//! graph.insert(triple);
//!
//! // simple filter
//! let results: Vec<_> = graph.triples_for_subject(ex).collect();
//! assert_eq!(vec![triple], results);
//! ```

pub use oxrdf::*;
