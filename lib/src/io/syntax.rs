/// A file serialization format.
///
/// Is implemented by `GraphSyntax` for graph files and `DatasetSyntax` for dataset files.
#[deprecated(note = "Use directly the methods on the implementing types")]
pub trait FileSyntax: Sized {
    /// Its canonical IRI according to the [Unique URIs for file formats registry](https://www.w3.org/ns/formats/).
    fn iri(self) -> &'static str;

    /// Its [IANA media type](https://tools.ietf.org/html/rfc2046).
    fn media_type(self) -> &'static str;

    /// Its [IANA-registered](https://tools.ietf.org/html/rfc2046) file extension.
    fn file_extension(self) -> &'static str;

    /// Looks for a known syntax from a media type.
    fn from_mime_type(media_type: &str) -> Option<Self>;
}

/// [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) serialization formats.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum GraphSyntax {
    /// [N-Triples](https://www.w3.org/TR/n-triples/)
    NTriples,
    /// [Turtle](https://www.w3.org/TR/turtle/)
    Turtle,
    /// [RDF XML](https://www.w3.org/TR/rdf-syntax-grammar/)
    RdfXml,
}

impl GraphSyntax {
    /// The syntax canonical IRI according to the [Unique URIs for file formats registry](https://www.w3.org/ns/formats/).
    ///
    /// ```
    /// use oxigraph::io::GraphSyntax;
    ///
    /// assert_eq!(GraphSyntax::NTriples.iri(), "http://www.w3.org/ns/formats/N-Triples")
    /// ```
    pub fn iri(self) -> &'static str {
        match self {
            GraphSyntax::NTriples => "http://www.w3.org/ns/formats/N-Triples",
            GraphSyntax::Turtle => "http://www.w3.org/ns/formats/Turtle",
            GraphSyntax::RdfXml => "http://www.w3.org/ns/formats/RDF_XML",
        }
    }

    /// The syntax [IANA media type](https://tools.ietf.org/html/rfc2046).
    ///
    /// ```
    /// use oxigraph::io::GraphSyntax;
    ///
    /// assert_eq!(GraphSyntax::NTriples.media_type(), "application/n-triples")
    /// ```
    pub fn media_type(self) -> &'static str {
        match self {
            GraphSyntax::NTriples => "application/n-triples",
            GraphSyntax::Turtle => "text/turtle",
            GraphSyntax::RdfXml => "application/rdf+xml",
        }
    }

    /// The syntax [IANA-registered](https://tools.ietf.org/html/rfc2046) file extension.
    ///
    /// ```
    /// use oxigraph::io::GraphSyntax;
    ///
    /// assert_eq!(GraphSyntax::NTriples.file_extension(), "nt")
    /// ```
    pub fn file_extension(self) -> &'static str {
        match self {
            GraphSyntax::NTriples => "nt",
            GraphSyntax::Turtle => "ttl",
            GraphSyntax::RdfXml => "rdf",
        }
    }
    /// Looks for a known syntax from a media type.
    ///
    /// It supports some media type aliases.
    /// For example "application/xml" is going to return `GraphSyntax::RdfXml` even if it is not its canonical media type.
    ///
    /// Example:
    /// ```
    /// use oxigraph::io::GraphSyntax;
    ///
    /// assert_eq!(GraphSyntax::from_media_type("text/turtle; charset=utf-8"), Some(GraphSyntax::Turtle))
    /// ```
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        if let Some(base_type) = media_type.split(';').next() {
            match base_type.trim() {
                "application/n-triples" | "text/plain" => Some(GraphSyntax::NTriples),
                "text/turtle" | "application/turtle" | "application/x-turtle" => {
                    Some(GraphSyntax::Turtle)
                }
                "application/rdf+xml" | "application/xml" | "text/xml" => Some(GraphSyntax::RdfXml),
                _ => None,
            }
        } else {
            None
        }
    }
}

#[allow(deprecated)]
impl FileSyntax for GraphSyntax {
    fn iri(self) -> &'static str {
        self.iri()
    }

    fn media_type(self) -> &'static str {
        self.media_type()
    }

    fn file_extension(self) -> &'static str {
        self.file_extension()
    }

    fn from_mime_type(media_type: &str) -> Option<Self> {
        Self::from_media_type(media_type)
    }
}

/// [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) serialization formats.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum DatasetSyntax {
    /// [N-Quads](https://www.w3.org/TR/n-quads/)
    NQuads,
    /// [TriG](https://www.w3.org/TR/trig/)
    TriG,
}

impl DatasetSyntax {
    /// The syntax canonical IRI according to the [Unique URIs for file formats registry](https://www.w3.org/ns/formats/).
    ///
    /// ```
    /// use oxigraph::io::DatasetSyntax;
    ///
    /// assert_eq!(DatasetSyntax::NQuads.iri(), "http://www.w3.org/ns/formats/N-Quads")
    /// ```
    pub fn iri(self) -> &'static str {
        match self {
            DatasetSyntax::NQuads => "http://www.w3.org/ns/formats/N-Quads",
            DatasetSyntax::TriG => "http://www.w3.org/ns/formats/TriG",
        }
    }

    /// The syntax [IANA media type](https://tools.ietf.org/html/rfc2046).
    ///
    /// ```
    /// use oxigraph::io::DatasetSyntax;
    ///
    /// assert_eq!(DatasetSyntax::NQuads.media_type(), "application/n-quads")
    /// ```
    pub fn media_type(self) -> &'static str {
        match self {
            DatasetSyntax::NQuads => "application/n-quads",
            DatasetSyntax::TriG => "application/trig",
        }
    }

    /// The syntax [IANA-registered](https://tools.ietf.org/html/rfc2046) file extension.
    ///
    /// ```
    /// use oxigraph::io::DatasetSyntax;
    ///
    /// assert_eq!(DatasetSyntax::NQuads.file_extension(), "nq")
    /// ```
    pub fn file_extension(self) -> &'static str {
        match self {
            DatasetSyntax::NQuads => "nq",
            DatasetSyntax::TriG => "trig",
        }
    }
    /// Looks for a known syntax from a media type.
    ///
    /// It supports some media type aliases.
    ///
    /// Example:
    /// ```
    /// use oxigraph::io::DatasetSyntax;
    ///
    /// assert_eq!(DatasetSyntax::from_media_type("application/n-quads; charset=utf-8"), Some(DatasetSyntax::NQuads))
    /// ```
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        if let Some(base_type) = media_type.split(';').next() {
            match base_type.trim() {
                "application/n-quads" | "text/x-nquads" | "text/nquads" => {
                    Some(DatasetSyntax::NQuads)
                }
                "application/trig" | "application/x-trig" => Some(DatasetSyntax::TriG),
                _ => None,
            }
        } else {
            None
        }
    }
}

#[allow(deprecated)]
impl FileSyntax for DatasetSyntax {
    fn iri(self) -> &'static str {
        self.iri()
    }

    fn media_type(self) -> &'static str {
        self.media_type()
    }

    fn file_extension(self) -> &'static str {
        self.file_extension()
    }

    fn from_mime_type(media_type: &str) -> Option<Self> {
        Self::from_media_type(media_type)
    }
}
