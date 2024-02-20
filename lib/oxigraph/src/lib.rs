#![doc = include_str!("../README.md")]
#![doc(test(attr(deny(warnings))))]
#![doc(test(attr(allow(deprecated))))]
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

pub mod io;
pub mod model;
pub mod sparql;
mod storage;
pub mod store;
