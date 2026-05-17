//! Lexer for the SPARQL grammar i.e. ALL_CAPS rules

use chumsky::span::{SimpleSpan, Spanned, WrappingSpan};
use logos::Logos;
use std::fmt;

pub fn lex_sparql(slice: &str) -> Vec<Spanned<Token<'_>>> {
    Token::lexer(slice)
        .spanned()
        .map(|(token, span)| {
            SimpleSpan::from(span.clone())
                .make_wrapped(token.unwrap_or_else(|()| Token::Error(&slice[span])))
        })
        .collect()
}

#[derive(Logos, Debug, Clone, Copy, PartialEq, Eq)]
#[logos(skip r"([ \t\n\r]|#[^\r\n]*)+")]
pub enum Token<'a> {
    #[cfg_attr(
        feature = "standard-unicode-escaping",
        regex("<[^<>\"{}|^`\\\\\u{00}-\u{20}]*>")
    )]
    #[cfg_attr(
        not(feature = "standard-unicode-escaping"),
        regex("<([^<>\"{}|^`\\\\\u{00}-\u{20}]|\\\\u[0-9a-fA-F]{4}|\\\\U[0-9a-fA-F]{8})*>")
    )]
    IriRef(&'a str),
    #[regex(
        "([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}]([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}.]*[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}])?)?:([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_:0-9]|%[0-9A-Fa-f]{2}|\\\\[_\\~.\\-!$&'()*+,;=/?#@%])(([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}.:]|%[0-9A-Fa-f]{2}|\\\\[_\\~.\\-!$&'()*+,;=/?#@%])*([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}:]|%[0-9A-Fa-f]{2}|\\\\[_\\~.\\-!$&'()*+,;=/?#@%]))?"
    )]
    PnameLn(&'a str),
    #[regex(
        "([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}]([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}.]*[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}])?)?:"
    )]
    PnameNs(&'a str),
    #[regex(
        "_:[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_0-9]([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}.]*[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}])?"
    )]
    BlankNodeLabel(&'a str),
    #[cfg_attr(
        feature = "standard-unicode-escaping",
        regex("'([^'\\\\\r\n]|\\\\[tbnrf\\\\\"'])*'")
    )]
    #[cfg_attr(
        not(feature = "standard-unicode-escaping"),
        regex("'([^'\\\\\r\n]|\\\\[tbnrf\\\\\"']|\\\\u[0-9a-fA-F]{4}|\\\\U[0-9a-fA-F]{8})*'")
    )]
    StringLiteral1(&'a str),
    #[cfg_attr(
        feature = "standard-unicode-escaping",
        regex("\"([^\"\\\\\r\n]|\\\\[tbnrf\\\\\"'])*\"")
    )]
    #[cfg_attr(
        not(feature = "standard-unicode-escaping"),
        regex("\"([^\"\\\\\r\n]|\\\\[tbnrf\\\\\"']|\\\\u[0-9a-fA-F]{4}|\\\\U[0-9a-fA-F]{8})*\"")
    )]
    StringLiteral2(&'a str),
    #[cfg_attr(
        feature = "standard-unicode-escaping",
        regex("'''('{0,2}([^'\\\\]|\\\\[tbnrf\\\\\"']))*'''")
    )]
    #[cfg_attr(
        not(feature = "standard-unicode-escaping"),
        regex(
            "'''('{0,2}([^'\\\\]|\\\\[tbnrf\\\\\"']|\\\\u[0-9a-fA-F]{4}|\\\\U[0-9a-fA-F]{8}))*'''"
        )
    )]
    StringLiteralLong1(&'a str),
    #[cfg_attr(
        feature = "standard-unicode-escaping",
        regex("\"\"\"(\"{0,2}([^\"\\\\]|\\\\[tbnrf\\\\\"']))*\"\"\"")
    )]
    #[cfg_attr(
        not(feature = "standard-unicode-escaping"),
        regex(
            "\"\"\"(\"{0,2}([^\"\\\\]|\\\\[tbnrf\\\\\"']|\\\\u[0-9a-fA-F]{4}|\\\\U[0-9a-fA-F]{8}))*\"\"\""
        )
    )]
    StringLiteralLong2(&'a str),
    #[regex("@[a-zA-Z]+(-[a-zA-Z0-9]+)*(--[a-zA-Z]+)?")]
    LangDir(&'a str),
    #[regex("[0-9]+")]
    Integer(&'a str),
    #[regex("[0-9]*\\.[0-9]+")]
    Decimal(&'a str),
    #[regex("(([0-9]+(\\.[0-9]*)?)|(\\.[0-9]+))[eE][+\\-]?[0-9]+")]
    Double(&'a str),
    #[regex("\\+[0-9]+")]
    IntegerPositive(&'a str),
    #[regex("\\+[0-9]*\\.[0-9]+")]
    DecimalPositive(&'a str),
    #[regex("\\+(([0-9]+(\\.[0-9]*)?)|(\\.[0-9]+))[eE][+\\-]?[0-9]+")]
    DoublePositive(&'a str),
    #[regex("-[0-9]+")]
    IntegerNegative(&'a str),
    #[regex("-[0-9]*\\.[0-9]+")]
    DecimalNegative(&'a str),
    #[regex("-(([0-9]+(\\.[0-9]*)?)|(\\.[0-9]+))[eE][+\\-]?[0-9]+")]
    DoubleNegative(&'a str),
    #[regex(
        "\\?[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_0-9][A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}]*"
    )]
    Var1(&'a str),
    #[regex(
        "\\$[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_0-9][A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}]*"
    )]
    Var2(&'a str),
    #[regex("[a-zA-Z][a-zA-Z0-9]*(_[a-zA-Z0-9]+)*")]
    Keyword(&'a str),
    #[token("<<(")]
    #[token(")>>")]
    #[token("||")]
    #[token("&&")]
    #[token("^^")]
    #[token("<=")]
    #[token(">=")]
    #[token("!=")]
    #[token("<<")]
    #[token(">>")]
    #[token("{|")]
    #[token("|}")]
    #[token("{")]
    #[token("}")]
    #[token("[")]
    #[token("]")]
    #[token("(")]
    #[token(")")]
    #[token(";")]
    #[token(",")]
    #[token(".")]
    #[token("+")]
    #[token("-")]
    #[token("*")]
    #[token("/")]
    #[token("<")]
    #[token(">")]
    #[token("=")]
    #[token("!")]
    #[token("^")]
    #[token("|")]
    #[token("~")]
    #[token("?")]
    Operator(&'a str),
    Error(&'a str),
}

impl fmt::Display for Token<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::IriRef(v)
            | Self::PnameLn(v)
            | Self::PnameNs(v)
            | Self::BlankNodeLabel(v)
            | Self::StringLiteral1(v)
            | Self::StringLiteral2(v)
            | Self::StringLiteralLong1(v)
            | Self::StringLiteralLong2(v)
            | Self::LangDir(v)
            | Self::Var1(v)
            | Self::Var2(v)
            | Self::Integer(v)
            | Self::Decimal(v)
            | Self::Double(v)
            | Self::IntegerPositive(v)
            | Self::DecimalPositive(v)
            | Self::DoublePositive(v)
            | Self::IntegerNegative(v)
            | Self::DecimalNegative(v)
            | Self::DoubleNegative(v)
            | Self::Keyword(v)
            | Self::Operator(v)
            | Self::Error(v) => f.write_str(v),
        }
    }
}
