use oxjsonld::{JsonLdProfile, JsonLdProfileSet};
use std::fmt;

/// RDF serialization formats.
///
/// This enumeration is non exhaustive. New formats might be added in the future.
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
    /// [JSON-LD](https://www.w3.org/TR/json-ld/)
    JsonLd { profile: JsonLdProfileSet },
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
            Self::JsonLd { .. } => "https://www.w3.org/ns/formats/data/JSON-LD",
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
            Self::JsonLd { profile } => {
                // TODO: more combinations
                if profile.contains(JsonLdProfile::Streaming) {
                    "application/ld+json;profile=http://www.w3.org/ns/json-ld#streaming"
                } else {
                    "application/ld+json"
                }
            }
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
            Self::JsonLd { .. } => "jsonld",
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
            Self::JsonLd { profile } => {
                // TODO: more combinations
                if profile.contains(JsonLdProfile::Streaming) {
                    "Streaming JSON-LD"
                } else {
                    "JSON-LD"
                }
            }
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
        matches!(self, Self::JsonLd { .. } | Self::NQuads | Self::TriG)
    }

    #[deprecated(note = "All format will soon support RDF 1.2", since = "0.2.0")]
    #[inline]
    #[cfg(feature = "rdf-12")]
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
    /// use oxjsonld::JsonLdProfile;
    /// use oxrdfio::RdfFormat;
    ///
    /// assert_eq!(
    ///     RdfFormat::from_media_type("text/turtle; charset=utf-8"),
    ///     Some(RdfFormat::Turtle)
    /// );
    /// assert_eq!(
    ///     RdfFormat::from_media_type(
    ///         "application/ld+json ; profile = http://www.w3.org/ns/json-ld#streaming"
    ///     ),
    ///     Some(RdfFormat::JsonLd {
    ///         profile: JsonLdProfile::Streaming.into()
    ///     })
    /// )
    /// ```
    #[inline]
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        const MEDIA_SUBTYPES: [(&str, RdfFormat); 14] = [
            (
                "activity+json",
                RdfFormat::JsonLd {
                    profile: JsonLdProfileSet::empty(),
                },
            ),
            (
                "json",
                RdfFormat::JsonLd {
                    profile: JsonLdProfileSet::empty(),
                },
            ),
            (
                "ld+json",
                RdfFormat::JsonLd {
                    profile: JsonLdProfileSet::empty(),
                },
            ),
            (
                "jsonld",
                RdfFormat::JsonLd {
                    profile: JsonLdProfileSet::empty(),
                },
            ),
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
        const UTF8_CHARSETS: [&str; 3] = ["ascii", "utf8", "utf-8"];

        let (type_subtype, parameters) = media_type.split_once(';').unwrap_or((media_type, ""));

        let (r#type, subtype) = type_subtype.split_once('/')?;
        let r#type = r#type.trim();
        if !r#type.eq_ignore_ascii_case("application") && !r#type.eq_ignore_ascii_case("text") {
            return None;
        }
        let subtype = subtype.trim();
        let subtype = subtype.strip_prefix("x-").unwrap_or(subtype);

        let parameters = parameters.trim();
        let parameters = if parameters.is_empty() {
            Vec::new()
        } else {
            parameters
                .split(';')
                .map(|p| {
                    let (key, value) = p.split_once('=')?;
                    Some((key.trim(), value.trim()))
                })
                .collect::<Option<Vec<_>>>()?
        };

        for (candidate_subtype, mut candidate_id) in MEDIA_SUBTYPES {
            if candidate_subtype.eq_ignore_ascii_case(subtype) {
                // We have a look at parameters
                for (key, mut value) in parameters {
                    match key {
                        "charset" => {
                            if !UTF8_CHARSETS.iter().any(|c| c.eq_ignore_ascii_case(value)) {
                                return None; // No other charset than UTF-8 is supported
                            }
                        }
                        "profile" => {
                            // We remove enclosing double quotes
                            if value.starts_with('"') && value.ends_with('"') {
                                value = &value[1..value.len() - 1];
                            }
                            if let RdfFormat::JsonLd { profile } = &mut candidate_id {
                                for value in value.split(' ') {
                                    if let Some(value) = JsonLdProfile::from_iri(value.trim()) {
                                        *profile |= value;
                                    }
                                }
                            }
                        }
                        _ => (), // We ignore
                    }
                }
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
        const EXTENSIONS: [(&str, RdfFormat); 10] = [
            (
                "json",
                RdfFormat::JsonLd {
                    profile: JsonLdProfileSet::empty(),
                },
            ),
            (
                "jsonld",
                RdfFormat::JsonLd {
                    profile: JsonLdProfileSet::empty(),
                },
            ),
            ("n3", RdfFormat::N3),
            ("nq", RdfFormat::NQuads),
            ("nt", RdfFormat::NTriples),
            ("rdf", RdfFormat::RdfXml),
            ("trig", RdfFormat::TriG),
            ("ttl", RdfFormat::Turtle),
            ("txt", RdfFormat::NTriples),
            ("xml", RdfFormat::RdfXml),
        ];
        for (candidate_extension, candidate_id) in EXTENSIONS {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_media_type() {
        assert_eq!(RdfFormat::from_media_type("foo/bar"), None);
        assert_eq!(RdfFormat::from_media_type("text/csv"), None);
        assert_eq!(
            RdfFormat::from_media_type("text/turtle"),
            Some(RdfFormat::Turtle)
        );
        assert_eq!(
            RdfFormat::from_media_type("application/x-turtle"),
            Some(RdfFormat::Turtle)
        );
        assert_eq!(
            RdfFormat::from_media_type("application/ld+json"),
            Some(RdfFormat::JsonLd {
                profile: JsonLdProfileSet::empty()
            })
        );
        assert_eq!(
            RdfFormat::from_media_type("application/ld+json;profile=foo"),
            Some(RdfFormat::JsonLd {
                profile: JsonLdProfileSet::empty()
            })
        );
        assert_eq!(
            RdfFormat::from_media_type(
                "application/ld+json;profile=http://www.w3.org/ns/json-ld#streaming"
            ),
            Some(RdfFormat::JsonLd {
                profile: JsonLdProfile::Streaming.into()
            })
        );
        assert_eq!(
            RdfFormat::from_media_type(
                "application/ld+json ; profile = \" http://www.w3.org/ns/json-ld#streaming  http://www.w3.org/ns/json-ld#expanded \" "
            ),
            Some(RdfFormat::JsonLd {
                profile: JsonLdProfile::Streaming | JsonLdProfile::Expanded
            })
        );
    }
}
