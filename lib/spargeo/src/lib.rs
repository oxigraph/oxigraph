#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

//! GeoSPARQL support crate.
//!
//! As of version 0.5.5, the GeoSPARQL extension function table lives in
//! `spareval::geosparql`. This crate now acts as a thin re-export plus the
//! optional `bridge` and `spatial_index` modules.

pub use spareval::geosparql::GEOSPARQL_EXTENSION_FUNCTIONS;
pub mod vocab;

#[cfg(feature = "bridge")]
pub mod bridge;
#[cfg(feature = "spatial_index")]
pub mod index;
