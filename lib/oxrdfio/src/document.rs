use crate::RdfFormat;
use std::error::Error;
use std::panic::{RefUnwindSafe, UnwindSafe};

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

/// Callback used in [`ReaderQuadParser::with_document_loader`](crate::ReaderQuadParser::with_document_loader)
pub trait DocumentLoader: Send + Sync + UnwindSafe + RefUnwindSafe + 'static {
    type Error: Error + Send + Sync;

    fn load(
        &self,
        url: &str,
        accepted_formats: &[RdfFormat],
    ) -> Result<LoadedDocument, Self::Error>;
}

impl<
    E: Error + Send + Sync,
    F: Fn(&str, &[RdfFormat]) -> Result<LoadedDocument, E>
        + Send
        + Sync
        + UnwindSafe
        + RefUnwindSafe
        + 'static,
> DocumentLoader for F
{
    type Error = E;

    #[inline]
    fn load(
        &self,
        url: &str,
        accepted_formats: &[RdfFormat],
    ) -> Result<LoadedDocument, Self::Error> {
        self(url, accepted_formats)
    }
}
