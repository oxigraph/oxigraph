//! OWL 2 RL forward chaining over an Oxigraph [`Store`](crate::store::Store).
//!
//! This module re-exports the public [`oxreason`] surface so callers only
//! need to depend on `oxigraph` with the `reasoning` feature enabled. The
//! entry point is [`Store::reason`](crate::store::Store::reason), which
//! materialises the reasoning closure of the store's default graph and
//! writes inferred triples back into a configurable target graph.
//!
//! ```
//! use oxigraph::model::*;
//! use oxigraph::reasoning::ReasonerConfig;
//! use oxigraph::store::Store;
//!
//! let store = Store::new()?;
//! let alice = NamedNodeRef::new("http://example.com/alice")?;
//! let person = NamedNodeRef::new("http://example.com/Person")?;
//! let agent = NamedNodeRef::new("http://example.com/Agent")?;
//! let ty = NamedNodeRef::new("http://www.w3.org/1999/02/22-rdf-syntax-ns#type")?;
//! let sub_class =
//!     NamedNodeRef::new("http://www.w3.org/2000/01/rdf-schema#subClassOf")?;
//!
//! store.insert(QuadRef::new(alice, ty, person, GraphNameRef::DefaultGraph))?;
//! store.insert(QuadRef::new(person, sub_class, agent, GraphNameRef::DefaultGraph))?;
//!
//! let report = store.reason(&ReasonerConfig::owl2_rl())?;
//! assert!(report.added >= 1);
//! assert!(store.contains(QuadRef::new(alice, ty, agent, GraphNameRef::DefaultGraph))?);
//! # Result::<_, Box<dyn std::error::Error>>::Ok(())
//! ```

pub use oxreason::{
    ReasonError, Reasoner, ReasonerConfig, ReasoningProfile, ReasoningReport, Rule, RuleId,
    RuleSet,
};

pub use crate::store::ReasoningError;
