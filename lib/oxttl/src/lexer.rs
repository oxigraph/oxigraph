#![allow(clippy::range_plus_one)]

use crate::toolkit::{TokenRecognizer, TokenRecognizerError};
use logos::{Lexer, Logos};
use oxilangtag::LanguageTag;
use oxiri::Iri;
#[cfg(feature = "rdf-12")]
use oxrdf::BaseDirection;
use oxrdf::NamedNode;
use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::Range;
use std::str;

#[derive(Debug, PartialEq, Eq)]
pub enum N3Token<'a> {
    IriRef(Cow<'a, str>),
    PrefixedName {
        prefix: &'a str,
        local: Cow<'a, str>,
        might_be_invalid_iri: bool,
    },
    Variable(Cow<'a, str>),
    BlankNodeLabel(&'a str),
    String(Cow<'a, str>),
    LongString(Cow<'a, str>),
    Integer(&'a str),
    Decimal(&'a str),
    Double(&'a str),
    LangTag {
        language: &'a str,
        #[cfg(feature = "rdf-12")]
        direction: Option<BaseDirection>,
    },
    Punctuation(&'a str),
    PlainKeyword(&'a str),
}

#[derive(Logos, Debug, PartialEq, Eq, Clone, Copy)]
#[logos(skip r"([ \t\n\r]|#[^\r\n]*)+")]
#[logos(utf8 = false)]
pub enum N3LogosToken<'a> {
    #[regex("<([^<>\"{}|^`\\\\\u{00}-\u{20}]|\\\\u[0-9a-fA-F]{4}|\\\\U[0-9a-fA-F]{8})*>")]
    IriRefWithEscapes(&'a [u8]),
    #[regex("<([^<>\"{}|^`\\\\\u{00}-\u{20}])*>", priority = 10)]
    IriRefWithoutEscapes(&'a [u8]),
    #[regex(
        "([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}]([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}.]*[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}])?)?:(([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_:0-9]|%[0-9A-Fa-f]{2}|\\\\[_\\~.\\-!$&'()*+,;=/?#@%])(([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}.:]|%[0-9A-Fa-f]{2}|\\\\[_\\~.\\-!$&'()*+,;=/?#@%])*([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}:]|%[0-9A-Fa-f]{2}|\\\\[_\\~.\\-!$&'()*+,;=/?#@%]))?)?"
    )]
    PrefixedNameWithEscapes(&'a [u8]),
    #[regex(
        "([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}]([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}.]*[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}])?)?:(([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_:0-9]|%[0-9A-Fa-f]{2})(([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}.:]|%[0-9A-Fa-f]{2})*([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}:]|%[0-9A-Fa-f]{2}))?)?",
        priority = 10
    )]
    PrefixedNameWithoutEscapes(&'a [u8]),
    #[regex(
        "\\?[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_0-9][A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}]*"
    )]
    Variable(&'a [u8]),
    #[regex(
        "_:[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_0-9]([A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}.]*[A-Za-z\u{00C0}-\u{00D6}\u{00D8}-\u{00F6}\u{00F8}-\u{02FF}\u{0370}-\u{037D}\u{037F}-\u{1FFF}\u{200C}-\u{200D}\u{2070}-\u{218F}\u{2C00}-\u{2FEF}\u{3001}-\u{D7FF}\u{F900}-\u{FDCF}\u{FDF0}-\u{FFFD}\u{10000}-\u{EFFFF}_\\-0-9\u{00B7}\u{0300}-\u{036F}\u{203F}-\u{2040}])?"
    )]
    BlankNodeLabel(&'a [u8]),
    #[regex("'([^'\\\\\r\n]|\\\\[tbnrf\\\\\"']|\\\\u[0-9a-fA-F]{4}|\\\\U[0-9a-fA-F]{8})*'")]
    #[regex("\"([^\"\\\\\r\n]|\\\\[tbnrf\\\\\"']|\\\\u[0-9a-fA-F]{4}|\\\\U[0-9a-fA-F]{8})*\"")]
    String(&'a [u8]),
    #[regex("'''('{0,2}([^'\\\\]|\\\\[tbnrf\\\\\"']|\\\\u[0-9a-fA-F]{4}|\\\\U[0-9a-fA-F]{8}))*'''")]
    #[regex(
        "\"\"\"(\"{0,2}([^\"\\\\]|\\\\[tbnrf\\\\\"']|\\\\u[0-9a-fA-F]{4}|\\\\U[0-9a-fA-F]{8}))*\"\"\""
    )]
    LongString(&'a [u8]),
    #[regex("[+-]?[0-9]+")]
    Integer(&'a [u8]),
    #[regex("[+-]?[0-9]*\\.[0-9]+")]
    Decimal(&'a [u8]),
    #[regex("[+-]?(([0-9]+(\\.[0-9]*)?)|(\\.[0-9]+))[eE][+\\-]?[0-9]+")]
    Double(&'a [u8]),
    #[regex("@[a-zA-Z]+(-[a-zA-Z0-9]+)*(--[a-zA-Z]+)?")]
    LangTag(&'a [u8]),
    #[token("<<(")]
    #[token("<<")]
    #[token("<=")]
    #[token("<-")]
    #[token(">>")]
    #[token(".")]
    #[token("^^")]
    #[token("^")]
    #[token("(")]
    #[token(")>>")]
    #[token(")")]
    #[token("[")]
    #[token("]")]
    #[token("{")]
    #[token("}")]
    #[token("{|")]
    #[token(",")]
    #[token(";")]
    #[token("!")]
    #[token("|")]
    #[token("|}")]
    #[token("=>")]
    #[token("=")]
    #[token("~")]
    Punctuation(&'a [u8]),
    #[regex("[a-zA-Z][a-zA-Z0-9]*(_[a-zA-Z0-9]+)*")]
    PlainKeyword(&'a [u8]),
}

#[derive(Default)]
pub struct N3LexerOptions {
    pub base_iri: Option<Iri<String>>,
}

pub struct N3Lexer {
    lenient: bool,
}

impl TokenRecognizer for N3Lexer {
    type Token<'a> = N3Token<'a>;
    type Options = N3LexerOptions;

    fn recognize_next_token<'a>(
        &mut self,
        data: &'a [u8],
        is_ending: bool,
        options: &N3LexerOptions,
    ) -> Option<(usize, Result<N3Token<'a>, TokenRecognizerError>)> {
        let mut lexer = if is_ending {
            Lexer::new(data)
        } else {
            Lexer::new_prefix(data)
        };
        let token = lexer.next()?;
        let location = lexer.span();
        Some((
            location.end,
            match token {
                Ok(token) => self.convert_token(token, location, options),
                Err(()) => Err(TokenRecognizerError {
                    location,
                    message: format!(
                        "unexpected content '{}'",
                        String::from_utf8_lossy(lexer.slice())
                    ),
                }),
            },
        ))
    }
}

impl N3Lexer {
    pub fn new(lenient: bool) -> Self {
        Self { lenient }
    }

    fn convert_token<'a>(
        &self,
        token: N3LogosToken<'a>,
        span: Range<usize>,
        options: &N3LexerOptions,
    ) -> Result<N3Token<'a>, TokenRecognizerError> {
        Ok(match token {
            N3LogosToken::IriRefWithEscapes(iri) => {
                let span = span.start + 1..span.end - 1;
                let iri = str_from_utf8(&iri[1..iri.len() - 1]);
                let iri = unescape_string(iri, span.clone())?;
                self.parse_iri(iri, span, options)?
            }
            N3LogosToken::IriRefWithoutEscapes(iri) => {
                let span = span.start + 1..span.end - 1;
                let iri = str_from_utf8(&iri[1..iri.len() - 1]);
                self.parse_iri(Cow::Borrowed(iri), span, options)?
            }
            N3LogosToken::PrefixedNameWithEscapes(pname) => {
                let pname = str_from_utf8(pname);
                let (prefix, local) = pname.split_once(':').unwrap_or((pname, ""));
                N3Token::PrefixedName {
                    prefix,
                    local: unescape_string(local, span.start + prefix.len() + 1..span.end)?,
                    might_be_invalid_iri: true, // TODO
                }
            }
            N3LogosToken::PrefixedNameWithoutEscapes(pname) => {
                let pname = str_from_utf8(pname);
                let (prefix, local) = pname.split_once(':').unwrap_or((pname, ""));
                N3Token::PrefixedName {
                    prefix,
                    local: Cow::Borrowed(local),
                    might_be_invalid_iri: false,
                }
            }
            N3LogosToken::BlankNodeLabel(id) => N3Token::BlankNodeLabel(str_from_utf8(&id[2..])),
            N3LogosToken::String(str) => {
                let span = span.start + 1..span.end - 1;
                N3Token::String(unescape_string(
                    str_from_utf8(&str[1..str.len() - 1]),
                    span,
                )?)
            }
            N3LogosToken::LongString(str) => {
                let span = span.start + 3..span.end - 3;
                N3Token::LongString(unescape_string(
                    str_from_utf8(&str[3..str.len() - 3]),
                    span,
                )?)
            }
            N3LogosToken::LangTag(lang_tag) => {
                let span = span.start + 1..span.end;
                let lang_tag = str_from_utf8(&lang_tag[1..]);
                let (language, direction) = lang_tag.split_once("--").unwrap_or((lang_tag, ""));
                #[cfg(not(feature = "rdf-12"))]
                if !direction.is_empty() {
                    return Err((
                        span.end - direction.len()..span.end,
                        "Literal base direction are only allowed in RDF 1.2",
                    )
                        .into());
                }
                N3Token::LangTag {
                    language: if self.lenient {
                        language
                    } else {
                        LanguageTag::parse(language)
                            .map_err(|e| (span.clone(), e.to_string()))?
                            .into_inner()
                    },
                    #[cfg(feature = "rdf-12")]
                    direction: match direction {
                        "" => None,
                        "ltr" => Some(BaseDirection::Ltr),
                        "rtl" => Some(BaseDirection::Rtl),
                        _ => {
                            return Err((
                                span.end - direction.len()..span.end,
                                format!(
                                    "The allowed base directions are --ltr and --rtl, found --{direction}",
                                ),
                            )
                                .into());
                        }
                    },
                }
            }
            N3LogosToken::Integer(v) => N3Token::Integer(str_from_utf8(v)),
            N3LogosToken::Decimal(v) => N3Token::Decimal(str_from_utf8(v)),
            N3LogosToken::Double(v) => N3Token::Double(str_from_utf8(v)),
            N3LogosToken::Punctuation(p) => N3Token::Punctuation(str_from_utf8(p)),
            N3LogosToken::PlainKeyword(v) => N3Token::PlainKeyword(str_from_utf8(v)),
            N3LogosToken::Variable(v) => N3Token::Variable(Cow::Borrowed(str_from_utf8(&v[1..]))),
        })
    }

    fn parse_iri<'a>(
        &self,
        iri: Cow<'a, str>,
        span: Range<usize>,
        options: &N3LexerOptions,
    ) -> Result<N3Token<'a>, TokenRecognizerError> {
        Ok(N3Token::IriRef(
            if let Some(base_iri) = options.base_iri.as_ref() {
                Cow::Owned(
                    if self.lenient {
                        base_iri.resolve_unchecked(&iri)
                    } else {
                        base_iri.resolve(&iri).map_err(|e| (span, e.to_string()))?
                    }
                    .into_inner(),
                )
            } else if self.lenient {
                iri
            } else {
                Iri::parse(iri)
                    .map_err(|e| (span, e.to_string()))?
                    .into_inner()
            },
        ))
    }
}

pub fn resolve_local_name(
    prefix: &str,
    local: &str,
    might_be_invalid_iri: bool,
    prefixes: &HashMap<String, Iri<String>>,
) -> Result<NamedNode, String> {
    if let Some(start) = prefixes.get(prefix) {
        let iri = format!("{start}{local}");
        if might_be_invalid_iri || start.path().is_empty() {
            // We validate again. We always validate if the local part might be the IRI authority.
            if let Err(e) = Iri::parse(iri.as_str()) {
                return Err(format!(
                    "The prefixed name {prefix}:{local} builds IRI {iri} that is invalid: {e}"
                ));
            }
        }
        Ok(NamedNode::new_unchecked(iri))
    } else {
        Err(format!("The prefix {prefix}: has not been declared"))
    }
}

#[expect(unsafe_code)]
fn str_from_utf8(data: &[u8]) -> &str {
    // SAFETY: Logos validates UTF-8
    unsafe { str::from_utf8_unchecked(data) }
}

fn unescape_string(
    mut input: &str,
    span: Range<usize>,
) -> Result<Cow<'_, str>, TokenRecognizerError> {
    // TODO: return &str when no need for escaping
    let mut output = None;
    while let Some((before, after)) = input.split_once('\\') {
        let output = output.get_or_insert_with(|| String::with_capacity(input.len()));
        output.push_str(before);
        let mut after = after.chars();
        let (escape, after) = match after
            .next()
            .expect("strings are not allowed to end with a '\\'")
        {
            'u' => read_hex_char::<4>(after.as_str(), span.clone())?,
            'U' => read_hex_char::<8>(after.as_str(), span.clone())?,
            't' => ('\u{0009}', after.as_str()),
            'b' => ('\u{0008}', after.as_str()),
            'n' => ('\u{000A}', after.as_str()),
            'r' => ('\u{000D}', after.as_str()),
            'f' => ('\u{000C}', after.as_str()),
            c => (c, after.as_str()),
        };
        output.push(escape);
        input = after;
    }
    Ok(output.map_or(Cow::Borrowed(input), |mut output| {
        output.push_str(input);
        Cow::Owned(output)
    }))
}

fn read_hex_char<const SIZE: usize>(
    input: &str,
    span: Range<usize>,
) -> Result<(char, &str), TokenRecognizerError> {
    let escape = input
        .get(..SIZE)
        .expect("\\u escape sequence must contain 4 characters");
    let char = u32::from_str_radix(escape, 16)
        .expect("\\u escape sequence must be followed by hexadecimal digits");
    let char = char::from_u32(char).ok_or_else(|| {
        (
            span,
            format!("{char:#X} is not a valid unicode codepoint (surrogates are not supported"),
        )
    })?;
    Ok((char, &input[SIZE..]))
}
