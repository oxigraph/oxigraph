#[cfg(feature = "http-client")]
use crate::http::HttpClient;
use oxrdfio::{LoadedDocument, RdfFormat};
#[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
use std::fs;
#[cfg(feature = "http-client")]
use std::io::Read;
use std::io::{Error, ErrorKind, Result};
#[cfg(feature = "http-client")]
use std::time::Duration;
use url::Url;

/// A JSON-LD context loader to be used with [`ReaderQuadParser::with_document_loader`](oxrdfio::ReaderQuadParser::with_document_loader)
///
/// <div class="warning">Enabling the "file" or "http" features can have security consequences.
/// For example, if you accept JSON-LD document from untrusted users and enable the "fs" feature,
/// they might trigger reads of arbitrary files from the local system and see its content because of the returned errors.</div>
#[derive(Default, Clone)]
pub struct DocumentLoader {
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    file: bool,
    #[cfg(feature = "http-client")]
    http: Option<HttpClient>,
}

impl DocumentLoader {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Enable support of `file://` URLs
    #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
    #[must_use]
    #[inline]
    pub fn with_file_support(mut self) -> Self {
        self.file = true;
        self
    }

    /// Enable support of `http://` and `https://` URLs
    #[cfg(feature = "http-client")]
    #[must_use]
    #[inline]
    pub fn with_http_support(
        self,
        http_timeout: Option<Duration>,
        http_redirection_limit: usize,
    ) -> Self {
        self.with_http_client(HttpClient::new(http_timeout, http_redirection_limit))
    }

    /// Enable support of `http://` and `https://` URLs
    #[cfg(feature = "http-client")]
    #[must_use]
    #[inline]
    pub(crate) fn with_http_client(mut self, client: HttpClient) -> Self {
        self.http = Some(client);
        self
    }
}

impl oxrdfio::DocumentLoader for DocumentLoader {
    type Error = Error;

    #[cfg_attr(not(feature = "http-client"), expect(unused))]
    fn load(&self, url: &str, accepted_formats: &[RdfFormat]) -> Result<LoadedDocument> {
        let parsed_url = Url::parse(url).map_err(invalid_input_error)?;
        match parsed_url.scheme() {
            #[cfg(not(all(target_family = "wasm", target_os = "unknown")))]
            "file" => {
                if !self.file {
                    return Err(invalid_input_error(
                        "File URLs are not allowed, enable with_file_support option to allow them",
                    ));
                }
                let path = parsed_url
                    .to_file_path()
                    .map_err(|()| invalid_input_error(format!("Invalid file URL {url}")))?;
                let extension = path.extension().ok_or_else(|| invalid_input_error(format!("The file must have an extension for Oxigraph to guess its format, {} found", path.display())))?;
                Ok(LoadedDocument {
                    url: url.to_owned(),
                    content: fs::read(&path)?,
                    format: RdfFormat::from_extension(extension.to_str().ok_or_else(|| {
                        invalid_input_error(format!(
                            "Non UTF-8 extension '{}'",
                            extension.display()
                        ))
                    })?)
                    .ok_or_else(|| {
                        invalid_input_error(format!(
                            "Unsupported file extension '{}'",
                            extension.display()
                        ))
                    })?,
                })
            }
            #[cfg(feature = "http-client")]
            "http" | "https" => {
                let Some(client) = &self.http else {
                    return Err(invalid_input_error(
                        "HTTP(S) URLs are not allowed, enable with_http_support option to allow them",
                    ));
                };
                let accept = accepted_formats
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(", ");
                let (content_type, mut body) = client.get(url, &accept)?;
                let format = RdfFormat::from_media_type(&content_type).ok_or_else(|| {
                    invalid_data_error(format!("Unsupported content type '{content_type}'"))
                })?;
                let mut content = Vec::new();
                body.read_to_end(&mut content)?;
                Ok(LoadedDocument {
                    url: url.to_owned(),
                    content,
                    format,
                })
            }
            _ => Err(invalid_input_error(format!(
                "Unsupported context URL: {url}"
            ))),
        }
    }
}

#[cfg(feature = "http-client")]
fn invalid_data_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
    Error::new(ErrorKind::InvalidData, error)
}

fn invalid_input_error(error: impl Into<Box<dyn std::error::Error + Send + Sync>>) -> Error {
    Error::new(ErrorKind::InvalidInput, error)
}
