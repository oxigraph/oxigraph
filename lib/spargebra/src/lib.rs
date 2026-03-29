#![cfg_attr(doc, doc = include_str!("../README.md"))]
#![doc(test(attr(deny(warnings))))]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![doc(html_favicon_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]
#![doc(html_logo_url = "https://raw.githubusercontent.com/oxigraph/oxigraph/main/logo.svg")]

pub mod algebra;
mod algebra_builder;
mod ast;
mod error;
#[cfg(feature = "standard-unicode-escaping")]
mod escaping;
mod lexer;
mod parser;
mod query;
pub mod term;
mod update;

use crate::algebra_builder::AlgebraBuilder;
pub use crate::error::{SparqlSyntaxError, TextPosition};
#[cfg(feature = "standard-unicode-escaping")]
use crate::escaping::unescape_unicode_codepoints;
use crate::lexer::lex_sparql;
use crate::parser::{parse_sparql_query, parse_sparql_update};
use oxiri::{Iri, IriParseError};
use oxrdf::NamedNode;
pub use query::*;
use std::collections::{HashMap, HashSet};
pub use update::*;

/// A SPARQL parser
///
/// ```
/// use spargebra::SparqlParser;
///
/// let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
/// let query = SparqlParser::new().parse_query(query_str)?;
/// assert_eq!(query.to_string(), query_str);
/// # Ok::<_, spargebra::SparqlSyntaxError>(())
/// ```
#[must_use]
#[derive(Clone, Default)]
pub struct SparqlParser {
    base_iri: Option<Iri<String>>,
    prefixes: HashMap<String, String>,
    custom_aggregate_functions: HashSet<NamedNode>,
}

impl SparqlParser {
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Provides an IRI that could be used to resolve the operation relative IRIs.
    ///
    /// ```
    /// use spargebra::SparqlParser;
    ///
    /// let query = SparqlParser::new().with_base_iri("http://example.com/")?.parse_query("SELECT * WHERE { <s> <p> <o> }")?;
    /// assert_eq!(query.to_string(), "BASE <http://example.com/>\nSELECT * WHERE { <http://example.com/s> <http://example.com/p> <http://example.com/o> . }");
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_base_iri(mut self, base_iri: impl Into<String>) -> Result<Self, IriParseError> {
        self.base_iri = Some(Iri::parse(base_iri.into())?);
        Ok(self)
    }

    /// Set a default IRI prefix used during parsing.
    ///
    /// ```
    /// use spargebra::SparqlParser;
    ///
    /// let query = SparqlParser::new()
    ///     .with_prefix("ex", "http://example.com/")?
    ///     .parse_query("SELECT * WHERE { ex:s ex:p ex:o }")?;
    /// assert_eq!(
    ///     query.to_string(),
    ///     "SELECT * WHERE { <http://example.com/s> <http://example.com/p> <http://example.com/o> . }"
    /// );
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_prefix(
        mut self,
        prefix_name: impl Into<String>,
        prefix_iri: impl Into<String>,
    ) -> Result<Self, IriParseError> {
        self.prefixes.insert(
            prefix_name.into(),
            Iri::parse(prefix_iri.into())?.into_inner(),
        );
        Ok(self)
    }

    /// Adds a new function to be parsed as a custom aggregate function and not as a regular custom function.
    ///
    /// ```
    /// use oxrdf::NamedNode;
    /// use spargebra::SparqlParser;
    ///
    /// SparqlParser::new()
    ///     .with_custom_aggregate_function(NamedNode::new("http://example.com/concat")?)
    ///     .parse_query(
    ///         "PREFIX ex: <http://example.com/> SELECT (ex:concat(?o) AS ?concat) WHERE { ex:s ex:p ex:o }",
    ///     )?;
    /// # Result::<_, Box<dyn std::error::Error>>::Ok(())
    /// ```
    #[inline]
    pub fn with_custom_aggregate_function(mut self, name: impl Into<NamedNode>) -> Self {
        self.custom_aggregate_functions.insert(name.into());
        self
    }

    /// Parse the given query string using the already set options.
    ///
    /// ```
    /// use spargebra::SparqlParser;
    ///
    /// let query_str = "SELECT ?s ?p ?o WHERE { ?s ?p ?o . }";
    /// let query = SparqlParser::new().parse_query(query_str)?;
    /// assert_eq!(query.to_string(), query_str);
    /// # Ok::<_, spargebra::SparqlSyntaxError>(())
    /// ```
    #[cfg_attr(
        not(feature = "standard-unicode-escaping"),
        expect(clippy::needless_borrow)
    )]
    pub fn parse_query(&self, query: &str) -> Result<Query, SparqlSyntaxError> {
        #[cfg(feature = "standard-unicode-escaping")]
        let query = unescape_unicode_codepoints(query);
        let tokens = lex_sparql(&query);
        let ast = parse_sparql_query(&tokens, query.len())
            .map_err(|e| SparqlSyntaxError::from_chumsky(e, &query))?;
        AlgebraBuilder::new(
            self.base_iri.clone(),
            self.prefixes.clone(),
            &self.custom_aggregate_functions,
        )
        .build_query(ast)
        .map_err(|e| SparqlSyntaxError::from_algebra_builder(e, &query))
    }

    /// Parse the given update string using the already set options.
    ///
    /// ```
    /// use spargebra::SparqlParser;
    ///
    /// let update_str = "CLEAR ALL ;";
    /// let update = SparqlParser::new().parse_update(update_str)?;
    /// assert_eq!(update.to_string().trim(), update_str);
    /// # Ok::<_, spargebra::SparqlSyntaxError>(())
    /// ```
    #[cfg_attr(
        not(feature = "standard-unicode-escaping"),
        expect(clippy::needless_borrow)
    )]
    pub fn parse_update(&self, update: &str) -> Result<Update, SparqlSyntaxError> {
        #[cfg(feature = "standard-unicode-escaping")]
        let update = unescape_unicode_codepoints(update);
        let tokens = lex_sparql(&update);
        let ast = parse_sparql_update(&tokens, update.len())
            .map_err(|e| SparqlSyntaxError::from_chumsky(e, &update))?;
        AlgebraBuilder::new(
            self.base_iri.clone(),
            self.prefixes.clone(),
            &self.custom_aggregate_functions,
        )
        .build_update(ast)
        .map_err(|e| SparqlSyntaxError::from_algebra_builder(e, &update))
    }
}
