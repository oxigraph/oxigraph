#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

//! OWL 2 RL reasoning and SHACL Core validation for Oxigraph.
//!
//! [`Reasoner`] applies OWL 2 RL property and class rules over an
//! [`oxrdf::Graph`] using semi-naive forward chaining and emits the closure
//! either back into the graph or through a streaming sink. [`Validator`]
//! checks SHACL Core constraints against a data graph and produces a
//! validation report.
//!
//! See `DESIGN.md` next to this file for the rule coverage matrix, the
//! semi-naive evaluation model, and the public API contract.

mod engine;
mod error;
mod reasoner;
mod rules;
mod shacl;

pub use crate::error::{ReasonError, ReasonStreamError, ValidateError};
pub use crate::reasoner::{Reasoner, ReasonerConfig, ReasoningProfile, ReasoningReport};
pub use crate::rules::{Rule, RuleId, RuleSet};
pub use crate::shacl::{Severity, ValidationReport, ValidationResult, Validator, ValidatorConfig};
