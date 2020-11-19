use crate::model::blank_node::{BlankNode, BlankNodeIdParseError};
use crate::model::named_node::NamedNode;
use crate::model::vocab::xsd;
use crate::model::{Literal, Term};
use crate::sparql::{Variable, VariableNameParseError};
use oxilangtag::LanguageTagParseError;
use oxiri::IriParseError;
use std::char;
use std::error::Error;
use std::fmt;
use std::str::{Chars, FromStr};

impl FromStr for NamedNode {
    type Err = TermParseError;

    /// Parses a named node from its NTriples and Turtle serialization
    ///
    /// ```
    /// use oxigraph::model::NamedNode;
    /// use std::str::FromStr;
    ///
    /// assert_eq!(NamedNode::from_str("<http://example.com>").unwrap(), NamedNode::new("http://example.com").unwrap())
    /// ```
    fn from_str(s: &str) -> Result<Self, TermParseError> {
        if !s.starts_with('<') || !s.ends_with('>') {
            return Err(TermParseError::msg(
                "Named node serialization should be enclosed between < and >",
            ));
        }
        NamedNode::new(&s[1..s.len() - 1]).map_err(|error| TermParseError {
            kind: TermParseErrorKind::Iri {
                value: s.to_owned(),
                error,
            },
        })
    }
}

impl FromStr for BlankNode {
    type Err = TermParseError;

    /// Parses a blank node from its NTriples and Turtle serialization
    ///
    /// ```
    /// use oxigraph::model::BlankNode;
    /// use std::str::FromStr;
    ///
    /// assert_eq!(BlankNode::from_str("_:ex").unwrap(), BlankNode::new("ex").unwrap())
    /// ```
    fn from_str(s: &str) -> Result<Self, TermParseError> {
        if !s.starts_with("_:") {
            return Err(TermParseError::msg(
                "Blank node serialization should start with '_:'",
            ));
        }
        BlankNode::new(&s[2..]).map_err(|error| TermParseError {
            kind: TermParseErrorKind::BlankNode {
                value: s.to_owned(),
                error,
            },
        })
    }
}

impl FromStr for Literal {
    type Err = TermParseError;

    /// Parses a literal from its NTriples or Turtle serialization
    ///
    /// ```
    /// use oxigraph::model::{Literal, NamedNode, vocab::xsd};
    /// use std::str::FromStr;
    ///
    /// assert_eq!(Literal::from_str("\"ex\\n\"").unwrap(), Literal::new_simple_literal("ex\n"));
    /// assert_eq!(Literal::from_str("\"ex\"@en").unwrap(), Literal::new_language_tagged_literal("ex", "en").unwrap());
    /// assert_eq!(Literal::from_str("\"2020\"^^<http://www.w3.org/2001/XMLSchema#gYear>").unwrap(), Literal::new_typed_literal("2020", NamedNode::new("http://www.w3.org/2001/XMLSchema#gYear").unwrap()));
    /// assert_eq!(Literal::from_str("true").unwrap(), Literal::new_typed_literal("true", xsd::BOOLEAN));
    /// assert_eq!(Literal::from_str("+122").unwrap(), Literal::new_typed_literal("+122", xsd::INTEGER));
    /// assert_eq!(Literal::from_str("-122.23").unwrap(), Literal::new_typed_literal("-122.23", xsd::DECIMAL));
    /// assert_eq!(Literal::from_str("-122e+1").unwrap(), Literal::new_typed_literal("-122e+1", xsd::DOUBLE));
    /// ```
    fn from_str(s: &str) -> Result<Self, TermParseError> {
        if let Some(s) = s.strip_prefix('"') {
            let mut value = String::with_capacity(s.len() - 1);
            let mut chars = s.chars();
            while let Some(c) = chars.next() {
                match c {
                    '"' => {
                        let remain = chars.as_str();
                        return if remain.is_empty() {
                            Ok(Literal::new_simple_literal(value))
                        } else if let Some(language) = remain.strip_prefix('@') {
                            Literal::new_language_tagged_literal(value, &remain[1..]).map_err(
                                |error| TermParseError {
                                    kind: TermParseErrorKind::LanguageTag {
                                        value: language.to_owned(),
                                        error,
                                    },
                                },
                            )
                        } else if let Some(datatype) = remain.strip_prefix("^^") {
                            Ok(Literal::new_typed_literal(
                                value,
                                NamedNode::from_str(datatype)?,
                            ))
                        } else {
                            Err(TermParseError::msg("Unexpected characters after a literal"))
                        };
                    }
                    '\\' => {
                        if let Some(c) = chars.next() {
                            value.push(match c {
                                't' => '\t',
                                'b' => '\u{8}',
                                'n' => '\n',
                                'r' => '\r',
                                'f' => '\u{C}',
                                '"' => '"',
                                '\'' => '\'',
                                '\\' => '\\',
                                'u' => read_hexa_char(&mut chars, 4)?,
                                'U' => read_hexa_char(&mut chars, 8)?,
                                _ => return Err(TermParseError::msg("Unexpected escaped char")),
                            })
                        } else {
                            return Err(TermParseError::msg("Unexpected literal end"));
                        }
                    }
                    c => value.push(c),
                }
            }
            Err(TermParseError::msg("Unexpected literal end"))
        } else if s == "true" {
            Ok(Literal::new_typed_literal("true", xsd::BOOLEAN))
        } else if s == "false" {
            Ok(Literal::new_typed_literal("false", xsd::BOOLEAN))
        } else {
            let input = s.as_bytes();
            if input.is_empty() {
                return Err(TermParseError::msg("Empty term serialization"));
            }

            let mut cursor = match input.get(0) {
                Some(b'+') | Some(b'-') => 1,
                _ => 0,
            };

            let mut count_before: usize = 0;
            while cursor < input.len() && b'0' <= input[cursor] && input[cursor] <= b'9' {
                count_before += 1;
                cursor += 1;
            }

            if cursor == input.len() {
                return if count_before > 0 {
                    Ok(Literal::new_typed_literal(s, xsd::INTEGER))
                } else {
                    Err(TermParseError::msg("Empty integer serialization"))
                };
            }

            let mut count_after: usize = 0;
            if input[cursor] == b'.' {
                cursor += 1;
                while cursor < input.len() && b'0' <= input[cursor] && input[cursor] <= b'9' {
                    count_after += 1;
                    cursor += 1;
                }
            }

            if cursor == input.len() {
                return if count_after > 0 {
                    Ok(Literal::new_typed_literal(s, xsd::DECIMAL))
                } else {
                    Err(TermParseError::msg(
                        "Decimal serialization without floating part",
                    ))
                };
            }

            if input[cursor] != b'e' && input[cursor] != b'E' {
                return Err(TermParseError::msg("Double serialization without exponent"));
            }
            cursor += 1;
            cursor += match input.get(cursor) {
                Some(b'+') | Some(b'-') => 1,
                _ => 0,
            };
            let mut count_exponent = 0;
            while cursor < input.len() && b'0' <= input[cursor] && input[cursor] <= b'9' {
                count_exponent += 1;
                cursor += 1;
            }
            if cursor == input.len() && count_exponent > 0 {
                Ok(Literal::new_typed_literal(s, xsd::DOUBLE))
            } else {
                Err(TermParseError::msg(
                    "Double serialization with an invalid exponent",
                ))
            }
        }
    }
}

