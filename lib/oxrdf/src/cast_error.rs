use std::error::Error;

/// An error return if some content in the database is corrupted.
#[derive(Debug, thiserror::Error)]
#[error(transparent)]
pub struct TermCastError(#[from] pub TermCastErrorKind);

/// An error return if some content in the database is corrupted.
#[derive(Debug, thiserror::Error)]
pub enum TermCastErrorKind {
    #[error("{0}")]
    Msg(String),
    #[error("{0}")]
    Other(#[source] Box<dyn Error + Send + Sync + 'static>),
}
