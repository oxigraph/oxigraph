//! Oxigraph is a work in progress graph database implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.
//!
//! Its goal is to provide a compliant, safe and fast graph database.
//!
//! It currently provides three `Store` implementation providing [SPARQL 1.1 query](https://www.w3.org/TR/sparql11-query/) capability:
//! * `MemoryStore`: a simple in memory implementation.
//! * `RocksDbStore`: a file system implementation based on the [RocksDB](https://rocksdb.org/) key-value store.
//!   It requires the `"rocksdb"` feature to be activated.
//!   It also requires the clang](https://clang.llvm.org/) compiler to be installed.
//! * `Sled`: an other file system implementation based on the [Sled](https://sled.rs/) key-value store.
//!   It requires the `"sled"` feature to be activated.
//!   Sled is much faster to build than RockDB and does not require a C++ compiler.
//!   However Sled is still in heavy developpment, less tested and data load seems much slower than RocksDB.
//!
//! Usage example with the `MemoryStore`:
//!
//! ```
//! use oxigraph::model::*;
//! use oxigraph::{MemoryStore, Result};
//! use crate::oxigraph::sparql::{PreparedQuery, QueryOptions};
//! use oxigraph::sparql::QueryResult;
//!
//! let store = MemoryStore::default();
//!
//! // insertion
//! let ex = NamedNode::parse("http://example.com")?;
//! let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
//! store.insert(&quad)?;
//!
//! // quad filter
//! let results: Result<Vec<Quad>> = store.quads_for_pattern(None, None, None, None).collect();
//! assert_eq!(vec![quad], results?);
//!
//! // SPARQL query
//! let prepared_query = store.prepare_query("SELECT ?s WHERE { ?s ?p ?o }", QueryOptions::default())?;
//! let results = prepared_query.exec()?;
//! if let QueryResult::Bindings(results) = results {
//!     assert_eq!(results.into_values_iter().next().unwrap()?[0], Some(ex.into()));
//! }
//! # Result::Ok(())
//! ```
#![deny(
    future_incompatible,
    nonstandard_style,
    rust_2018_idioms,
    missing_copy_implementations,
    trivial_casts,
    trivial_numeric_casts,
    unsafe_code,
    unused_qualifications
)]
#![warn(
    clippy::unimplemented,
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::checked_conversions,
    clippy::decimal_literal_representation,
    //TODO clippy::doc_markdown,
    clippy::empty_enum,
    clippy::option_expect_used,
    clippy::expl_impl_clone_on_copy,
    clippy::explicit_into_iter_loop,
    clippy::explicit_iter_loop,
    clippy::fallible_impl_from,
    clippy::filter_map,
    clippy::filter_map_next,
    clippy::find_map,
    clippy::get_unwrap,
    clippy::if_not_else,
    clippy::inline_always,
    clippy::invalid_upcast_comparisons,
    clippy::items_after_statements,
    clippy::map_flatten,
    //TODO clippy::match_same_arms,
    clippy::maybe_infinite_iter,
    clippy::mem_forget,
    //TODO clippy::must_use_candidate,
    clippy::multiple_inherent_impl,
    clippy::mut_mut,
    clippy::needless_borrow,
    clippy::needless_continue,
    clippy::needless_pass_by_value,
    clippy::non_ascii_literal,
    clippy::option_map_unwrap_or,
    clippy::option_map_unwrap_or_else,
    // clippy::panic, does not work well with tests
    clippy::path_buf_push_overwrite,
    clippy::print_stdout,
    clippy::pub_enum_variant_names,
    //TODO clippy::redundant_closure_for_method_calls,
    clippy::replace_consts,
    clippy::result_map_unwrap_or_else,
    // clippy::shadow_reuse,
    // clippy::shadow_same,
    // clippy::shadow_unrelated,
    // clippy::single_match_else,
    clippy::string_add,
    clippy::string_add_assign,
    clippy::todo,
    clippy::type_repetition_in_bounds,
    clippy::unicode_not_nfc,
    clippy::unseparated_literal_suffix,
    clippy::used_underscore_binding,
    clippy::wildcard_dependencies,
    clippy::wrong_pub_self_convention,
)]

mod error;
pub mod model;
pub mod sparql;
pub(crate) mod store;
mod syntax;

pub use error::Error;
pub type Result<T> = ::std::result::Result<T, Error>;
pub use crate::store::MemoryStore;
pub use crate::store::MemoryTransaction;
#[cfg(feature = "rocksdb")]
pub use crate::store::RocksDbStore;
#[cfg(feature = "rocksdb")]
pub use crate::store::RocksDbTransaction;
#[cfg(feature = "sled")]
pub use crate::store::SledStore;
pub use crate::syntax::DatasetSyntax;
pub use crate::syntax::FileSyntax;
pub use crate::syntax::GraphSyntax;
