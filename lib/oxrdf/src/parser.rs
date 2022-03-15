use crate::vocab::xsd;
use crate::{
    BlankNode, BlankNodeIdParseError, IriParseError, LanguageTagParseError, Literal, NamedNode,
    Subject, Term, Triple, Variable, VariableNameParseError,
};
use std::char;
use std::error::Error;
use std::fmt;
use std::str::{Chars, FromStr};

/// This limit is set in order to avoid stack overflow error when parsing nested triples due to too many recursive calls.
/// The actual limit value is a wet finger compromise between not failing to parse valid files and avoiding to trigger stack overflow errors.
const MAX_NUMBER_OF_NESTED_TRIPLES: usize = 128;

impl FromStr for NamedNode {
    type Err = TermParseError;

    /// Parses a named node from its NTriples and Turtle serialization
    ///
    /// ```
    /// use oxrdf::NamedNode;
    /// use std::str::FromStr;
    ///
    /// assert_eq!(NamedNode::from_str("<http://example.com>").unwrap(), NamedNode::new("http://example.com").unwrap())
    /// ```
    fn from_str(s: &str) -> Result<Self, TermParseError> {
        let (term, left) = read_named_node(s)?;
        if !left.is_empty() {
            return Err(TermParseError::msg(
                "Named node serialization should end with a >",
            ));
        }
        Ok(term)
    }
}

impl FromStr for BlankNode {
    type Err = TermParseError;

    /// Parses a blank node from its NTriples and Turtle serialization
    ///
    /// ```
    /// use oxrdf::BlankNode;
    /// use std::str::FromStr;
    ///
    /// assert_eq!(BlankNode::from_str("_:ex").unwrap(), BlankNode::new("ex").unwrap())
    /// ```
    fn from_str(s: &str) -> Result<Self, TermParseError> {
        let (term, left) = read_blank_node(s)?;
        if !left.is_empty() {
            return Err(TermParseError::msg(
                "Blank node serialization should not contain whitespaces",
            ));
        }
        Ok(term)
    }
}

impl FromStr for Literal {
    type Err = TermParseError;

    /// Parses a literal from its NTriples or Turtle serialization
    ///
    /// ```
    /// use oxrdf::{Literal, NamedNode, vocab::xsd};
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
        let (term, left) = read_literal(s)?;
        if !left.is_empty() {
            return Err(TermParseError::msg("Invalid literal serialization"));
        }
        Ok(term)
    }
}

impl FromStr for Term {
    type Err = TermParseError;

    /// Parses a term from its NTriples or Turtle serialization
    ///
    /// ```
    /// use oxrdf::*;
    /// use std::str::FromStr;
    ///
    /// assert_eq!(Term::from_str("\"ex\"").unwrap(), Literal::new_simple_literal("ex").into());
    /// assert_eq!(Term::from_str("<< _:s <http://example.com/p> \"o\" >>").unwrap(), Triple::new(
    ///     BlankNode::new("s").unwrap(),
    ///     NamedNode::new("http://example.com/p").unwrap(),
    ///     Literal::new_simple_literal("o")
    /// ).into());
    /// ```
    fn from_str(s: &str) -> Result<Self, TermParseError> {
        let (term, left) = read_term(s, 0)?;
        if !left.is_empty() {
            return Err(TermParseError::msg("Invalid term serialization"));
        }
        Ok(term)
    }
}

impl FromStr for Variable {
    type Err = TermParseError;

    /// Parses a variable from its SPARQL serialization
    ///
    /// ```
    /// use oxrdf::Variable;
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
        Self::new(&s[1..]).map_err(|error| TermParseError {
            kind: TermParseErrorKind::Variable {
                value: s.to_owned(),
                error,
            },
        })
    }
}

fn read_named_node(s: &str) -> Result<(NamedNode, &str), TermParseError> {
    let s = s.trim();
    if let Some(remain) = s.strip_prefix('<') {
        let end = remain
            .find('>')
            .ok_or_else(|| TermParseError::msg("Named node serialization should end with a >"))?;
        let (value, remain) = remain.split_at(end);
        let remain = &remain[1..];
        let term = NamedNode::new(value).map_err(|error| TermParseError {
            kind: TermParseErrorKind::Iri {
                value: value.to_owned(),
                error,
            },
        })?;
        Ok((term, remain))
    } else {
        Err(TermParseError::msg(
            "Named node serialization should start with a <",
        ))
    }
}

fn read_blank_node(s: &str) -> Result<(BlankNode, &str), TermParseError> {
    let s = s.trim();
    if let Some(remain) = s.strip_prefix("_:") {
        let end = remain
            .find(|v: char| v.is_whitespace() || matches!(v, '<' | '_' | '?' | '$' | '"' | '\''))
            .unwrap_or(remain.len());
        let (value, remain) = remain.split_at(end);
        let term = BlankNode::new(value).map_err(|error| TermParseError {
            kind: TermParseErrorKind::BlankNode {
                value: value.to_owned(),
                error,
            },
        })?;
        Ok((term, remain))
    } else {
        Err(TermParseError::msg(
            "Blank node serialization should start with '_:'",
        ))
    }
}

