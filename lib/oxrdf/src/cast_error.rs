use std::{fmt, str};


use crate::{NamedNode, Subject, Term};

// An error return if trying to cast a term as something it cannot be converted to.
#[derive(Debug, Clone, thiserror::Error)]
#[error("{term} can not be converted to a {target}")]
pub struct TryFromTermError {
    pub(crate) term: Term,
    pub(crate) target: &'static str
}

impl From<TryFromTermError> for Term {
    #[inline]
    fn from(error: TryFromTermError) -> Self {
        error.term
    }
}

// An error return if trying to construct an invalid triple.
#[derive(Debug, thiserror::Error)]
pub struct TripleConstructionError {
    pub(crate) subject: Result<Subject, TryFromTermError>,
    pub(crate) predicate: Result<NamedNode, TryFromTermError>,
    pub(crate) object: Term,
}

impl fmt::Display for TripleConstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.subject.clone().err(), self.predicate.clone().err()) {
            (Some(e), Some(e2)) => write!(f, "subject: [{}], predicate: [{}]", e, e2),
            (Some(e), _) => write!(f, "subject: [{}]", e),
            (_, Some(e)) => write!(f, "predicate: [{}]", e),
            _ => write!(f, "object: {}", self.object)
        }
    }
}
