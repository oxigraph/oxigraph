use std::io;

#[derive(Debug, thiserror::Error)]
pub enum JellyParseError {
    #[error(transparent)]
    Io(#[from] io::Error),

    #[error(transparent)]
    Syntax(#[from] JellySyntaxError),
}

#[derive(Debug, thiserror::Error)]
pub enum JellySyntaxError {
    #[error(transparent)]
    Protobuf(#[from] protobuf::Error),

    #[error(transparent)]
    IriParseError(#[from] oxrdf::IriParseError),

    #[error(transparent)]
    BlankNodeIdParseError(#[from] oxrdf::BlankNodeIdParseError),

    #[error("Prefix ID not found: {0}")]
    PrefixIdNotFound(u32),

    #[error("Name ID not found: {0}")]
    NameIdNotFound(u32),

    #[error("Datatype ID not found: datatype_id = {0}")]
    DatatypeIdNotFound(u32),

    #[error("ID out of bounds: provided = {0}, maximum = {1}")]
    IdOutOfBounds(u32, u32),

    #[error("No previous subject")]
    NoPreviousSubject,

    #[error("No previous predicate")]
    NoPreviousPredicate,

    #[error("No previous object")]
    NoPreviousObject,

    #[error("No previous graph name")]
    NoPreviousGraphName,
}

impl From<JellySyntaxError> for io::Error {
    fn from(error: JellySyntaxError) -> Self {
        Self::new(io::ErrorKind::InvalidData, error)
    }
}