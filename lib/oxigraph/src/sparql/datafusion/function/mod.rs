mod compare;
mod ebv;
mod equality;
mod order;
mod to_rdf_literal;

pub use compare::{ComparisonOperator, compare_terms};
pub use ebv::effective_boolean_value;
pub use equality::term_equals;
pub use order::order_by_collation;
pub use to_rdf_literal::to_rdf_literal;
