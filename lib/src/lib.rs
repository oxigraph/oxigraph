//! Oxigraph is a graph database library implementing the [SPARQL](https://www.w3.org/TR/sparql11-overview/) standard.
//!
//! Its goal is to provide a compliant, safe and fast graph database.
//! It also provides a set of utility functions for reading, writing, and processing RDF files.
//!
//! It currently provides three store implementations providing [SPARQL](https://www.w3.org/TR/sparql11-overview/) capability:
//! * [`MemoryStore`](store::memory::MemoryStore): a simple in memory implementation.
//! * [`RocksDbStore`](store::rocksdb::RocksDbStore): a file system implementation based on the [RocksDB](https://rocksdb.org/) key-value store.
//!   It requires the `"rocksdb"` feature to be activated.
//!   The [clang](https://clang.llvm.org/) compiler needs to be installed to compile RocksDB.
//! * [`SledStore`](store::sled::SledStore): another file system implementation based on the [Sled](https://sled.rs/) key-value store.
//!   It requires the `"sled"` feature to be activated.
//!   Sled is much faster to build than RockDB and does not require a C++ compiler.
//!   However, Sled is still in developpment, less tested and data load seems much slower than RocksDB.
//!
//! Oxigraph is in heavy development and SPARQL query evaluation has not been optimized yet.
//!
//! The disabled by default `"sophia"` feature provides [`sophia_api`](https://docs.rs/sophia_api/) traits implemention on Oxigraph terms and stores.
//!
//! Oxigraph also provides [a standalone HTTP server](https://crates.io/crates/oxigraph_server) based on this library.
//!
//! Usage example with the [`MemoryStore`](store::memory::MemoryStore):
//!
//! ```
//! use oxigraph::MemoryStore;
//! use oxigraph::model::*;
//! use oxigraph::sparql::QueryResults;
//!
//! let store = MemoryStore::new();
//!
//! // insertion
//! let ex = NamedNode::new("http://example.com")?;
//! let quad = Quad::new(ex.clone(), ex.clone(), ex.clone(), None);
//! store.insert(quad.clone());
//!
//! // quad filter
//! let results: Vec<Quad> = store.quads_for_pattern(Some(ex.as_ref().into()), None, None, None).collect();
//! assert_eq!(vec![quad], results);
//!
//! // SPARQL query
//! if let QueryResults::Solutions(mut solutions) =  store.query("SELECT ?s WHERE { ?s ?p ?o }")? {
//!     assert_eq!(solutions.next().unwrap()?.get("s"), Some(&ex.into()));
//! }
//! # Result::<_,Box<dyn std::error::Error>>::Ok(())
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
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/master/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/master/logo.svg")]
#![allow(
    clippy::multiple_crate_versions, //TODO
    clippy::rc_buffer //TODO: enforce
)]
#![warn(
    clippy::cast_lossless,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::checked_conversions,
    clippy::decimal_literal_representation,
    //TODO clippy::doc_markdown,
    //clippy::else_if_without_else,
    clippy::empty_enum,
    clippy::expect_used,
    clippy::expl_impl_clone_on_copy,
    clippy::explicit_into_iter_loop,
    clippy::explicit_iter_loop,
    clippy::expl_impl_clone_on_copy,
    clippy::fallible_impl_from,
    clippy::filter_map_next,
    clippy::manual_find_map,
    clippy::get_unwrap,
    clippy::if_not_else,
    clippy::inline_always,
    clippy::invalid_upcast_comparisons,
    clippy::items_after_statements,
    clippy::map_entry,
    clippy::map_flatten,
    clippy::map_unwrap_or,
    //TODO clippy::match_same_arms,
    clippy::maybe_infinite_iter,
    clippy::mem_forget,
    //TODO clippy::must_use_candidate,
    //TODO clippy::missing_const_for_fn,
    clippy::multiple_crate_versions,
    clippy::multiple_inherent_impl,
    clippy::mut_mut,
    clippy::needless_borrow,
    clippy::needless_continue,
    clippy::needless_pass_by_value,
    clippy::non_ascii_literal,
    // clippy::panic, does not work well with tests
    clippy::path_buf_push_overwrite,
    clippy::print_stdout,
    clippy::pub_enum_variant_names,
    //TODO clippy::redundant_closure_for_method_calls,
    // clippy::shadow_reuse,
    // clippy::shadow_same,
    // clippy::shadow_unrelated,
    // clippy::single_match_else,
    clippy::string_add,
    clippy::string_add_assign,
    clippy::todo,
    clippy::type_repetition_in_bounds,
    clippy::unicode_not_nfc,
    clippy::unimplemented,
    clippy::unseparated_literal_suffix,
    clippy::used_underscore_binding,
    clippy::wildcard_dependencies,
    clippy::wrong_pub_self_convention,
)]
#![doc(test(attr(deny(warnings))))]

mod error;
pub mod io;
pub mod model;
pub mod sparql;
pub mod store;

pub use crate::store::memory::MemoryStore;
#[cfg(feature = "rocksdb")]
pub use crate::store::rocksdb::RocksDbStore;
#[cfg(feature = "sled")]
pub use crate::store::sled::SledStore;
