/// [RDF graph](https://www.w3.org/TR/rdf11-concepts/#dfn-graph) serialization formats.
///
/// This enumeration is non exhaustive. New formats like JSON-LD will be added in the future.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[non_exhaustive]
pub enum GraphFormat {
    /// [N-Triples](https://www.w3.org/TR/n-triples/)
    NTriples,
    /// [Turtle](https://www.w3.org/TR/turtle/)
    Turtle,
    /// [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/)
    RdfXml,
}

impl GraphFormat {
    /// The format canonical IRI according to the [Unique URIs for file formats registry](https://www.w3.org/ns/formats/).
    ///
    /// ```
    /// use oxigraph::io::GraphFormat;
    ///
    /// assert_eq!(GraphFormat::NTriples.iri(), "http://www.w3.org/ns/formats/N-Triples")
    /// ```
    #[inline]
    pub fn iri(self) -> &'static str {
        match self {
            GraphFormat::NTriples => "http://www.w3.org/ns/formats/N-Triples",
            GraphFormat::Turtle => "http://www.w3.org/ns/formats/Turtle",
            GraphFormat::RdfXml => "http://www.w3.org/ns/formats/RDF_XML",
        }
    }

    /// The format [IANA media type](https://tools.ietf.org/html/rfc2046).
    ///
    /// ```
    /// use oxigraph::io::GraphFormat;
    ///
    /// assert_eq!(GraphFormat::NTriples.media_type(), "application/n-triples")
    /// ```
    #[inline]
    pub fn media_type(self) -> &'static str {
        match self {
            GraphFormat::NTriples => "application/n-triples",
            GraphFormat::Turtle => "text/turtle",
            GraphFormat::RdfXml => "application/rdf+xml",
        }
    }

    /// The format [IANA-registered](https://tools.ietf.org/html/rfc2046) file extension.
    ///
    /// ```
    /// use oxigraph::io::GraphFormat;
    ///
    /// assert_eq!(GraphFormat::NTriples.file_extension(), "nt")
    /// ```
    #[inline]
    pub fn file_extension(self) -> &'static str {
        match self {
            GraphFormat::NTriples => "nt",
            GraphFormat::Turtle => "ttl",
            GraphFormat::RdfXml => "rdf",
        }
    }
    /// Looks for a known format from a media type.
    ///
    /// It supports some media type aliases.
    /// For example "application/xml" is going to return `GraphFormat::RdfXml` even if it is not its canonical media type.
    ///
    /// Example:
    /// ```
    /// use oxigraph::io::GraphFormat;
    ///
    /// assert_eq!(GraphFormat::from_media_type("text/turtle; charset=utf-8"), Some(GraphFormat::Turtle))
    /// ```
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        if let Some(base_type) = media_type.split(';').next() {
            match base_type.trim() {
                "application/n-triples" | "text/plain" => Some(GraphFormat::NTriples),
                "text/turtle" | "application/turtle" | "application/x-turtle" => {
                    Some(GraphFormat::Turtle)
                }
                "application/rdf+xml" | "application/xml" | "text/xml" => Some(GraphFormat::RdfXml),
                _ => None,
            }
        } else {
            None
        }
    }
}

/// [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) serialization formats.
///
/// This enumeration is non exhaustive. New formats like JSON-LD will be added in the future.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[non_exhaustive]
pub enum DatasetFormat {
    /// [N-Quads](https://www.w3.org/TR/n-quads/)
    NQuads,
    /// [TriG](https://www.w3.org/TR/trig/)
    TriG,
}

impl DatasetFormat {
    /// The format canonical IRI according to the [Unique URIs for file formats registry](https://www.w3.org/ns/formats/).
    ///
    /// ```
    /// use oxigraph::io::DatasetFormat;
    ///
    /// assert_eq!(DatasetFormat::NQuads.iri(), "http://www.w3.org/ns/formats/N-Quads")
    /// ```
    #[inline]
    pub fn iri(self) -> &'static str {
        match self {
            DatasetFormat::NQuads => "http://www.w3.org/ns/formats/N-Quads",
            DatasetFormat::TriG => "http://www.w3.org/ns/formats/TriG",
        }
    }

    /// The format [IANA media type](https://tools.ietf.org/html/rfc2046).
    ///
    /// ```
    /// use oxigraph::io::DatasetFormat;
    ///
    /// assert_eq!(DatasetFormat::NQuads.media_type(), "application/n-quads")
    /// ```
    #[inline]
    pub fn media_type(self) -> &'static str {
        match self {
            DatasetFormat::NQuads => "application/n-quads",
            DatasetFormat::TriG => "application/trig",
        }
    }

    /// The format [IANA-registered](https://tools.ietf.org/html/rfc2046) file extension.
    ///
    /// ```
    /// use oxigraph::io::DatasetFormat;
    ///
    /// assert_eq!(DatasetFormat::NQuads.file_extension(), "nq")
    /// ```
    #[inline]
    pub fn file_extension(self) -> &'static str {
        match self {
            DatasetFormat::NQuads => "nq",
            DatasetFormat::TriG => "trig",
        }
    }
    /// Looks for a known format from a media type.
    ///
    /// It supports some media type aliases.
    ///
    /// Example:
    /// ```
    /// use oxigraph::io::DatasetFormat;
    ///
    /// assert_eq!(DatasetFormat::from_media_type("application/n-quads; charset=utf-8"), Some(DatasetFormat::NQuads))
    /// ```
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        if let Some(base_type) = media_type.split(';').next() {
            match base_type.trim() {
                "application/n-quads" | "text/x-nquads" | "text/nquads" => {
                    Some(DatasetFormat::NQuads)
                }
                "application/trig" | "application/x-trig" => Some(DatasetFormat::TriG),
                _ => None,
            }
        } else {
            None
        }
    }
}
