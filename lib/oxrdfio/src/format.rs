use std::fmt;

/// RDF serialization formats.
///
/// This enumeration is non exhaustive. New formats like JSON-LD might be added in the future.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[non_exhaustive]
pub enum RdfFormat {
    /// [N3](https://w3c.github.io/N3/spec/)
    N3,
    /// [N-Quads](https://www.w3.org/TR/n-quads/)
    NQuads,
    /// [N-Triples](https://www.w3.org/TR/n-triples/)
    NTriples,
    /// [RDF/XML](https://www.w3.org/TR/rdf-syntax-grammar/)
    RdfXml,
    /// [TriG](https://www.w3.org/TR/trig/)
    TriG,
    /// [Turtle](https://www.w3.org/TR/turtle/)
    Turtle,
}

impl RdfFormat {
    /// The format canonical IRI according to the [Unique URIs for file formats registry](https://www.w3.org/ns/formats/).
    ///
    /// ```
    /// use oxrdfio::RdfFormat;
    ///
    /// assert_eq!(
    ///     RdfFormat::NTriples.iri(),
    ///     "http://www.w3.org/ns/formats/N-Triples"
    /// )
    /// ```
    #[inline]
    pub const fn iri(self) -> &'static str {
        match self {
            Self::N3 => "http://www.w3.org/ns/formats/N3",
            Self::NQuads => "http://www.w3.org/ns/formats/N-Quads",
            Self::NTriples => "http://www.w3.org/ns/formats/N-Triples",
            Self::RdfXml => "http://www.w3.org/ns/formats/RDF_XML",
            Self::TriG => "http://www.w3.org/ns/formats/TriG",
            Self::Turtle => "http://www.w3.org/ns/formats/Turtle",
        }
    }

    /// The format [IANA media type](https://tools.ietf.org/html/rfc2046).
    ///
    /// ```
    /// use oxrdfio::RdfFormat;
    ///
    /// assert_eq!(RdfFormat::NTriples.media_type(), "application/n-triples")
    /// ```
    #[inline]
    pub const fn media_type(self) -> &'static str {
        match self {
            Self::N3 => "text/n3",
            Self::NQuads => "application/n-quads",
            Self::NTriples => "application/n-triples",
            Self::RdfXml => "application/rdf+xml",
            Self::TriG => "application/trig",
            Self::Turtle => "text/turtle",
        }
    }

    /// The format [IANA-registered](https://tools.ietf.org/html/rfc2046) file extension.
    ///
    /// ```
    /// use oxrdfio::RdfFormat;
    ///
    /// assert_eq!(RdfFormat::NTriples.file_extension(), "nt")
    /// ```
    #[inline]
    pub const fn file_extension(self) -> &'static str {
        match self {
            Self::N3 => "n3",
            Self::NQuads => "nq",
            Self::NTriples => "nt",
            Self::RdfXml => "rdf",
            Self::TriG => "trig",
            Self::Turtle => "ttl",
        }
    }

    /// The format name.
    ///
    /// ```
    /// use oxrdfio::RdfFormat;
    ///
    /// assert_eq!(RdfFormat::NTriples.name(), "N-Triples")
    /// ```
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::N3 => "N3",
            Self::NQuads => "N-Quads",
            Self::NTriples => "N-Triples",
            Self::RdfXml => "RDF/XML",
            Self::TriG => "TriG",
            Self::Turtle => "Turtle",
        }
    }

    /// Checks if the formats supports [RDF datasets](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-dataset) and not only [RDF graphs](https://www.w3.org/TR/rdf11-concepts/#dfn-rdf-graph).
    ///
    /// ```
    /// use oxrdfio::RdfFormat;
    ///
    /// assert_eq!(RdfFormat::NTriples.supports_datasets(), false);
    /// assert_eq!(RdfFormat::NQuads.supports_datasets(), true);
    /// ```
    #[inline]
    pub const fn supports_datasets(self) -> bool {
        matches!(self, Self::NQuads | Self::TriG)
    }

    /// Checks if the formats supports [RDF-star quoted triples](https://w3c.github.io/rdf-star/cg-spec/2021-12-17.html#dfn-quoted).
    ///
    /// ```
    /// use oxrdfio::RdfFormat;
    ///
    /// assert_eq!(RdfFormat::NTriples.supports_rdf_star(), true);
    /// assert_eq!(RdfFormat::RdfXml.supports_rdf_star(), false);
    /// ```
    #[inline]
    #[cfg(feature = "rdf-star")]
    pub const fn supports_rdf_star(self) -> bool {
        matches!(
            self,
            Self::NTriples | Self::NQuads | Self::Turtle | Self::TriG
        )
    }

    /// Looks for a known format from a media type.
    ///
    /// It supports some media type aliases.
    /// For example, "application/xml" is going to return `RdfFormat::RdfXml` even if it is not its canonical media type.
    ///
    /// Example:
    /// ```
    /// use oxrdfio::RdfFormat;
    ///
    /// assert_eq!(
    ///     RdfFormat::from_media_type("text/turtle; charset=utf-8"),
    ///     Some(RdfFormat::Turtle)
    /// )
    /// ```
    #[inline]
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        const MEDIA_SUBTYPES: [(&str, RdfFormat); 10] = [
            ("n-quads", RdfFormat::NQuads),
            ("n-triples", RdfFormat::NTriples),
            ("n3", RdfFormat::N3),
            ("nquads", RdfFormat::NQuads),
            ("ntriples", RdfFormat::NTriples),
            ("plain", RdfFormat::NTriples),
            ("rdf+xml", RdfFormat::RdfXml),
            ("trig", RdfFormat::TriG),
            ("turtle", RdfFormat::Turtle),
            ("xml", RdfFormat::RdfXml),
        ];

        let (r#type, subtype) = media_type
            .split_once(';')
            .unwrap_or((media_type, ""))
            .0
            .split_once('/')?;
        let r#type = r#type.trim();
        if !r#type.eq_ignore_ascii_case("application") && !r#type.eq_ignore_ascii_case("text") {
            return None;
        }
        let subtype = subtype.trim();
        let subtype = subtype.strip_prefix("x-").unwrap_or(subtype);
        for (candidate_subtype, candidate_id) in MEDIA_SUBTYPES {
            if candidate_subtype.eq_ignore_ascii_case(subtype) {
                return Some(candidate_id);
            }
        }
        None
    }

    /// Looks for a known format from an extension.
    ///
    /// It supports some aliases.
    ///
    /// Example:
    /// ```
    /// use oxrdfio::RdfFormat;
    ///
    /// assert_eq!(RdfFormat::from_extension("nt"), Some(RdfFormat::NTriples))
    /// ```
    #[inline]
    pub fn from_extension(extension: &str) -> Option<Self> {
        const MEDIA_TYPES: [(&str, RdfFormat); 8] = [
            ("n3", RdfFormat::N3),
            ("nq", RdfFormat::NQuads),
            ("nt", RdfFormat::NTriples),
            ("rdf", RdfFormat::RdfXml),
            ("trig", RdfFormat::TriG),
            ("ttl", RdfFormat::Turtle),
            ("txt", RdfFormat::NTriples),
            ("xml", RdfFormat::RdfXml),
        ];
        for (candidate_extension, candidate_id) in MEDIA_TYPES {
            if candidate_extension.eq_ignore_ascii_case(extension) {
                return Some(candidate_id);
            }
        }
        None
    }
}

impl fmt::Display for RdfFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
