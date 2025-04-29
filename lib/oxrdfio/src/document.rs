use crate::RdfFormat;

/// A remote document fetched to support parsing.
///
/// This is mostly used to retrieve JSON-LD remote contexts.
pub struct LoadedDocument {
    /// Final URL of the remote document after possible redirections and normalizations.
    pub url: String,
    /// Content of the document.
    pub content: Vec<u8>,
    /// Format of the document.
    pub format: RdfFormat,
}