fn read_hexa_char(input: &mut Chars<'_>, len: usize) -> Result<char, TermParseError> {
    let mut value = 0;
    for _ in 0..len {
        if let Some(c) = input.next() {
            value = value * 16
                + match c {
                    '0'..='9' => u32::from(c) - u32::from('0'),
                    'a'..='f' => u32::from(c) - u32::from('a') + 10,
                    'A'..='F' => u32::from(c) - u32::from('A') + 10,
                    _ => {
                        return Err(TermParseError::msg(
                            "Unexpected character in a unicode escape",
                        ))
                    }
                }
        } else {
            return Err(TermParseError::msg("Unexpected literal string end"));
        }
    }
    char::from_u32(value).ok_or_else(|| TermParseError::msg("Invalid encoded unicode code point"))
}

impl FromStr for Term {
    type Err = TermParseError;

    /// Parses a term from its NTriples or Turtle serialization
    ///
    /// ```
    /// use oxigraph::model::{Literal, Term};
    /// use std::str::FromStr;
    ///
    /// assert_eq!(Term::from_str("\"ex\"").unwrap(), Literal::new_simple_literal("ex").into())
    /// ```
    fn from_str(s: &str) -> Result<Self, TermParseError> {
        Ok(if s.starts_with('<') {
            NamedNode::from_str(s)?.into()
        } else if s.starts_with('_') {
            BlankNode::from_str(s)?.into()
        } else {
            Literal::from_str(s)?.into()
        })
    }
}

impl FromStr for Variable {
    type Err = TermParseError;

    /// Parses a variable from its SPARQL serialization
    ///
    /// ```
    /// use oxigraph::sparql::Variable;
    /// use std::str::FromStr;
    ///
    /// assert_eq!(Variable::from_str("$foo").unwrap(), Variable::new("foo").unwrap())
    /// ```
    fn from_str(s: &str) -> Result<Self, TermParseError> {
        if !s.starts_with('?') && !s.starts_with('$') {
            return Err(TermParseError::msg(
                "Variable serialization should start with ? or $",
            ));
        }
        Variable::new(&s[1..]).map_err(|error| TermParseError {
            kind: TermParseErrorKind::Variable {
                value: s.to_owned(),
                error,
            },
        })
    }
}

/// An error raised during term serialization parsing.
#[allow(missing_copy_implementations)]
#[derive(Debug)]
pub struct TermParseError {
    kind: TermParseErrorKind,
}

#[derive(Debug)]
enum TermParseErrorKind {
    Iri {
        error: IriParseError,
        value: String,
    },
    BlankNode {
        error: BlankNodeIdParseError,
        value: String,
    },
    LanguageTag {
        error: LanguageTagParseError,
        value: String,
    },
    Variable {
        error: VariableNameParseError,
        value: String,
    },
    Msg {
        msg: &'static str,
    },
}

impl fmt::Display for TermParseError {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.kind {
            TermParseErrorKind::Iri { error, value } => write!(
                f,
                "Error while parsing the named node '{}': {}",
                value, error
            ),
            TermParseErrorKind::BlankNode { error, value } => write!(
                f,
                "Error while parsing the blank node '{}': {}",
                value, error
            ),
            TermParseErrorKind::LanguageTag { error, value } => write!(
                f,
                "Error while parsing the language tag '{}': {}",
                value, error
            ),
            TermParseErrorKind::Variable { error, value } => {
                write!(f, "Error while parsing the variable '{}': {}", value, error)
            }
            TermParseErrorKind::Msg { msg } => write!(f, "{}", msg),
        }
    }
}

impl Error for TermParseError {}

impl TermParseError {
    pub(crate) fn msg(msg: &'static str) -> Self {
        Self {
            kind: TermParseErrorKind::Msg { msg },
        }
    }
}
