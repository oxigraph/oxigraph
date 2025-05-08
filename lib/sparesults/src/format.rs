use std::fmt;

/// [SPARQL query](https://www.w3.org/TR/sparql11-query/) results serialization formats.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[non_exhaustive]
pub enum QueryResultsFormat {
    /// [SPARQL Query Results XML Format](https://www.w3.org/TR/rdf-sparql-XMLres/)
    Xml,
    /// [SPARQL Query Results JSON Format](https://www.w3.org/TR/sparql11-results-json/)
    Json,
    /// [SPARQL Query Results CSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/)
    Csv,
    /// [SPARQL Query Results TSV Format](https://www.w3.org/TR/sparql11-results-csv-tsv/)
    Tsv,
}

impl QueryResultsFormat {
    /// The format canonical IRI according to the [Unique URIs for file formats registry](https://www.w3.org/ns/formats/).
    ///
    /// ```
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(
    ///     QueryResultsFormat::Json.iri(),
    ///     "http://www.w3.org/ns/formats/SPARQL_Results_JSON"
    /// )
    /// ```
    #[inline]
    pub fn iri(self) -> &'static str {
        match self {
            Self::Xml => "http://www.w3.org/ns/formats/SPARQL_Results_XML",
            Self::Json => "http://www.w3.org/ns/formats/SPARQL_Results_JSON",
            Self::Csv => "http://www.w3.org/ns/formats/SPARQL_Results_CSV",
            Self::Tsv => "http://www.w3.org/ns/formats/SPARQL_Results_TSV",
        }
    }

    /// The format [IANA media type](https://tools.ietf.org/html/rfc2046).
    ///
    /// ```
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(
    ///     QueryResultsFormat::Json.media_type(),
    ///     "application/sparql-results+json"
    /// )
    /// ```
    #[inline]
    pub fn media_type(self) -> &'static str {
        match self {
            Self::Xml => "application/sparql-results+xml",
            Self::Json => "application/sparql-results+json",
            Self::Csv => "text/csv; charset=utf-8",
            Self::Tsv => "text/tab-separated-values; charset=utf-8",
        }
    }

    /// The format [IANA-registered](https://tools.ietf.org/html/rfc2046) file extension.
    ///
    /// ```
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::Json.file_extension(), "srj")
    /// ```
    #[inline]
    pub fn file_extension(self) -> &'static str {
        match self {
            Self::Xml => "srx",
            Self::Json => "srj",
            Self::Csv => "csv",
            Self::Tsv => "tsv",
        }
    }

    /// The format name.
    ///
    /// ```
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(QueryResultsFormat::Json.name(), "SPARQL Results in JSON")
    /// ```
    #[inline]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Xml => "SPARQL Results in XML",
            Self::Json => "SPARQL Results in JSON",
            Self::Csv => "SPARQL Results in CSV",
            Self::Tsv => "SPARQL Results in TSV",
        }
    }

    /// Looks for a known format from a media type.
    ///
    /// It supports some media type aliases.
    /// For example, "application/xml" is going to return `Xml` even if it is not its canonical media type.
    ///
    /// Example:
    /// ```
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(
    ///     QueryResultsFormat::from_media_type("application/sparql-results+json; charset=utf-8"),
    ///     Some(QueryResultsFormat::Json)
    /// )
    /// ```
    #[inline]
    pub fn from_media_type(media_type: &str) -> Option<Self> {
        const MEDIA_SUBTYPES: [(&str, QueryResultsFormat); 8] = [
            ("csv", QueryResultsFormat::Csv),
            ("json", QueryResultsFormat::Json),
            ("plain", QueryResultsFormat::Csv),
            ("sparql-results+json", QueryResultsFormat::Json),
            ("sparql-results+xml", QueryResultsFormat::Xml),
            ("tab-separated-values", QueryResultsFormat::Tsv),
            ("tsv", QueryResultsFormat::Tsv),
            ("xml", QueryResultsFormat::Xml),
        ];

        let (r#type, subtype) = media_type
            .split_once(';')
            .unwrap_or((media_type, ""))
            .0
            .trim()
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
    /// use sparesults::QueryResultsFormat;
    ///
    /// assert_eq!(
    ///     QueryResultsFormat::from_extension("json"),
    ///     Some(QueryResultsFormat::Json)
    /// )
    /// ```
    #[inline]
    pub fn from_extension(extension: &str) -> Option<Self> {
        const EXTENSIONS: [(&str, QueryResultsFormat); 7] = [
            ("csv", QueryResultsFormat::Csv),
            ("json", QueryResultsFormat::Json),
            ("srj", QueryResultsFormat::Json),
            ("srx", QueryResultsFormat::Xml),
            ("tsv", QueryResultsFormat::Tsv),
            ("txt", QueryResultsFormat::Csv),
            ("xml", QueryResultsFormat::Xml),
        ];
        for (candidate_extension, candidate_id) in EXTENSIONS {
            if candidate_extension.eq_ignore_ascii_case(extension) {
                return Some(candidate_id);
            }
        }
        None
    }
}

impl fmt::Display for QueryResultsFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}