fn read_literal(s: &str) -> Result<(Literal, &str), TermParseError> {
    let s = s.trim();
    if let Some(s) = s.strip_prefix('"') {
        let mut value = String::with_capacity(s.len());
        let mut chars = s.chars();
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    let remain = chars.as_str();
                    return if let Some(remain) = remain.strip_prefix('@') {
                        let end = remain
                            .find(|v| !matches!(v, 'a'..='z' | 'A'..='Z' | '-'))
                            .unwrap_or(remain.len());
                        let (language, remain) = remain.split_at(end);
                        Ok((
                            Literal::new_language_tagged_literal(value, language).map_err(
                                |error| TermParseError {
                                    kind: TermParseErrorKind::LanguageTag {
                                        value: language.to_owned(),
                                        error,
                                    },
                                },
                            )?,
                            remain,
                        ))
                    } else if let Some(remain) = remain.strip_prefix("^^") {
                        let (datatype, remain) = read_named_node(remain)?;
                        Ok((Literal::new_typed_literal(value, datatype), remain))
                    } else {
                        Ok((Literal::new_simple_literal(value), remain))
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
    } else if let Some(remain) = s.strip_prefix("true") {
        Ok((Literal::new_typed_literal("true", xsd::BOOLEAN), remain))
    } else if let Some(remain) = s.strip_prefix("false") {
        Ok((Literal::new_typed_literal("false", xsd::BOOLEAN), remain))
    } else {
        let input = s.as_bytes();
        if input.is_empty() {
            return Err(TermParseError::msg("Empty term serialization"));
        }

        let mut cursor = match input.get(0) {
            Some(b'+' | b'-') => 1,
            _ => 0,
        };
        let mut with_dot = false;

        let mut count_before: usize = 0;
        while cursor < input.len() && b'0' <= input[cursor] && input[cursor] <= b'9' {
            count_before += 1;
            cursor += 1;
        }

        let mut count_after: usize = 0;
        if cursor < input.len() && input[cursor] == b'.' {
            with_dot = true;
            cursor += 1;
            while cursor < input.len() && b'0' <= input[cursor] && input[cursor] <= b'9' {
                count_after += 1;
                cursor += 1;
            }
        }

        if cursor < input.len() && (input[cursor] == b'e' || input[cursor] == b'E') {
            cursor += 1;
            cursor += match input.get(cursor) {
                Some(b'+' | b'-') => 1,
                _ => 0,
            };
            let mut count_exponent = 0;
            while cursor < input.len() && b'0' <= input[cursor] && input[cursor] <= b'9' {
                count_exponent += 1;
                cursor += 1;
            }
            if count_exponent > 0 {
                Ok((Literal::new_typed_literal(s, xsd::DOUBLE), &s[cursor..]))
            } else {
                Err(TermParseError::msg(
                    "Double serialization with an invalid exponent",
                ))
            }
        } else if with_dot {
            if count_after > 0 {
                Ok((Literal::new_typed_literal(s, xsd::DECIMAL), &s[cursor..]))
            } else {
                Err(TermParseError::msg(
                    "Decimal serialization without floating part",
                ))
            }
        } else if count_before > 0 {
            Ok((Literal::new_typed_literal(s, xsd::INTEGER), &s[cursor..]))
        } else {
            Err(TermParseError::msg("Empty integer serialization"))
        }
    }
}

fn read_term(s: &str, number_of_recursive_calls: usize) -> Result<(Term, &str), TermParseError> {
    let s = s.trim();
    if let Some(remain) = s.strip_prefix("<<") {
        if number_of_recursive_calls == MAX_NUMBER_OF_NESTED_TRIPLES {
            return Err(TermParseError::msg(
                "Too many nested triples. The parser fails here to avoid a stack overflow.",
            ));
        }
        let (subject, remain) = read_term(remain, number_of_recursive_calls + 1)?;
        let (predicate, remain) = read_named_node(remain)?;
        let (object, remain) = read_term(remain, number_of_recursive_calls + 1)?;
        let remain = remain.trim_start();
        if let Some(remain) = remain.strip_prefix(">>") {
            #[cfg(feature = "rdf-star")]
            {
                Ok((
                    Triple {
                        subject: match subject {
                            Term::NamedNode(s) => s.into(),
                            Term::BlankNode(s) => s.into(),
                            Term::Literal(_) => {
                                return Err(TermParseError::msg(
                                    "Literals are not allowed in subject position",
                                ))
                            }
                            Term::Triple(s) => Subject::Triple(s),
                        },
                        predicate,
                        object,
                    }
                    .into(),
                    remain,
                ))
            }
            #[cfg(not(feature = "rdf-star"))]
            {
                Err(TermParseError::msg("RDF-star is not supported"))
            }
        } else {
            Err(TermParseError::msg(
                "Nested triple serialization should be enclosed between << and >>",
            ))
        }
    } else if s.starts_with('<') {
        let (term, remain) = read_named_node(s)?;
        Ok((term.into(), remain))
    } else if s.starts_with('_') {
        let (term, remain) = read_blank_node(s)?;
        Ok((term.into(), remain))
    } else {
        let (term, remain) = read_literal(s)?;
        Ok((term.into(), remain))
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

/// An error raised during term serialization parsing using the [`FromStr`] trait.
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
            TermParseErrorKind::Msg { msg } => f.write_str(msg),
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
