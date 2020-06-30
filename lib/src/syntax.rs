/// A file serialization format.
///
/// Is implemented by `GraphSyntax` for graph files and `DatasetSyntax` for dataset files.
pub trait FileSyntax: Sized {
    /// Its canonical IRI according to [Unique URIs for file formats registry](https://www.w3.org/ns/formats/).
    fn iri(self) -> &'static str;

    /// Its [IANA media type](https://tools.ietf.org/html/rfc2046).
    fn media_type(self) -> &'static str;

    /// Its [IANA-registered](https://tools.ietf.org/html/rfc2046) file extension.
    fn file_extension(self) -> &'static str;

    /// Looks for a known syntax from a media type.
    ///
    /// Example:
    /// ```
    /// use oxigraph::{GraphSyntax, FileSyntax};
    /// assert_eq!(GraphSyntax::from_mime_type("text/turtle; charset=utf-8"), Some(GraphSyntax::Turtle))
    /// ```
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

impl FileSyntax for GraphSyntax {
    fn iri(self) -> &'static str {
        match self {
            GraphSyntax::NTriples => "http://www.w3.org/ns/formats/N-Triples",
            GraphSyntax::Turtle => "http://www.w3.org/ns/formats/Turtle",
            GraphSyntax::RdfXml => "http://www.w3.org/ns/formats/RDF_XML",
        }
    }

    fn media_type(self) -> &'static str {
        match self {
            GraphSyntax::NTriples => "application/n-triples",
            GraphSyntax::Turtle => "text/turtle",
            GraphSyntax::RdfXml => "application/rdf+xml",
        }
    }

    fn file_extension(self) -> &'static str {
        match self {
            GraphSyntax::NTriples => "nt",
            GraphSyntax::Turtle => "ttl",
            GraphSyntax::RdfXml => "rdf",
        }
    }

    fn from_mime_type(media_type: &str) -> Option<Self> {
        if let Some(base_type) = media_type.split(';').next() {
            match base_type {
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

/// [RDF dataset](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) serialization formats.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
pub enum DatasetSyntax {
    /// [N-Quads](https://www.w3.org/TR/n-quads/)
    NQuads,
    /// [TriG](https://www.w3.org/TR/trig/)
    TriG,
}

impl FileSyntax for DatasetSyntax {
    fn iri(self) -> &'static str {
        match self {
            DatasetSyntax::NQuads => "http://www.w3.org/ns/formats/N-Quads",
            DatasetSyntax::TriG => "http://www.w3.org/ns/formats/TriG",
        }
    }

    fn media_type(self) -> &'static str {
        match self {
            DatasetSyntax::NQuads => "application/n-quads",
            DatasetSyntax::TriG => "application/trig",
        }
    }

    fn file_extension(self) -> &'static str {
        match self {
            DatasetSyntax::NQuads => "nq",
            DatasetSyntax::TriG => "trig",
        }
    }

    fn from_mime_type(media_type: &str) -> Option<Self> {
        if let Some(base_type) = media_type.split(';').next() {
            match base_type {
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
