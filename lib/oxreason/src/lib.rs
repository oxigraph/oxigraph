#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

//! OWL 2 RL reasoning and SHACL validation for Oxigraph.
//!
//! This crate is an early scaffold tracking
//! [oxigraph issue #130](https://github.com/oxigraph/oxigraph/issues/130).
//! All public types compile and are documented, but evaluation methods
//! currently return [`ReasonError::NotImplemented`] or
//! [`ValidateError::NotImplemented`]. See `DESIGN.md` next to this file for
//! the plan, rule coverage, and milestones.

mod engine;
mod error;
mod reasoner;
mod rules;
mod shacl;

pub use crate::error::{ReasonError, ValidateError};
pub use crate::reasoner::{Reasoner, ReasonerConfig, ReasoningProfile, ReasoningReport};
pub use crate::rules::{Rule, RuleId, RuleSet};
pub use crate::shacl::{
    Severity, ValidationReport, ValidationResult, Validator, ValidatorConfig,
};
