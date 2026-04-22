//! GeoSPARQL 1.1 vocabulary constants.
//!
//! Re-exports every IRI that lives in [`spareval::geosparql::vocab`] and adds
//! the `geosparql.ttl` ontology stub that only the bridge module in this
//! crate needs. The goal is to let downstream consumers reference core
//! GeoSPARQL predicates without hard coding the IRI strings.

pub use spareval::geosparql::vocab::*;

/// GeoSPARQL 1.1 ontology stub shipped with the crate.
///
/// This is not a complete ontology. It carries just enough axioms for a
/// downstream reasoner to understand that Simple Features topological
/// predicates are symmetric and that `geo:sfContains` inverts
/// `geo:sfWithin`, which is what the bridge module relies on when it
/// materialises relations from paired geometries.
pub const GEOSPARQL_TTL: &str = include_str!("../data/geosparql.ttl");
