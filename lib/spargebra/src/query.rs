//! Data structures around SPARQL queries. The main type is [`Query`].

use crate::SparqlParser;
use crate::algebra::*;
use crate::error::SparqlSyntaxError;
use crate::term::*;
use oxiri::Iri;
use std::fmt;
use std::str::FromStr;

/// A parsed [SPARQL query](https://www.w3.org/TR/sparql11-query/).
///
/// ```
/// use spargebra::SparqlParser;
///
/// let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
/// let query = SparqlParser::new().parse_query(query_str)?;
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
    Select(SelectQuery),
    /// [CONSTRUCT](https://www.w3.org/TR/sparql11-query/#construct).
    Construct(ConstructQuery),
    /// [DESCRIBE](https://www.w3.org/TR/sparql11-query/#describe).
    Describe(DescribeQuery),
    /// [ASK](https://www.w3.org/TR/sparql11-query/#ask).
    Ask(AskQuery),
}

impl Query {
    #[inline]
    pub fn dataset(&self) -> Option<&QueryDataset> {
        match self {
            Self::Select(query) => query.dataset.as_ref(),
            Self::Construct(query) => query.dataset.as_ref(),
            Self::Describe(query) => query.dataset.as_ref(),
            Self::Ask(query) => query.dataset.as_ref(),
        }
    }

    #[inline]
    pub fn dataset_mut(&mut self) -> Option<&mut QueryDataset> {
        match self {
            Self::Select(query) => query.dataset.as_mut(),
            Self::Construct(query) => query.dataset.as_mut(),
            Self::Describe(query) => query.dataset.as_mut(),
            Self::Ask(query) => query.dataset.as_mut(),
        }
    }

    #[inline]
    pub fn base_iri(&self) -> Option<&Iri<String>> {
        match self {
            Self::Select(query) => query.base_iri.as_ref(),
            Self::Construct(query) => query.base_iri.as_ref(),
            Self::Describe(query) => query.base_iri.as_ref(),
            Self::Ask(query) => query.base_iri.as_ref(),
        }
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
            Self::Select(query) => query.fmt_sse(f),
            Self::Construct(query) => query.fmt_sse(f),
            Self::Describe(query) => query.fmt_sse(f),
            Self::Ask(query) => query.fmt_sse(f),
        }
    }
}

impl fmt::Display for Query {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Select(query) => query.fmt(f),
            Self::Construct(query) => query.fmt(f),
            Self::Describe(query) => query.fmt(f),
            Self::Ask(query) => query.fmt(f),
        }
    }
}

impl FromStr for Query {
    type Err = SparqlSyntaxError;

    fn from_str(query: &str) -> Result<Self, Self::Err> {
        SparqlParser::new().parse_query(query)
    }
}

impl TryFrom<&str> for Query {
    type Error = SparqlSyntaxError;

    fn try_from(query: &str) -> Result<Self, Self::Error> {
        Self::from_str(query)
    }
}

impl TryFrom<&String> for Query {
    type Error = SparqlSyntaxError;

    fn try_from(query: &String) -> Result<Self, Self::Error> {
        Self::from_str(query)
    }
}

/// A parsed  [SELECT SPARQL query](https://www.w3.org/TR/sparql11-query/#select).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct SelectQuery {
    /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset).
    pub dataset: Option<QueryDataset>,
    /// The query selection graph pattern.
    pub pattern: GraphPattern,
    /// The query base IRI.
    pub base_iri: Option<Iri<String>>,
}

impl SelectQuery {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub fn to_sse(&self) -> String {
        let mut buffer = String::new();
        self.fmt_sse(&mut buffer).unwrap();
        buffer
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            write!(f, "(base <{base_iri}> ")?;
        }
        if let Some(dataset) = &self.dataset {
            f.write_str("(dataset ")?;
            dataset.fmt_sse(f)?;
            f.write_str(" ")?;
        }
        self.pattern.fmt_sse(f)?;
        if self.dataset.is_some() {
            f.write_str(")")?;
        }
        if self.base_iri.is_some() {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl fmt::Display for SelectQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            writeln!(f, "BASE <{base_iri}>")?;
        }
        SparqlGraphRootPattern::new(&self.pattern, self.dataset.as_ref())?.fmt(f)
    }
}

impl From<SelectQuery> for Query {
    #[inline]
    fn from(query: SelectQuery) -> Self {
        Self::Select(query)
    }
}

/// A parsed [CONSTRUCT SPARQL query](https://www.w3.org/TR/sparql11-query/#construct).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct ConstructQuery {
    /// The query construction template.
    pub template: Vec<TriplePattern>,
    /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset).
    pub dataset: Option<QueryDataset>,
    /// The query selection graph pattern.
    pub pattern: GraphPattern,
    /// The query base IRI.
    pub base_iri: Option<Iri<String>>,
}

