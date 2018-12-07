//! This crate is a work in progress of implementation of an RDF and SPARQL software stack in Rust.
//!
//! Its goal is to provide a compliant, safe and fast implementation of W3C specifications.
//!
//! It currently provides:
//! * Basic RDF data structures in the `model` package
//! * Parsers for XML, Turtle and N-Triples syntaxes in the `rio` package
//! * A memory based and a disk based stores in the `store` package
//! * A work in progress SPARQL implementation in the `sparql` package

#![warn(
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::default_trait_access,
    clippy::empty_enum,
    clippy::enum_glob_use,
    clippy::expl_impl_clone_on_copy,
    clippy::explicit_into_iter_loop,
    clippy::filter_map,
    clippy::if_not_else,
    clippy::inline_always,
    clippy::invalid_upcast_comparisons,
    clippy::items_after_statements,
    clippy::linkedlist,
    //TODO match_same_arms,
    clippy::maybe_infinite_iter,
    clippy::mut_mut,
    clippy::needless_continue,
    clippy::option_map_unwrap_or,
    //TODO option_map_unwrap_or_else,
    clippy::pub_enum_variant_names,
    clippy::replace_consts,
    clippy::result_map_unwrap_or_else,
    //TODO single_match_else,
    clippy::string_add_assign,
    clippy::unicode_not_nfc
)]

pub mod model;
pub mod rio;
pub mod sparql;
pub mod store;
mod utils;

pub use failure::Error;
pub type Result<T> = ::std::result::Result<T, failure::Error>;
