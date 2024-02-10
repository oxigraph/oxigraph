use crate::algebra::*;
use crate::parser::{parse_query, SparqlSyntaxError};
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
/// assert_eq!(
///     query.to_sse(),
///     "(project (?s ?p ?o) (bgp (triple ?s ?p ?o)))"
/// );
/// # Ok::<_, spargebra::SparqlSyntaxError>(())
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
    pub fn parse(query: &str, base_iri: Option<&str>) -> Result<Self, SparqlSyntaxError> {
        parse_query(query, base_iri)
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub fn to_sse(&self) -> String {
        let mut buffer = String::new();
        self.fmt_sse(&mut buffer).unwrap();
        buffer
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        match self {
            Self::Select {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    write!(f, "(base <{base_iri}> ")?;
                }
                if let Some(dataset) = dataset {
                    f.write_str("(dataset ")?;
                    dataset.fmt_sse(f)?;
                    f.write_str(" ")?;
                }
                pattern.fmt_sse(f)?;
                if dataset.is_some() {
                    f.write_str(")")?;
                }
                if base_iri.is_some() {
                    f.write_str(")")?;
                }
                Ok(())
            }
            Self::Construct {
                template,
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    write!(f, "(base <{base_iri}> ")?;
                }
                f.write_str("(construct (")?;
                for (i, t) in template.iter().enumerate() {
                    if i > 0 {
                        f.write_str(" ")?;
                    }
                    t.fmt_sse(f)?;
                }
                f.write_str(") ")?;
                if let Some(dataset) = dataset {
                    f.write_str("(dataset ")?;
                    dataset.fmt_sse(f)?;
                    f.write_str(" ")?;
                }
                pattern.fmt_sse(f)?;
                if dataset.is_some() {
                    f.write_str(")")?;
                }
                f.write_str(")")?;
                if base_iri.is_some() {
                    f.write_str(")")?;
                }
                Ok(())
            }
            Self::Describe {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    write!(f, "(base <{base_iri}> ")?;
                }
                f.write_str("(describe ")?;
                if let Some(dataset) = dataset {
                    f.write_str("(dataset ")?;
                    dataset.fmt_sse(f)?;
                    f.write_str(" ")?;
                }
                pattern.fmt_sse(f)?;
                if dataset.is_some() {
                    f.write_str(")")?;
                }
                f.write_str(")")?;
                if base_iri.is_some() {
                    f.write_str(")")?;
                }
                Ok(())
            }
            Self::Ask {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    write!(f, "(base <{base_iri}> ")?;
                }
                f.write_str("(ask ")?;
                if let Some(dataset) = dataset {
                    f.write_str("(dataset ")?;
                    dataset.fmt_sse(f)?;
                    f.write_str(" ")?;
                }
                pattern.fmt_sse(f)?;
                if dataset.is_some() {
                    f.write_str(")")?;
                }
                f.write_str(")")?;
                if base_iri.is_some() {
                    f.write_str(")")?;
                }
                Ok(())
            }
        }
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Select {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{base_iri}>")?;
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
            Self::Construct {
                template,
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{base_iri}>")?;
                }
                f.write_str("CONSTRUCT { ")?;
                for triple in template {
                    write!(f, "{triple} . ")?;
                }
                f.write_str("}")?;
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
            Self::Describe {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{}>", base_iri.as_str())?;
                }
                f.write_str("DESCRIBE *")?;
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
            Self::Ask {
                dataset,
                pattern,
                base_iri,
            } => {
                if let Some(base_iri) = base_iri {
                    writeln!(f, "BASE <{base_iri}>")?;
                }
                f.write_str("ASK")?;
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
    type Err = SparqlSyntaxError;

    fn from_str(query: &str) -> Result<Self, Self::Err> {
        Self::parse(query, None)
    }
}

impl<'a> TryFrom<&'a str> for Query {
    type Error = SparqlSyntaxError;

    fn try_from(query: &str) -> Result<Self, Self::Error> {
        Self::from_str(query)
    }
}

impl<'a> TryFrom<&'a String> for Query {
    type Error = SparqlSyntaxError;

    fn try_from(query: &String) -> Result<Self, Self::Error> {
        Self::from_str(query)
    }
}
