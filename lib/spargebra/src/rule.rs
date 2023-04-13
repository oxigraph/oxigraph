#![cfg_attr(not(feature = "rules"), allow(dead_code))]
use crate::algebra::*;
use crate::parser::{parse_rule_set, ParseError};
use crate::term::*;
use std::fmt;
use std::str::FromStr;

/// A parsed if/then rule set.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
}

impl RuleSet {
    /// Parses a set of rules with an optional base IRI to resolve relative IRIs in the rules.
    /// Note that this base IRI will not be used during execution.
    pub fn parse(rules: &str, base_iri: Option<&str>) -> Result<Self, ParseError> {
        parse_rule_set(rules, base_iri)
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
        write!(f, "(")?;
        for (i, r) in self.rules.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            r.fmt_sse(f)?;
        }
        write!(f, ") ")
    }
}

impl fmt::Display for RuleSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for r in &self.rules {
            writeln!(f, "{r} ;")?;
        }
        Ok(())
    }
}

impl FromStr for RuleSet {
    type Err = ParseError;

    fn from_str(rules: &str) -> Result<Self, ParseError> {
        Self::parse(rules, None)
    }
}

impl<'a> TryFrom<&'a str> for RuleSet {
    type Error = ParseError;

    fn try_from(rules: &str) -> Result<Self, ParseError> {
        Self::from_str(rules)
    }
}

impl<'a> TryFrom<&'a String> for RuleSet {
    type Error = ParseError;

    fn try_from(rules: &String) -> Result<Self, ParseError> {
        Self::from_str(rules)
    }
}

/// A parsed if/then rule.
#[derive(Eq, PartialEq, Debug, Clone, Hash)]
pub struct Rule {
    /// The construction template.
    pub head: Vec<GroundTriplePattern>,
    /// The rule body graph pattern.
    pub body: GraphPattern,
}

impl Rule {
    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    pub fn to_sse(&self) -> String {
        let mut buffer = String::new();
        self.fmt_sse(&mut buffer)
            .expect("Unexpected error during SSE formatting");
        buffer
    }

    /// Formats using the [SPARQL S-Expression syntax](https://jena.apache.org/documentation/notes/sse.html).
    fn fmt_sse(&self, f: &mut impl fmt::Write) -> fmt::Result {
        write!(f, "(rule (")?;
        for (i, t) in self.head.iter().enumerate() {
            if i > 0 {
                write!(f, " ")?;
            }
            t.fmt_sse(f)?;
        }
        write!(f, ") ")?;
        self.body.fmt_sse(f)?;
        write!(f, ")")
    }
}

impl fmt::Display for Rule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "IF {{ {} }} THEN {{ ",
            SparqlGraphRootPattern {
                pattern: &self.body,
                dataset: None
            }
        )?;
        for triple in self.head.iter() {
            write!(f, "{triple} . ")?;
        }
        write!(f, "}}")
    }
}
