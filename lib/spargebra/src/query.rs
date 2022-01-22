use crate::algebra::*;
use crate::parser::{parse_query, ParseError};
use crate::term::*;
use oxiri::Iri;
use std::fmt;
use std::str::FromStr;

/// A parsed [SPARQL query](https://www.w3.org/TR/sparql11-query/).
///
/// ```
/// use spargebra::Query;
///
/// let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
/// let query = Query::parse(query_str, None)?;
/// assert_eq!(query.to_string(), query_str);
/// assert_eq!(query.to_sse(), "(project (?s ?p ?o) (bgp (triple ?s ?p ?o)))");
/// # Result::Ok::<_, spargebra::ParseError>(())
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub enum Query {
    /// [SELECT](https://www.w3.org/TR/sparql11-query/#select).
    Select {
        /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset).
        dataset: Option<QueryDataset>,
        /// The query selection graph pattern.
        pattern: GraphPattern,
        /// The query base IRI.
        base_iri: Option<Iri<String>>,
    },
    /// [CONSTRUCT](https://www.w3.org/TR/sparql11-query/#construct).
    Construct {
        /// The query construction template.
        template: Vec<TriplePattern>,
        /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset).
        dataset: Option<QueryDataset>,
        /// The query selection graph pattern.
        pattern: GraphPattern,
        /// The query base IRI.
        base_iri: Option<Iri<String>>,
    },
    /// [DESCRIBE](https://www.w3.org/TR/sparql11-query/#describe).
    Describe {
        /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset).
        dataset: Option<QueryDataset>,
        /// The query selection graph pattern.
        pattern: GraphPattern,
        /// The query base IRI.
        base_iri: Option<Iri<String>>,
    },
    /// [ASK](https://www.w3.org/TR/sparql11-query/#ask).
    Ask {
        /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset).
        dataset: Option<QueryDataset>,
        /// The query selection graph pattern.
        pattern: GraphPattern,
        /// The query base IRI.
        base_iri: Option<Iri<String>>,
    },
}

impl Query {
    /// Parses a SPARQL query with an optional base IRI to resolve relative IRIs in the query.
    pub fn parse(query: &str, base_iri: Option<&str>) -> Result<Self, ParseError> {
        parse_query(query, base_iri)
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub fn to_sse(&self) -> String {
        let mut buffer = String::new();
        self.fmt_sse(&mut buffer)
            .expect("Unexpected error during SSE formatting");
        buffer
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Query::Select {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    write!(f, "(base <{}> ", base_iri)?;
                }
                if let Some(dataset) = dataset {
                    write!(f, "(dataset ")?;
                    dataset.fmt_sse(f)?;
                    write!(f, " ")?;
                }
                pattern.fmt_sse(f)?;
                if dataset.is_some() {
                    write!(f, ")")?;
                }
                if base_iri.is_some() {
                    write!(f, ")")?;
                }
                Ok(())
            }
            Query::Construct {
                template,
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    write!(f, "(base <{}> ", base_iri)?;
                }
                write!(f, "(construct (")?;
                for (i, t) in template.iter().enumerate() {
                    if i > 0 {
                        write!(f, " ")?;
                    }
                    t.fmt_sse(f)?;
                }
                write!(f, ") ")?;
                if let Some(dataset) = dataset {
                    write!(f, "(dataset ")?;
                    dataset.fmt_sse(f)?;
                    write!(f, " ")?;
                }
                pattern.fmt_sse(f)?;
                if dataset.is_some() {
                    write!(f, ")")?;
                }
                write!(f, ")")?;
                if base_iri.is_some() {
                    write!(f, ")")?;
                }
                Ok(())
            }
            Query::Describe {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    write!(f, "(base <{}> ", base_iri)?;
                }
                write!(f, "(describe ")?;
                if let Some(dataset) = dataset {
                    write!(f, "(dataset ")?;
                    dataset.fmt_sse(f)?;
                    write!(f, " ")?;
                }
                pattern.fmt_sse(f)?;
                if dataset.is_some() {
                    write!(f, ")")?;
                }
                write!(f, ")")?;
                if base_iri.is_some() {
                    write!(f, ")")?;
                }
                Ok(())
            }
            Query::Ask {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    write!(f, "(base <{}> ", base_iri)?;
                }
                write!(f, "(ask ")?;
                if let Some(dataset) = dataset {
                    write!(f, "(dataset ")?;
                    dataset.fmt_sse(f)?;
                    write!(f, " ")?;
                }
                pattern.fmt_sse(f)?;
                if dataset.is_some() {
                    write!(f, ")")?;
                }
                write!(f, ")")?;
                if base_iri.is_some() {
                    write!(f, ")")?;
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Query::Select {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{}>", base_iri)?;
                }
                write!(
                    f,
                    "{}",
                    SparqlGraphRootPattern {
                        pattern,
                        dataset: dataset.as_ref()
                    }
                )
            }
            Query::Construct {
                template,
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{}>", base_iri)?;
                }
                write!(f, "CONSTRUCT {{ ")?;
                for triple in template.iter() {
                    write!(f, "{} . ", triple)?;
                }
                write!(f, "}}")?;
                if let Some(dataset) = dataset {
                    dataset.fmt(f)?;
                }
                write!(
                    f,
                    " WHERE {{ {} }}",
                    SparqlGraphRootPattern {
                        pattern,
                        dataset: None
                    }
                )
            }
            Query::Describe {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{}>", base_iri.as_str())?;
                }
                write!(f, "DESCRIBE *")?;
                if let Some(dataset) = dataset {
                    dataset.fmt(f)?;
                }
                write!(
                    f,
                    " WHERE {{ {} }}",
                    SparqlGraphRootPattern {
                        pattern,
                        dataset: None
                    }
                )
            }
            Query::Ask {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{}>", base_iri)?;
                }
                write!(f, "ASK")?;
                if let Some(dataset) = dataset {
                    dataset.fmt(f)?;
                }
                write!(
                    f,
                    " WHERE {{ {} }}",
                    SparqlGraphRootPattern {
                        pattern,
                        dataset: None
                    }
                )
            }
        }
    }
}

impl FromStr for Query {
    type Err = ParseError;

    fn from_str(query: &str) -> Result<Self, ParseError> {
        Self::parse(query, None)
    }
}

impl<'a> TryFrom<&'a str> for Query {
    type Error = ParseError;

    fn try_from(query: &str) -> Result<Self, ParseError> {
        Self::from_str(query)
    }
}

impl<'a> TryFrom<&'a String> for Query {
    type Error = ParseError;

    fn try_from(query: &String) -> Result<Self, ParseError> {
        Self::from_str(query)
    }
}