impl ConstructQuery {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub fn to_sse(&self) -> String {
        let mut buffer = String::new();
        self.fmt_sse(&mut buffer).unwrap();
        buffer
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            write!(f, "(base <{base_iri}> ")?;
        }
        f.write_str("(construct (")?;
        for (i, t) in self.template.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            t.fmt_sse(f)?;
        }
        f.write_str(") ")?;
        if let Some(dataset) = &self.dataset {
            f.write_str("(dataset ")?;
            dataset.fmt_sse(f)?;
            f.write_str(" ")?;
        }
        self.pattern.fmt_sse(f)?;
        if self.dataset.is_some() {
            f.write_str(")")?;
        }
        f.write_str(")")?;
        if self.base_iri.is_some() {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl fmt::Display for ConstructQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            writeln!(f, "BASE <{base_iri}>")?;
        }
        f.write_str("CONSTRUCT { ")?;
        for triple in &self.template {
            write!(f, "{triple} . ")?;
        }
        f.write_str("}")?;
        if let Some(dataset) = &self.dataset {
            write!(f, " {dataset}")?;
        }
        let mut pattern = &self.pattern;
        // We ignore the root projection, it's useless
        if let GraphPattern::Project { inner, .. } = pattern {
            pattern = inner;
        }
        write!(f, " WHERE {{ {pattern} }}")
    }
}

impl From<ConstructQuery> for Query {
    #[inline]
    fn from(query: ConstructQuery) -> Self {
        Self::Construct(query)
    }
}

/// A parsed [DESCRIBE SPARQL query](https://www.w3.org/TR/sparql11-query/#describe).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct DescribeQuery {
    /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset).
    pub dataset: Option<QueryDataset>,
    /// The query selection graph pattern.
    pub pattern: GraphPattern,
    /// The query base IRI.
    pub base_iri: Option<Iri<String>>,
}

impl DescribeQuery {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub fn to_sse(&self) -> String {
        let mut buffer = String::new();
        self.fmt_sse(&mut buffer).unwrap();
        buffer
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            write!(f, "(base <{base_iri}> ")?;
        }
        f.write_str("(describe ")?;
        if let Some(dataset) = &self.dataset {
            f.write_str("(dataset ")?;
            dataset.fmt_sse(f)?;
            f.write_str(" ")?;
        }
        self.pattern.fmt_sse(f)?;
        if self.dataset.is_some() {
            f.write_str(")")?;
        }
        f.write_str(")")?;
        if self.base_iri.is_some() {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl fmt::Display for DescribeQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            writeln!(f, "BASE <{}>", base_iri.as_str())?;
        }

        // We find the DESCRIBE IRIs
        let mut pattern = &self.pattern;
        let mut iris = Vec::new();
        while let GraphPattern::Extend {
            inner,
            expression: Expression::NamedNode(iri),
            ..
        } = pattern
        {
            iris.push(iri);
            pattern = inner;
        }

        // If there is a projection, we can inline it too
        let mut select_variables = Vec::new();
        if let GraphPattern::Project { inner, variables } = pattern {
            select_variables.extend(variables);
            pattern = inner;
        } else {
            pattern.on_in_scope_variable(|v| {
                if !select_variables.contains(&v) {
                    select_variables.push(v);
                }
            })
        }

        f.write_str("DESCRIBE ")?;
        if select_variables.is_empty() && iris.is_empty() {
            writeln!(f, " *")?;
        }
        for variable in select_variables {
            write!(f, " {variable}")?;
        }
        for iri in iris.iter().rev() {
            // reverse to keep the same order as the parsing
            write!(f, " {iri}")?;
        }
        if let Some(dataset) = &self.dataset {
            dataset.fmt(f)?;
        }
        write!(f, " WHERE {{ {pattern} }}")
    }
}

impl From<DescribeQuery> for Query {
    #[inline]
    fn from(query: DescribeQuery) -> Self {
        Self::Describe(query)
    }
}

/// A parsed [ASK SPARQL query](https://www.w3.org/TR/sparql11-query/#ask).
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct AskQuery {
    /// The [query dataset specification](https://www.w3.org/TR/sparql11-query/#specifyingDataset).
    pub dataset: Option<QueryDataset>,
    /// The query selection graph pattern.
    pub pattern: GraphPattern,
    /// The query base IRI.
    pub base_iri: Option<Iri<String>>,
}

impl AskQuery {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub fn to_sse(&self) -> String {
        let mut buffer = String::new();
        self.fmt_sse(&mut buffer).unwrap();
        buffer
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            write!(f, "(base <{base_iri}> ")?;
        }
        f.write_str("(ask ")?;
        if let Some(dataset) = &self.dataset {
            f.write_str("(dataset ")?;
            dataset.fmt_sse(f)?;
            f.write_str(" ")?;
        }
        self.pattern.fmt_sse(f)?;
        if self.dataset.is_some() {
            f.write_str(")")?;
        }
        f.write_str(")")?;
        if self.base_iri.is_some() {
            f.write_str(")")?;
        }
        Ok(())
    }
}

impl fmt::Display for AskQuery {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(base_iri) = &self.base_iri {
            writeln!(f, "BASE <{base_iri}>")?;
        }
        f.write_str("ASK")?;
        if let Some(dataset) = &self.dataset {
            write!(f, " {dataset}")?;
        }
        let mut pattern = &self.pattern;
        // We ignore the root projection, it's useless
        if let GraphPattern::Project { inner, .. } = pattern {
            pattern = inner;
        }
        write!(f, " WHERE {{ {pattern} }}")
    }
}

impl From<AskQuery> for Query {
    #[inline]
    fn from(query: AskQuery) -> Self {
        Self::Ask(query)
    }
}
