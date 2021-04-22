//! RDF [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) storage implementations.

pub use crate::store::sled::SledStore;

mod binary_encoder;
pub(crate) mod io;
pub(crate) mod numeric_encoder;
pub mod sled;
pub(crate) mod small_string;
#[cfg(feature = "sophia")]
mod sophia;
pub(crate) mod storage;
