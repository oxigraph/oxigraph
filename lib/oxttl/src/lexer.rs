use crate::toolkit::{TokenRecognizer, TokenRecognizerError};
use memchr::{memchr, memchr2};
use oxilangtag::LanguageTag;
use oxiri::Iri;
use oxrdf::NamedNode;
use std::borrow::Cow;
use std::cmp::min;
use std::collections::HashMap;
use std::ops::Range;
use std::str;

#[derive(Debug, PartialEq, Eq)]
pub enum N3Token<'a> {
    IriRef(String),
    PrefixedName {
        prefix: &'a str,
        local: Cow<'a, str>,
        might_be_invalid_iri: bool,
    },
    Variable(Cow<'a, str>),
    BlankNodeLabel(&'a str),
    String(String),
    Integer(&'a str),
    Decimal(&'a str),
    Double(&'a str),
    LangTag(&'a str),
    Punctuation(&'a str),
    PlainKeyword(&'a str),
}

#[derive(Eq, PartialEq)]
pub enum N3LexerMode {
    NTriples,
    Turtle,
    N3,
}

#[derive(Default)]
pub struct N3LexerOptions {
    pub base_iri: Option<Iri<String>>,
}

pub struct N3Lexer {
    mode: N3LexerMode,
    unchecked: bool,
}

// TODO: there are a lot of 'None' (missing data) returned even if the stream is ending!!!
// TODO: simplify by not giving is_end and fail with an "unexpected eof" is none is returned when is_end=true?

impl TokenRecognizer for N3Lexer {
    type Token<'a> = N3Token<'a>;
    type Options = N3LexerOptions;

    fn recognize_next_token<'a>(
        &mut self,
        data: &'a [u8],
        is_ending: bool,
        options: &N3LexerOptions,
    ) -> Option<(usize, Result<N3Token<'a>, TokenRecognizerError>)> {
        match *data.first()? {
            b'<' => match *data.get(1)? {
                b'<' => Some((2, Ok(N3Token::Punctuation("<<")))),
                b'=' if self.mode == N3LexerMode::N3 => {
                    if let Some((consumed, result)) = self.recognize_iri(data, options) {
                        Some(if let Ok(result) = result {
                            (consumed, Ok(result))
                        } else {
                            (2, Ok(N3Token::Punctuation("<=")))
                        })
                    } else if is_ending {
                        Some((2, Ok(N3Token::Punctuation("<="))))
                    } else {
                        None
                    }
                }
                b'-' if self.mode == N3LexerMode::N3 => {
                    if let Some((consumed, result)) = self.recognize_iri(data, options) {
                        Some(if let Ok(result) = result {
                            (consumed, Ok(result))
                        } else {
                            (2, Ok(N3Token::Punctuation("<-")))
                        })
                    } else if is_ending {
                        Some((2, Ok(N3Token::Punctuation("<-"))))
                    } else {
                        None
                    }
                }
                _ => self.recognize_iri(data, options),
            },
            b'>' => {
                if *data.get(1)? == b'>' {
                    Some((2, Ok(N3Token::Punctuation(">>"))))
                } else {
                    Some((1, Ok(N3Token::Punctuation(">"))))
                }
            }
            b'_' => match data.get(1)? {
                b':' => Self::recognize_blank_node_label(data),
                c => Some((
                    1,
                    Err((0, format!("Unexpected character '{}'", char::from(*c))).into()),
                )),
            },
            b'"' => {
                if self.mode != N3LexerMode::NTriples
                    && *data.get(1)? == b'"'
                    && *data.get(2)? == b'"'
                {
                    Self::recognize_long_string(data, b'"')
                } else {
                    Self::recognize_string(data, b'"')
                }
            }
            b'\'' if self.mode != N3LexerMode::NTriples => {
                if *data.get(1)? == b'\'' && *data.get(2)? == b'\'' {
                    Self::recognize_long_string(data, b'\'')
                } else {
                    Self::recognize_string(data, b'\'')
                }
            }
            b'@' => self.recognize_lang_tag(data),
            b'.' => match data.get(1) {
                Some(b'0'..=b'9') => Self::recognize_number(data),
                Some(_) => Some((1, Ok(N3Token::Punctuation(".")))),
                None => is_ending.then_some((1, Ok(N3Token::Punctuation(".")))),
            },
            b'^' => {
                if *data.get(1)? == b'^' {
                    Some((2, Ok(N3Token::Punctuation("^^"))))
                } else {
                    Some((1, Ok(N3Token::Punctuation("^"))))
                }
            }
            b'(' => Some((1, Ok(N3Token::Punctuation("(")))),
            b')' => Some((1, Ok(N3Token::Punctuation(")")))),
            b'[' => Some((1, Ok(N3Token::Punctuation("[")))),
            b']' => Some((1, Ok(N3Token::Punctuation("]")))),
            b'{' => {
                if *data.get(1)? == b'|' {
                    Some((2, Ok(N3Token::Punctuation("{|"))))
                } else {
                    Some((1, Ok(N3Token::Punctuation("{"))))
                }
            }
            b'}' => Some((1, Ok(N3Token::Punctuation("}")))),
            b',' => Some((1, Ok(N3Token::Punctuation(",")))),
            b';' => Some((1, Ok(N3Token::Punctuation(";")))),
            b'!' => Some((1, Ok(N3Token::Punctuation("!")))),
            b'|' => {
                if *data.get(1)? == b'}' {
                    Some((2, Ok(N3Token::Punctuation("|}"))))
                } else {
                    Some((1, Ok(N3Token::Punctuation("|"))))
                }
            }
            b'=' => {
                if *data.get(1)? == b'>' {
                    Some((2, Ok(N3Token::Punctuation("=>"))))
                } else {
                    Some((1, Ok(N3Token::Punctuation("="))))
                }
            }
            b'0'..=b'9' | b'+' | b'-' => Self::recognize_number(data),
            b'?' => self.recognize_variable(data, is_ending),
            _ => self.recognize_pname_or_keyword(data, is_ending),
        }
    }
}

impl N3Lexer {
    pub fn new(mode: N3LexerMode, unchecked: bool) -> Self {
        Self { mode, unchecked }
    }

    fn recognize_iri(
        &self,
        data: &[u8],
        options: &N3LexerOptions,
    ) -> Option<(usize, Result<N3Token<'static>, TokenRecognizerError>)> {
        // [18] IRIREF  ::=  '<' ([^#x00-#x20<>"{}|^`\] | UCHAR)* '>' /* #x00=NULL #01-#x1F=control codes #x20=space */
        let mut string = Vec::new();
        let mut i = 1;
        loop {
            let end = memchr2(b'>', b'\\', &data[i..])?;
            string.extend_from_slice(&data[i..i + end]);
            i += end;
            match data[i] {
                b'>' => {
                    #[allow(clippy::range_plus_one)]
                    return Some((i + 1, self.parse_iri(string, 0..i + 1, options)));
                }
                b'\\' => {
                    let (additional, c) = Self::recognize_escape(&data[i..], i, false)?;
                    i += additional + 1;
                    match c {
                        Ok(c) => {
                            let mut buf = [0; 4];
                            string.extend_from_slice(c.encode_utf8(&mut buf).as_bytes());
                        }
                        Err(e) => return Some((i, Err(e))),
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    fn parse_iri(
        &self,
        iri: Vec<u8>,
        position: Range<usize>,
        options: &N3LexerOptions,
    ) -> Result<N3Token<'static>, TokenRecognizerError> {
        let iri = string_from_utf8(iri, position.clone())?;
        Ok(N3Token::IriRef(
            if let Some(base_iri) = options.base_iri.as_ref() {
                if self.unchecked {
                    base_iri.resolve_unchecked(&iri)
                } else {
                    base_iri.resolve(&iri)
                }
                .map_err(|e| (position, e.to_string()))?
                .into_inner()
            } else if self.unchecked {
                iri
            } else {
                Iri::parse(iri)
                    .map_err(|e| (position, e.to_string()))?
                    .into_inner()
            },
        ))
    }

    fn recognize_pname_or_keyword<'a>(
        &self,
        data: &'a [u8],
        is_ending: bool,
    ) -> Option<(usize, Result<N3Token<'a>, TokenRecognizerError>)> {
        // [139s]  PNAME_NS   ::=  PN_PREFIX? ':'
        // [140s]  PNAME_LN   ::=  PNAME_NS PN_LOCAL
        // [167s]  PN_PREFIX  ::=  PN_CHARS_BASE ((PN_CHARS | '.')* PN_CHARS)?
        let mut i = 0;
        loop {
            if let Some(r) = Self::recognize_unicode_char(&data[i..], i) {
                match r {
                    Ok((c, consumed)) => {
                        if c == ':' {
                            i += consumed;
                            break;
                        } else if i == 0 {
                            if !Self::is_possible_pn_chars_base(c) {
                                return Some((
                                    consumed,
                                    Err((
                                        0..consumed,
                                        format!(
                                            "'{c}' is not allowed at the beginning of a prefix name"
                                        ),
                                    )
                                        .into()),
                                ));
                            }
                            i += consumed;
                        } else if Self::is_possible_pn_chars(c) || c == '.' {
                            i += consumed;
                        } else {
                            while data[..i].ends_with(b".") {
                                i -= 1;
                            }
                            return Some((
                                i,
                                str_from_utf8(&data[..i], 0..i).map(N3Token::PlainKeyword),
                            ));
                        }
                    }
                    Err(e) => return Some((e.location.end, Err(e))),
                }
            } else if is_ending {
                while data[..i].ends_with(b".") {
                    i -= 1;
                }
                return Some(if i == 0 {
                    (
                        1,
                        Err((0..1, format!("Unexpected byte {}", data[0])).into()),
                    )
                } else {
                    (
                        i,
                        str_from_utf8(&data[..i], 0..i).map(N3Token::PlainKeyword),
                    )
                });
            } else {
                return None;
            }
        }
        let pn_prefix = match str_from_utf8(&data[..i - 1], 0..i - 1) {
            Ok(pn_prefix) => pn_prefix,
            Err(e) => return Some((i, Err(e))),
        };
        if pn_prefix.ends_with('.') {
            return Some((
                i,
                Err((
                    0..i,
                    format!(
                        "'{pn_prefix}' is not a valid prefix: prefixes are not allowed to end with '.'"),
                )
                    .into()),
            ));
        }

        let (consumed, pn_local_result) =
            self.recognize_optional_pn_local(&data[i..], is_ending)?;
        Some((
            consumed + i,
            pn_local_result.map(|(local, might_be_invalid_iri)| N3Token::PrefixedName {
                prefix: pn_prefix,
                local,
                might_be_invalid_iri,
            }),
        ))
    }

    fn recognize_variable<'a>(
        &self,
        data: &'a [u8],
        is_ending: bool,
    ) -> Option<(usize, Result<N3Token<'a>, TokenRecognizerError>)> {
        // [36]  QUICK_VAR_NAME  ::=  "?" PN_LOCAL
        let (consumed, result) = self.recognize_optional_pn_local(&data[1..], is_ending)?;
        Some((
            consumed + 1,
            result.and_then(|(name, _)| {
                if name.is_empty() {
                    Err((0..consumed, "A variable name is not allowed to be empty").into())
                } else {
                    Ok(N3Token::Variable(name))
                }
            }),
        ))
    }

    fn recognize_optional_pn_local<'a>(
        &self,
        data: &'a [u8],
        is_ending: bool,
    ) -> Option<(usize, Result<(Cow<'a, str>, bool), TokenRecognizerError>)> {
        // [168s]  PN_LOCAL  ::=  (PN_CHARS_U | ':' | [0-9] | PLX) ((PN_CHARS | '.' | ':' | PLX)* (PN_CHARS | ':' | PLX))?
        let mut i = 0;
        let mut buffer = None; // Buffer if there are some escaped characters
        let mut position_that_is_already_in_buffer = 0;
        let mut might_be_invalid_iri = false;
        let mut ends_with_unescaped_dot = 0;
        loop {
            if let Some(r) = Self::recognize_unicode_char(&data[i..], i) {
                match r {
                    Ok((c, consumed)) => {
                        if c == '%' {
                            i += 1;
                            let a = char::from(*data.get(i)?);
                            i += 1;
                            let b = char::from(*data.get(i)?);
                            if !a.is_ascii_hexdigit() || !b.is_ascii_hexdigit() {
                                return Some((i + 1, Err((
                                    i - 2..=i, format!("escapes in IRIs should be % followed by two hexadecimal characters, found '%{a}{b}'")
                                ).into())));
                            }
                            i += 1;
                            ends_with_unescaped_dot = 0;
                        } else if c == '\\' {
                            i += 1;
                            let a = char::from(*data.get(i)?);
                            if self.unchecked
                                || matches!(
                                    a,
                                    '_' | '~'
                                        | '.'
                                        | '-'
                                        | '!'
                                        | '$'
                                        | '&'
                                        | '\''
                                        | '('
                                        | ')'
                                        | '*'
                                        | '+'
                                        | ','
                                        | ';'
                                        | '='
                                )
                            {
                                // ok to escape
                            } else if matches!(a, '/' | '?' | '#' | '@' | '%') {
                                // ok to escape but requires IRI validation
                                might_be_invalid_iri = true;
                            } else {
                                return Some((i + 1, Err((
                                    i..=i, format!("The character that are allowed to be escaped in IRIs are _~.-!$&'()*+,;=/?#@%, found '{a}'")
                                ).into())));
                            }
                            let buffer = buffer.get_or_insert_with(String::new);
                            // We add the missing bytes
                            if i - position_that_is_already_in_buffer > 1 {
                                buffer.push_str(
                                    match str_from_utf8(
                                        &data[position_that_is_already_in_buffer..i - 1],
                                        position_that_is_already_in_buffer..i - 1,
                                    ) {
                                        Ok(data) => data,
                                        Err(e) => return Some((i, Err(e))),
                                    },
                                )
                            }
                            buffer.push(a);
                            i += 1;
                            position_that_is_already_in_buffer = i;
                            ends_with_unescaped_dot = 0;
                        } else if i == 0 {
                            if !(Self::is_possible_pn_chars_u(c) || c == ':' || c.is_ascii_digit())
                            {
                                return Some((0, Ok((Cow::Borrowed(""), false))));
                            }
                            if !self.unchecked {
                                might_be_invalid_iri |=
                                    Self::is_possible_pn_chars_base_but_not_valid_iri(c)
                                        || c == ':';
                            }
                            i += consumed;
                        } else if Self::is_possible_pn_chars(c) || c == ':' {
                            if !self.unchecked {
                                might_be_invalid_iri |=
                                    Self::is_possible_pn_chars_base_but_not_valid_iri(c)
                                        || c == ':';
                            }
                            i += consumed;
                            ends_with_unescaped_dot = 0;
                        } else if c == '.' {
                            i += consumed;
                            ends_with_unescaped_dot += 1;
                        } else {
                            let buffer = if let Some(mut buffer) = buffer {
                                buffer.push_str(
                                    match str_from_utf8(
                                        &data[position_that_is_already_in_buffer..i],
                                        position_that_is_already_in_buffer..i,
                                    ) {
                                        Ok(data) => data,
                                        Err(e) => return Some((i, Err(e))),
                                    },
                                );
                                // We do not include the last dots
                                for _ in 0..ends_with_unescaped_dot {
                                    buffer.pop();
                                }
                                i -= ends_with_unescaped_dot;
                                Cow::Owned(buffer)
                            } else {
                                let mut data = match str_from_utf8(&data[..i], 0..i) {
                                    Ok(data) => data,
                                    Err(e) => return Some((i, Err(e))),
                                };
                                // We do not include the last dots
                                data = &data[..data.len() - ends_with_unescaped_dot];
                                i -= ends_with_unescaped_dot;
                                Cow::Borrowed(data)
                            };
                            return Some((i, Ok((buffer, might_be_invalid_iri))));
                        }
                    }
                    Err(e) => return Some((e.location.end, Err(e))),
                }
            } else if is_ending {
                let buffer = if let Some(mut buffer) = buffer {
                    // We do not include the last dot
                    while buffer.ends_with('.') {
                        buffer.pop();
                        i -= 1;
                    }
                    Cow::Owned(buffer)
                } else {
                    let mut data = match str_from_utf8(&data[..i], 0..i) {
                        Ok(data) => data,
                        Err(e) => return Some((i, Err(e))),
                    };
                    // We do not include the last dot
                    while let Some(d) = data.strip_suffix('.') {
                        data = d;
                        i -= 1;
                    }
                    Cow::Borrowed(data)
                };
                return Some((i, Ok((buffer, might_be_invalid_iri))));
            } else {
                return None;
            }
        }
    }

    fn recognize_blank_node_label(
        data: &[u8],
    ) -> Option<(usize, Result<N3Token<'_>, TokenRecognizerError>)> {
        // [141s]  BLANK_NODE_LABEL  ::=  '_:' (PN_CHARS_U | [0-9]) ((PN_CHARS | '.')* PN_CHARS)?
        let mut i = 2;
        loop {
            match Self::recognize_unicode_char(&data[i..], i)? {
                Ok((c, consumed)) => {
                    if (i == 2 && (Self::is_possible_pn_chars_u(c) || c.is_ascii_digit()))
                        || (i > 2 && Self::is_possible_pn_chars(c))
                    {
                        // Ok
                    } else if i > 2 && c == '.' {
                        if data[i - 1] == b'.' {
                            i -= 1;
                            return Some((
                                i,
                                str_from_utf8(&data[2..i], 2..i).map(N3Token::BlankNodeLabel),
                            ));
                        }
                    } else if i == 0 {
                        return Some((
                            i,
                            Err((0..i, "A blank node ID should not be empty").into()),
                        ));
                    } else if data[i - 1] == b'.' {
                        i -= 1;
                        return Some((
                            i,
                            str_from_utf8(&data[2..i], 2..i).map(N3Token::BlankNodeLabel),
                        ));
                    } else {
                        return Some((
                            i,
                            str_from_utf8(&data[2..i], 2..i).map(N3Token::BlankNodeLabel),
                        ));
                    }
                    i += consumed;
                }
                Err(e) => return Some((e.location.end, Err(e))),
            }
        }
    }

    fn recognize_lang_tag<'a>(
        &self,
        data: &'a [u8],
    ) -> Option<(usize, Result<N3Token<'a>, TokenRecognizerError>)> {
        // [144s]  LANGTAG  ::=  '@' [a-zA-Z]+ ('-' [a-zA-Z0-9]+)*
        let mut is_last_block_empty = true;
        for (i, c) in data[1..].iter().enumerate() {
            if c.is_ascii_alphabetic() {
                is_last_block_empty = false;
            } else if i == 0 {
                return Some((
                    1,
                    Err((1..2, "A language code should always start with a letter").into()),
                ));
            } else if is_last_block_empty {
                return Some((i, self.parse_lang_tag(&data[1..i], 1..i - 1)));
            } else if *c == b'-' {
                is_last_block_empty = true;
            } else {
                return Some((i + 1, self.parse_lang_tag(&data[1..=i], 1..i)));
            }
        }
        None
    }

    fn parse_lang_tag<'a>(
        &self,
        lang_tag: &'a [u8],
        position: Range<usize>,
    ) -> Result<N3Token<'a>, TokenRecognizerError> {
        let lang_tag = str_from_utf8(lang_tag, position.clone())?;
        Ok(N3Token::LangTag(if self.unchecked {
            lang_tag
        } else {
            LanguageTag::parse(lang_tag)
                .map_err(|e| (position.clone(), e.to_string()))?
                .into_inner()
        }))
    }

    fn recognize_string(
        data: &[u8],
        delimiter: u8,
    ) -> Option<(usize, Result<N3Token<'static>, TokenRecognizerError>)> {
        // [22]  STRING_LITERAL_QUOTE         ::=  '"' ([^#x22#x5C#xA#xD] | ECHAR | UCHAR)* '"' /* #x22=" #x5C=\ #xA=new line #xD=carriage return */
        // [23]  STRING_LITERAL_SINGLE_QUOTE  ::=  "'" ([^#x27#x5C#xA#xD] | ECHAR | UCHAR)* "'" /* #x27=' #x5C=\ #xA=new line #xD=carriage return */
        let mut string = String::new();
        let mut i = 1;
        loop {
            let end = memchr2(delimiter, b'\\', &data[i..])?;
            match str_from_utf8(&data[i..i + end], i..i + end) {
                Ok(s) => string.push_str(s),
                Err(e) => return Some((end, Err(e))),
            };
            i += end;
            match data[i] {
                c if c == delimiter => {
                    return Some((i + 1, Ok(N3Token::String(string))));
                }
                b'\\' => {
                    let (additional, c) = Self::recognize_escape(&data[i..], i, true)?;
                    i += additional + 1;
                    match c {
                        Ok(c) => {
                            string.push(c);
                        }
                        Err(e) => {
                            // We read until the end of string char
                            let end = memchr(delimiter, &data[i..])?;
                            return Some((i + end + 1, Err(e)));
                        }
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    fn recognize_long_string(
        data: &[u8],
        delimiter: u8,
    ) -> Option<(usize, Result<N3Token<'static>, TokenRecognizerError>)> {
        // [24]  STRING_LITERAL_LONG_SINGLE_QUOTE  ::=  "'''" (("'" | "''")? ([^'\] | ECHAR | UCHAR))* "'''"
        // [25]  STRING_LITERAL_LONG_QUOTE         ::=  '"""' (('"' | '""')? ([^"\] | ECHAR | UCHAR))* '"""'
        let mut string = String::new();
        let mut i = 3;
        loop {
            let end = memchr2(delimiter, b'\\', &data[i..])?;
            match str_from_utf8(&data[i..i + end], i..i + end) {
                Ok(s) => string.push_str(s),
                Err(e) => return Some((end, Err(e))),
            };
            i += end;
            match data[i] {
                c if c == delimiter => {
                    if *data.get(i + 1)? == delimiter && *data.get(i + 2)? == delimiter {
                        return Some((i + 3, Ok(N3Token::String(string))));
                    }
                    i += 1;
                    string.push(char::from(delimiter));
                }
                b'\\' => {
                    let (additional, c) = Self::recognize_escape(&data[i..], i, true)?;
                    i += additional + 1;
                    match c {
                        Ok(c) => {
                            string.push(c);
                        }
                        Err(e) => return Some((i, Err(e))),
                    }
                }
                _ => unreachable!(),
            }
        }
    }

    fn recognize_number(data: &[u8]) -> Option<(usize, Result<N3Token<'_>, TokenRecognizerError>)> {
        // [19]  INTEGER    ::=  [+-]? [0-9]+
        // [20]  DECIMAL    ::=  [+-]? [0-9]* '.' [0-9]+
        // [21]  DOUBLE     ::=  [+-]? ([0-9]+ '.' [0-9]* EXPONENT | '.' [0-9]+ EXPONENT | [0-9]+ EXPONENT)
        // [154s] EXPONENT  ::=  [eE] [+-]? [0-9]+
        let mut i = 0;
        let c = *data.first()?;
        if matches!(c, b'+' | b'-') {
            i += 1;
        }
        // We read the digits before .
        let mut count_before: usize = 0;
        loop {
            let c = *data.get(i)?;
            if c.is_ascii_digit() {
                i += 1;
                count_before += 1;
            } else {
                break;
            }
        }

        // We read the digits after .
        #[allow(clippy::if_then_some_else_none)]
        let count_after = if *data.get(i)? == b'.' {
            i += 1;

            let mut count_after = 0;
            loop {
                let c = *data.get(i)?;
                if c.is_ascii_digit() {
                    i += 1;
                    count_after += 1;
                } else {
                    break;
                }
            }
            Some(count_after)
        } else {
            None
        };

        // End
        let c = *data.get(i)?;
        if matches!(c, b'e' | b'E') {
            i += 1;

            let c = *data.get(i)?;
            if matches!(c, b'+' | b'-') {
                i += 1;
            }

            let mut found = false;
            loop {
                let c = *data.get(i)?;
                if c.is_ascii_digit() {
                    i += 1;
                    found = true;
                } else {
                    break;
                }
            }
            Some((
                i,
                if !found {
                    Err((0..i, "A double exponent cannot be empty").into())
                } else if count_before == 0 && count_after.unwrap_or(0) == 0 {
                    Err((0..i, "A double should not be empty").into())
                } else {
                    str_from_utf8(&data[..i], 0..i).map(N3Token::Double)
                },
            ))
        } else if let Some(count_after) = count_after {
            if count_after == 0 {
                // We do not consume the '.' after all
                i -= 1;
                Some((
                    i,
                    if count_before == 0 {
                        Err((0..i, "An integer should not be empty").into())
                    } else {
                        str_from_utf8(&data[..i], 0..i).map(N3Token::Integer)
                    },
                ))
            } else {
                Some((i, str_from_utf8(&data[..i], 0..i).map(N3Token::Decimal)))
            }
        } else {
            Some((
                i,
                if count_before == 0 {
                    Err((0..i, "An integer should not be empty").into())
                } else {
                    str_from_utf8(&data[..i], 0..i).map(N3Token::Integer)
                },
            ))
        }
    }

    fn recognize_escape(
        data: &[u8],
        position: usize,
        with_echar: bool,
    ) -> Option<(usize, Result<char, TokenRecognizerError>)> {
        // [26]   UCHAR  ::=  '\u' HEX HEX HEX HEX | '\U' HEX HEX HEX HEX HEX HEX HEX HEX
        // [159s] ECHAR  ::=  '\' [tbnrf"'\]
        match *data.get(1)? {
            b'u' => match Self::recognize_hex_char(&data[2..], 4, 'u', position) {
                Ok(c) => Some((5, Ok(c?))),
                Err(e) => Some((5, Err(e))),
            },
            b'U' => match Self::recognize_hex_char(&data[2..], 8, 'u', position) {
                Ok(c) => Some((9, Ok(c?))),
                Err(e) => Some((9, Err(e))),
            },
            b't' if with_echar => Some((1, Ok('\t'))),
            b'b' if with_echar => Some((1, Ok('\x08'))),
            b'n' if with_echar => Some((1, Ok('\n'))),
            b'r' if with_echar => Some((1, Ok('\r'))),
            b'f' if with_echar => Some((1, Ok('\x0C'))),
            b'"' if with_echar => Some((1, Ok('"'))),
            b'\'' if with_echar => Some((1, Ok('\''))),
            b'\\' if with_echar => Some((1, Ok('\\'))),
            c => Some((
                1,
                Err((
                    position..position + 2,
                    format!("Unexpected escape character '\\{}'", char::from(c)),
                )
                    .into()),
            )), // TODO: read until end of string
        }
    }

    fn recognize_hex_char(
        data: &[u8],
        len: usize,
        escape_char: char,
        position: usize,
    ) -> Result<Option<char>, TokenRecognizerError> {
        if data.len() < len {
            return Ok(None);
        }
        let val = str_from_utf8(&data[..len], position..position + len + 2)?;
        let codepoint = u32::from_str_radix(val, 16).map_err(|e| {
            (
                position..position + len + 2,
                format!(
                    "The escape sequence '\\{escape_char}{val}' is not a valid hexadecimal string: {e}"
                ),
            )
        })?;
        let c = char::from_u32(codepoint).ok_or_else(|| {
            (
                position..position + len +2,
                format!(
                    "The escape sequence '\\{escape_char}{val}' is encoding {codepoint:X} that is not a valid unicode character",
                ),
            )
        })?;
        Ok(Some(c))
    }

    fn recognize_unicode_char(
        data: &[u8],
        position: usize,
    ) -> Option<Result<(char, usize), TokenRecognizerError>> {
        let mut code_point: u32;
        let bytes_needed: usize;
        let mut lower_boundary = 0x80;
        let mut upper_boundary = 0xBF;

        let byte = *data.first()?;
        match byte {
            0x00..=0x7F => return Some(Ok((char::from(byte), 1))),
            0xC2..=0xDF => {
                bytes_needed = 1;
                code_point = u32::from(byte) & 0x1F;
            }
            0xE0..=0xEF => {
                if byte == 0xE0 {
                    lower_boundary = 0xA0;
                }
                if byte == 0xED {
                    upper_boundary = 0x9F;
                }
                bytes_needed = 2;
                code_point = u32::from(byte) & 0xF;
            }
            0xF0..=0xF4 => {
                if byte == 0xF0 {
                    lower_boundary = 0x90;
                }
                if byte == 0xF4 {
                    upper_boundary = 0x8F;
                }
                bytes_needed = 3;
                code_point = u32::from(byte) & 0x7;
            }
            _ => {
                return Some(Err((
                    position..=position,
                    "Invalid UTF-8 character encoding",
                )
                    .into()))
            }
        }

        for i in 1..=bytes_needed {
            let byte = *data.get(i)?;
            if byte < lower_boundary || upper_boundary < byte {
                return Some(Err((
                    position..=position + i,
                    "Invalid UTF-8 character encoding",
                )
                    .into()));
            }
            lower_boundary = 0x80;
            upper_boundary = 0xBF;
            code_point = (code_point << 6) | (u32::from(byte) & 0x3F);
        }

        Some(
            char::from_u32(code_point)
                .map(|c| (c, bytes_needed + 1))
                .ok_or_else(|| {
                    (
                        position..=position + bytes_needed,
                        format!("The codepoint {code_point:X} is not a valid unicode character"),
                    )
                        .into()
                }),
        )
    }

    // [157s]  PN_CHARS_BASE  ::=  [A-Z] | [a-z] | [#x00C0-#x00D6] | [#x00D8-#x00F6] | [#x00F8-#x02FF] | [#x0370-#x037D] | [#x037F-#x1FFF] | [#x200C-#x200D] | [#x2070-#x218F] | [#x2C00-#x2FEF] | [#x3001-#xD7FF] | [#xF900-#xFDCF] | [#xFDF0-#xFFFD] | [#x10000-#xEFFFF]
    fn is_possible_pn_chars_base(c: char) -> bool {
        matches!(c,
        'A'..='Z'
        | 'a'..='z'
        | '\u{00C0}'..='\u{00D6}'
        | '\u{00D8}'..='\u{00F6}'
        | '\u{00F8}'..='\u{02FF}'
        | '\u{0370}'..='\u{037D}'
        | '\u{037F}'..='\u{1FFF}'
        | '\u{200C}'..='\u{200D}'
        | '\u{2070}'..='\u{218F}'
        | '\u{2C00}'..='\u{2FEF}'
        | '\u{3001}'..='\u{D7FF}'
        | '\u{F900}'..='\u{FDCF}'
        | '\u{FDF0}'..='\u{FFFD}'
        | '\u{10000}'..='\u{EFFFF}')
    }

    // [158s]  PN_CHARS_U  ::=  PN_CHARS_BASE | '_' | ':'
    pub(super) fn is_possible_pn_chars_u(c: char) -> bool {
        Self::is_possible_pn_chars_base(c) || c == '_'
    }

    // [160s]  PN_CHARS  ::=  PN_CHARS_U | '-' | [0-9] | #x00B7 | [#x0300-#x036F] | [#x203F-#x2040]
    pub(crate) fn is_possible_pn_chars(c: char) -> bool {
        Self::is_possible_pn_chars_u(c)
            || matches!(c,
        '-' | '0'..='9' | '\u{00B7}' | '\u{0300}'..='\u{036F}' | '\u{203F}'..='\u{2040}')
    }

    fn is_possible_pn_chars_base_but_not_valid_iri(c: char) -> bool {
        matches!(c, '\u{FFF0}'..='\u{FFFD}')
            || u32::from(c) % u32::from('\u{FFFE}') == 0
            || u32::from(c) % u32::from('\u{FFFF}') == 0
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

fn str_from_utf8(data: &[u8], range: Range<usize>) -> Result<&str, TokenRecognizerError> {
    str::from_utf8(data).map_err(|e| {
        (
            range.start + e.valid_up_to()..min(range.end, range.start + e.valid_up_to() + 4),
            format!("Invalid UTF-8: {e}"),
        )
            .into()
    })
}

fn string_from_utf8(data: Vec<u8>, range: Range<usize>) -> Result<String, TokenRecognizerError> {
    String::from_utf8(data).map_err(|e| {
        (
            range.start + e.utf8_error().valid_up_to()
                ..min(range.end, range.start + e.utf8_error().valid_up_to() + 4),
            format!("Invalid UTF-8: {e}"),
        )
            .into()
    })
}
