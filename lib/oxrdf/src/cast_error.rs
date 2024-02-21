use std::error::Error;

/// An error return if trying to cast a term as something it cannot be converted to.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct TermCastError(#[from] pub TermCastErrorKind);

/// An error return if trying to cast a term as something it cannot be converted to.
#[derive(Debug, thiserror::Error)]
pub enum TermCastErrorKind {
    #[error("{0}")]
    Msg(String),
    #[error("{0}")]
    Other(#[source] Box<dyn Error + Send + Sync + 'static>),
}
