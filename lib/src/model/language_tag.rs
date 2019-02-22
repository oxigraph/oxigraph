//! This crate implements the [BCP 47](https://tools.ietf.org/html/bcp47#section-2.1)
//! Some code and comments of this file is taken from https://github.com/pyfisch/rust-language-tags under MIT license

use std::error::Error;
use std::fmt;
use std::iter::once;
use std::str::FromStr;
use std::str::Split;

/// A [RFC 5646](https://tools.ietf.org/html/rfc5646) language tag
#[derive(Eq, PartialEq, Ord, PartialOrd, Debug, Clone, Hash)]
pub struct LanguageTag {
    /// Syntax described in [RFC 5646 2.1](https://tools.ietf.org/html/rfc5646#section-2.1)
    serialization: String,
    language_end: usize,
    extlang_end: usize,
    script_end: usize,
    region_end: usize,
    variant_end: usize,
    extension_end: usize,
}

impl LanguageTag {
    /// Return the serialization of this language tag.
    ///
    /// This is fast since that serialization is already stored in the `LanguageTag` struct.
    #[inline]
    pub fn as_str(&self) -> &str {
        &self.serialization
    }

    /// Return the serialization of this language tag.
    ///
    /// This consumes the `LanguageTag` and takes ownership of the `String` stored in it.
    #[inline]
    pub fn into_string(self) -> String {
        self.serialization
    }

    /// Return the [primary language subtag](https://tools.ietf.org/html/rfc5646#section-2.2.1).
    #[inline]
    pub fn primary_language(&self) -> &str {
        &self.serialization[..self.language_end]
    }

    /// Return the [extended language subtags](https://tools.ietf.org/html/rfc5646#section-2.2.2).
    ///
    /// Valid language tags have at most one extended language.
    #[inline]
    pub fn extended_language(&self) -> Option<&str> {
        if self.language_end == self.extlang_end {
            None
        } else {
            Some(&self.serialization[self.language_end + 1..self.extlang_end])
        }
    }

    /// Iterate on the [extended language subtags](https://tools.ietf.org/html/rfc5646#section-2.2.2).
    ///
    /// Valid language tags have at most one extended language.
    #[inline]
    pub fn extended_language_subtags(&self) -> impl Iterator<Item = &str> {
        match self.extended_language() {
            Some(parts) => SubtagListIterator::new(parts),
            None => SubtagListIterator::new(""),
        }
    }

    /// Return the [primary language subtag](https://tools.ietf.org/html/rfc5646#section-2.2.1)
    /// and its [extended language subtags](https://tools.ietf.org/html/rfc5646#section-2.2.2).
    #[inline]
    pub fn full_language(&self) -> &str {
        &self.serialization[..self.extlang_end]
    }

    /// Return the [script subtag](https://tools.ietf.org/html/rfc5646#section-2.2.3).
    #[inline]
    pub fn script(&self) -> Option<&str> {
        if self.extlang_end == self.script_end {
            None
        } else {
            Some(&self.serialization[self.extlang_end + 1..self.script_end])
        }
    }

    /// Return the [region subtag](https://tools.ietf.org/html/rfc5646#section-2.2.4).
    #[inline]
    pub fn region(&self) -> Option<&str> {
        if self.script_end == self.region_end {
            None
        } else {
            Some(&self.serialization[self.script_end + 1..self.region_end])
        }
    }

    /// Return the [variant subtags](https://tools.ietf.org/html/rfc5646#section-2.2.5).
    #[inline]
    pub fn variant(&self) -> Option<&str> {
        if self.region_end == self.variant_end {
            None
        } else {
            Some(&self.serialization[self.region_end + 1..self.variant_end])
        }
    }

    /// Iterate on the [variant subtags](https://tools.ietf.org/html/rfc5646#section-2.2.5).
    #[inline]
    pub fn variant_subtags(&self) -> impl Iterator<Item = &str> {
        match self.variant() {
            Some(parts) => SubtagListIterator::new(parts),
            None => SubtagListIterator::new(""),
        }
    }

    /// Return the [extension subtags](https://tools.ietf.org/html/rfc5646#section-2.2.6).
    #[inline]
    pub fn extension(&self) -> Option<&str> {
        if self.variant_end == self.extension_end {
            None
        } else {
            Some(&self.serialization[self.variant_end + 1..self.extension_end])
        }
    }

    /// Iterate on the [extension subtags](https://tools.ietf.org/html/rfc5646#section-2.2.6).
    #[inline]
    pub fn extension_subtags(&self) -> impl Iterator<Item = (char, &str)> {
        match self.extension() {
            Some(parts) => ExtensionsIterator::new(parts),
            None => ExtensionsIterator::new(""),
        }
    }

    /// Return the [private use subtags](https://tools.ietf.org/html/rfc5646#section-2.2.7).
    #[inline]
    pub fn private_use(&self) -> Option<&str> {
        if self.serialization.starts_with("x-") {
            Some(&self.serialization)
        } else if self.extension_end == self.serialization.len() {
            None
        } else {
            Some(&self.serialization[self.extension_end + 1..])
        }
    }

    /// Iterate on the [private use subtags](https://tools.ietf.org/html/rfc5646#section-2.2.7).
    #[inline]
    pub fn private_use_subtags(&self) -> impl Iterator<Item = &str> {
        match self.private_use() {
            Some(parts) => SubtagListIterator::new(&parts[2..]),
            None => SubtagListIterator::new(""),
        }
    }

    /// Create a [`LanguageTag`] from its serialization.
    ///
    /// This parser accepts the language tags that are "well-formed" according to
    /// [RFC 5646](https://tools.ietf.org/html/rfc5646#section-2.2.9).
    /// Full validation could be done with the `validate` method.
    ///
    ///
    /// # Errors
    ///
    /// If the language tag is not "well-formed" a [`ParseError`] variant will be returned.
    pub fn parse(input: &str) -> Result<Self, ParseError> {
        //grandfathered tags
        if let Some(tag) = GRANDFATHERED.iter().find(|x| x.eq_ignore_ascii_case(input)) {
            return Ok(Self {
                serialization: tag.to_string(),
                language_end: tag.len(),
                extlang_end: tag.len(),
                script_end: tag.len(),
                region_end: tag.len(),
                variant_end: tag.len(),
                extension_end: tag.len(),
            });
        }

        //private use
        if input.starts_with("x-") {
            if !is_alphanumeric_or_dash(input) {
                return Err(ParseError::ForbiddenChar);
            }
            if input.len() == 2 {
                return Err(ParseError::EmptyPrivateUse);
            }
            return Ok(Self {
                serialization: input.to_ascii_lowercase(),
                language_end: input.len(),
                extlang_end: input.len(),
                script_end: input.len(),
                region_end: input.len(),
                variant_end: input.len(),
                extension_end: input.len(),
            });
        }

        #[derive(PartialEq, Eq)]
        enum State {
            Start,
            AfterLanguage,
            AfterExtLang,
            AfterScript,
            AfterRegion,
            InExtension { expected: bool },
            InPrivateUse { expected: bool },
        }
        let mut serialization = String::with_capacity(input.len());

        let mut state = State::Start;
        let mut language_end = 0;
        let mut extlang_end = 0;
        let mut script_end = 0;
        let mut region_end = 0;
        let mut variant_end = 0;
        let mut extension_end = 0;
        let mut extlangs_count = 0;
        for (subtag, end) in SubTagIterator::new(input) {
            if subtag.is_empty() {
                // All subtags have a maximum length of eight characters.
                return Err(ParseError::EmptySubtag);
            }
            if subtag.len() > 8 {
                // All subtags have a maximum length of eight characters.
                return Err(ParseError::SubtagTooLong);
            }
            if state == State::Start {
                // Primary language
                if subtag.len() < 2 || !is_alphabetic(subtag) {
                    return Err(ParseError::InvalidLanguage);
                }
                language_end = end;
                serialization.extend(to_lowercase(subtag));
                if subtag.len() < 4 {
                    // extlangs are only allowed for short language tags
                    state = State::AfterLanguage;
                } else {
                    state = State::AfterExtLang;
                }
            } else if let State::InPrivateUse { .. } = state {
                if !is_alphanumeric(subtag) {
                    return Err(ParseError::InvalidSubtag);
                }
                serialization.push('-');
                serialization.extend(to_lowercase(subtag));
                state = State::InPrivateUse { expected: false };
            } else if subtag == "x" || subtag == "X" {
                // We make sure extension is found
                if let State::InExtension { expected: true } = state {
                    return Err(ParseError::EmptyExtension);
                }
                serialization.push('-');
                serialization.push('x');
                state = State::InPrivateUse { expected: true };
            } else if subtag.len() == 1 && is_alphanumeric(subtag) {
                // We make sure extension is found
                if let State::InExtension { expected: true } = state {
                    return Err(ParseError::EmptyExtension);
                }
                let extension_tag = subtag.chars().next().unwrap().to_ascii_lowercase();
                serialization.push('-');
                serialization.push(extension_tag);
                state = State::InExtension { expected: true };
            } else if let State::InExtension { .. } = state {
                if !is_alphanumeric(subtag) {
                    return Err(ParseError::InvalidSubtag);
                }
                extension_end = end;
                serialization.push('-');
                serialization.extend(to_lowercase(subtag));
                state = State::InExtension { expected: false };
            } else if state == State::AfterLanguage && subtag.len() == 3 && is_alphabetic(subtag) {
                extlangs_count += 1;
                if extlangs_count > 3 {
                    return Err(ParseError::TooManyExtlangs);
                }
                // valid extlangs
                extlang_end = end;
                serialization.push('-');
                serialization.extend(to_lowercase(subtag));
            } else if (state == State::AfterLanguage || state == State::AfterExtLang)
                && subtag.len() == 4
                && is_alphabetic(subtag)
            {
                // Script
                script_end = end;
                serialization.push('-');
                serialization.extend(to_uppercase_first(subtag));
                state = State::AfterScript;
            } else if (state == State::AfterLanguage
                || state == State::AfterExtLang
                || state == State::AfterScript)
                && (subtag.len() == 2 && is_alphabetic(subtag)
                    || subtag.len() == 3 && is_numeric(subtag))
            {
                // Region
                region_end = end;
                serialization.push('-');
                serialization.extend(to_uppercase(subtag));
                state = State::AfterRegion;
            } else if (state == State::AfterLanguage
                || state == State::AfterExtLang
                || state == State::AfterScript
                || state == State::AfterRegion)
                && is_alphanumeric(subtag)
                && (subtag.len() >= 5 && is_alphabetic(&subtag[0..1])
                    || subtag.len() >= 4 && is_numeric(&subtag[0..1]))
            {
                // Variant
                variant_end = end;
                serialization.push('-');
                serialization.extend(to_lowercase(subtag));
                state = State::AfterRegion;
            } else {
                return Err(ParseError::InvalidSubtag);
            }
        }

        //We make sure we are in a correct final state
        if let State::InExtension { expected: true } = state {
            return Err(ParseError::EmptyExtension);
        }
        if let State::InPrivateUse { expected: true } = state {
            return Err(ParseError::EmptyPrivateUse);
        }

        //We make sure we have not skipped anyone
        if extlang_end < language_end {
            extlang_end = language_end;
        }
        if script_end < extlang_end {
            script_end = extlang_end;
        }
        if region_end < script_end {
            region_end = script_end;
        }
        if variant_end < region_end {
            variant_end = region_end;
        }
        if extension_end < variant_end {
            extension_end = variant_end;
        }

        Ok(Self {
            serialization,
            language_end,
            extlang_end,
            script_end,
            region_end,
            variant_end,
            extension_end,
        })
    }

    /// Check if the language tag is "valid" according to
    /// [RFC 5646](https://tools.ietf.org/html/rfc5646#section-2.2.9).
    ///
    ///
    /// # Errors
    ///
    /// If the language tag is not "valid" a [`ValidationError`] variant will be returned.
    pub fn validate(&self) -> Result<(), ValidationError> {
        // The tag is well-formed.
        // always ok

        // Either the tag is in the list of grandfathered tags or all of its
        // primary language, extended language, script, region, and variant
        // subtags appear in the IANA Language Subtag Registry as of the
        // particular registry date.
        // TODO

        // There are no duplicate variant subtags.
        if self.variant_subtags().enumerate().any(|(id1, variant1)| {
            self.variant_subtags()
                .enumerate()
                .any(|(id2, variant2)| id1 != id2 && variant1 == variant2)
        }) {
            return Err(ValidationError::DuplicateVariant);
        }

        // There are no duplicate singleton (extension) subtags.
        if let Some(extension) = self.extension() {
            let mut seen_extensions = AlphanumericLowerCharSet::new();
            if extension.split('-').any(|subtag| {
                if subtag.len() == 1 {
                    let extension = subtag.chars().next().unwrap();
                    if seen_extensions.contains(extension) {
                        true
                    } else {
                        seen_extensions.insert(extension);
                        false
                    }
                } else {
                    false
                }
            }) {
                return Err(ValidationError::DuplicateExtension);
            }
        }

        // There is no more than one extended language subtag.
        // From [errata 5457](https://www.rfc-editor.org/errata/eid5457).
        if let Some(extended_language) = self.extended_language() {
            if extended_language.contains('-') {
                return Err(ValidationError::MultipleExtendedLanguageSubtags);
            }
        }

        Ok(())
    }

    /// Check if the language tag is valid according to
    /// [RFC 5646](https://tools.ietf.org/html/rfc5646#section-2.2.9).
    pub fn is_valid(&self) -> bool {
        self.validate().is_ok()
    }
}

impl fmt::Display for LanguageTag {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for LanguageTag {
    type Err = ParseError;

    fn from_str(input: &str) -> Result<Self, ParseError> {
        Self::parse(input)
    }
}

const GRANDFATHERED: [&'static str; 26] = [
    "en-GB-oed",
    "i-ami",
    "i-bnn",
    "i-default",
    "i-enochian",
    "i-hak",
    "i-klingon",
    "i-lux",
    "i-mingo",
    "i-navajo",
    "i-pwn",
    "i-tao",
    "i-tay",
    "i-tsu",
    "sgn-BE-FR",
    "sgn-BE-NL",
    "sgn-CH-DE",
    "art-lojban",
    "cel-gaulish",
    "no-bok",
    "no-nyn",
    "zh-guoyu",
    "zh-hakka",
    "zh-min",
    "zh-min-nan",
    "zh-xiang",
];

struct SubtagListIterator<'a> {
    split: Split<'a, char>,
}

impl<'a> SubtagListIterator<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            split: input.split('-'),
        }
    }
}

impl<'a> Iterator for SubtagListIterator<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        let tag = self.split.next()?;
        if tag.is_empty() {
            None
        } else {
            Some(tag)
        }
    }
}

struct ExtensionsIterator<'a> {
    split: Split<'a, char>,
    singleton: Option<char>,
}

impl<'a> ExtensionsIterator<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            split: input.split('-'),
            singleton: None,
        }
    }
}

impl<'a> Iterator for ExtensionsIterator<'a> {
    type Item = (char, &'a str);

    fn next(&mut self) -> Option<(char, &'a str)> {
        let tag = self.split.next()?;
        if tag.is_empty() {
            None
        } else if tag.len() == 1 {
            self.singleton = tag.chars().next();
            self.next()
        } else if let Some(singleton) = self.singleton {
            Some((singleton, tag))
        } else {
            panic!("No singleton found in extension")
        }
    }
}

struct SubTagIterator<'a> {
    split: Split<'a, char>,
    position: usize,
}

impl<'a> SubTagIterator<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            split: input.split('-'),
            position: 0,
        }
    }
}

impl<'a> Iterator for SubTagIterator<'a> {
    type Item = (&'a str, usize);

    fn next(&mut self) -> Option<(&'a str, usize)> {
        let tag = self.split.next()?;
        let tag_end = self.position + tag.len();
        self.position = tag_end + 1;
        Some((tag, tag_end))
    }
}

struct AlphanumericLowerCharSet {
    alphabetic_set: [bool; 26],
    numeric_set: [bool; 10],
}

impl AlphanumericLowerCharSet {
    fn new() -> Self {
        Self {
            alphabetic_set: [false; 26],
            numeric_set: [false; 10],
        }
    }

    fn contains(&mut self, c: char) -> bool {
        if c.is_ascii_digit() {
            self.numeric_set[char_sub(c, '0')]
        } else if c.is_ascii_lowercase() {
            self.alphabetic_set[char_sub(c, 'a')]
        } else if c.is_ascii_uppercase() {
            self.alphabetic_set[char_sub(c, 'A')]
        } else {
            false
        }
    }

    fn insert(&mut self, c: char) {
        if c.is_ascii_digit() {
            self.numeric_set[char_sub(c, '0')] = true
        } else if c.is_ascii_lowercase() {
            self.alphabetic_set[char_sub(c, 'a')] = true
        } else if c.is_ascii_uppercase() {
            self.alphabetic_set[char_sub(c, 'A')] = true
        }
    }
}

fn char_sub(c1: char, c2: char) -> usize {
    (c1 as usize) - (c2 as usize)
}

fn is_alphabetic(s: &str) -> bool {
    s.chars().all(|x| x.is_ascii_alphabetic())
}

fn is_numeric(s: &str) -> bool {
    s.chars().all(|x| x.is_ascii_digit())
}

fn is_alphanumeric(s: &str) -> bool {
    s.chars().all(|x| x.is_ascii_alphanumeric())
}

fn is_alphanumeric_or_dash(s: &str) -> bool {
    s.chars().all(|x| x.is_ascii_alphanumeric() || x == '-')
}

fn to_uppercase<'a>(s: &'a str) -> impl Iterator<Item = char> + 'a {
    s.chars().map(|c| c.to_ascii_uppercase())
}

// Beware: panics if s.len() == 0 (should never happen in our code)
fn to_uppercase_first<'a>(s: &'a str) -> impl Iterator<Item = char> + 'a {
    let mut chars = s.chars();
    once(chars.next().unwrap().to_ascii_uppercase()).chain(chars.map(|c| c.to_ascii_lowercase()))
}

fn to_lowercase<'a>(s: &'a str) -> impl Iterator<Item = char> + 'a {
    s.chars().map(|c| c.to_ascii_lowercase())
}

#[derive(Debug, Eq, PartialEq)]
pub enum ParseError {
    /// If an extension subtag is present, it must not be empty.
    EmptyExtension,
    /// If the `x` subtag is present, it must not be empty.
    EmptyPrivateUse,
    /// The langtag contains a char that is not A-Z, a-z, 0-9 or the dash.
    ForbiddenChar,
    /// A subtag fails to parse, it does not match any other subtags.
    InvalidSubtag,
    /// The given language subtag is invalid.
    InvalidLanguage,
    /// A subtag may be eight characters in length at maximum.
    SubtagTooLong,
    /// A subtag should not be empty.
    EmptySubtag,
    /// At maximum three extlangs are allowed, but zero to one extlangs are preferred.
    TooManyExtlangs,
}

impl Error for ParseError {
    fn description(&self) -> &str {
        match self {
            ParseError::EmptyExtension => "If an extension subtag is present, it must not be empty",
            ParseError::EmptyPrivateUse => "If the `x` subtag is present, it must not be empty",
            ParseError::ForbiddenChar => "The langtag contains a char not allowed",
            ParseError::InvalidSubtag => {
                "A subtag fails to parse, it does not match any other subtags"
            }
            ParseError::InvalidLanguage => "The given language subtag is invalid",
            ParseError::SubtagTooLong => "A subtag may be eight characters in length at maximum",
            ParseError::EmptySubtag => "A subtag should not be empty",
            ParseError::TooManyExtlangs => "At maximum three extlangs are allowed",
        }
    }
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum ValidationError {
    /// The same variant subtag is only allowed once in a tag.
    DuplicateVariant,
    /// The same extension subtag is only allowed once in a tag before the private use part.
    DuplicateExtension,
    /// only one extended language subtag is allowed
    MultipleExtendedLanguageSubtags,
}

impl Error for ValidationError {
    fn description(&self) -> &str {
        match self {
            ValidationError::DuplicateVariant => {
                "The same variant subtag is only allowed once in a tag"
            }
            ValidationError::DuplicateExtension => {
                "The same extension subtag is only allowed once in a tag"
            }
            ValidationError::MultipleExtendedLanguageSubtags => {
                "only one extended language subtag is allowed"
            }
        }
    }
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.description())
    }
}

// Tests from RFC 5646 2.1.1
#[test]
fn test_formatting() {
    assert_eq!(
        "mn-Cyrl-MN",
        LanguageTag::from_str("mn-Cyrl-MN").unwrap().as_str()
    );
    assert_eq!(
        "mn-Cyrl-MN",
        LanguageTag::from_str("MN-cYRL-mn").unwrap().as_str()
    );
    assert_eq!(
        "mn-Cyrl-MN",
        LanguageTag::from_str("mN-cYrL-Mn").unwrap().as_str()
    );
    assert_eq!(
        "en-CA-x-ca",
        LanguageTag::from_str("en-CA-x-ca").unwrap().as_str()
    );
    assert_eq!(
        "sgn-BE-FR",
        LanguageTag::from_str("sgn-BE-FR").unwrap().as_str()
    );
    assert_eq!(
        "az-Latn-x-latn",
        LanguageTag::from_str("az-Latn-x-latn").unwrap().as_str()
    );
    assert_eq!("i-ami", LanguageTag::from_str("i-ami").unwrap().as_str());
    assert_eq!("i-ami", LanguageTag::from_str("I-AMI").unwrap().as_str());
}

// Tests from RFC 5646 2.2.1
#[test]
fn test_primary_language() {
    assert_eq!(
        "fr",
        LanguageTag::from_str("fr").unwrap().primary_language()
    );
    assert_eq!(
        "x-fr-ch",
        LanguageTag::from_str("x-fr-CH").unwrap().primary_language()
    );
    assert_eq!(
        "i-klingon",
        LanguageTag::from_str("i-klingon")
            .unwrap()
            .primary_language()
    );
    assert_eq!(
        "i-bnn",
        LanguageTag::from_str("i-bnn").unwrap().primary_language()
    );
}

// Tests from RFC 5646 2.2.2
#[test]
fn test_extended_language() {
    fn parts(tag: &LanguageTag) -> (&str, &str, Option<&str>, Vec<&str>) {
        (
            tag.full_language(),
            tag.primary_language(),
            tag.extended_language(),
            tag.extended_language_subtags().collect(),
        )
    }

    assert_eq!(
        ("zh", "zh", None, vec![]),
        parts(&LanguageTag::from_str("zh").unwrap())
    );
    assert_eq!(
        ("zh-gan", "zh", Some("gan"), vec!["gan"]),
        parts(&LanguageTag::from_str("zh-gan").unwrap())
    );
    assert_eq!(
        ("zh-gan-foo", "zh", Some("gan-foo"), vec!["gan", "foo"]),
        parts(&LanguageTag::from_str("zh-gan-foo").unwrap())
    );
    assert_eq!(
        ("zh-min-nan", "zh-min-nan", None, vec![]),
        parts(&LanguageTag::from_str("zh-min-nan").unwrap())
    );
    assert_eq!(
        ("i-tsu", "i-tsu", None, vec![]),
        parts(&LanguageTag::from_str("i-tsu").unwrap())
    );
    assert_eq!(
        ("zh", "zh", None, vec![]),
        parts(&LanguageTag::from_str("zh-CN").unwrap())
    );
    assert_eq!(
        ("zh-gan", "zh", Some("gan"), vec!["gan"]),
        parts(&LanguageTag::from_str("zh-gan-CN").unwrap())
    );
}

// Tests from RFC 5646 2.2.3
#[test]
fn test_script() {
    fn parts(tag: &LanguageTag) -> (&str, Option<&str>) {
        (tag.primary_language(), tag.script())
    }

    assert_eq!(
        ("sr", Some("Latn")),
        parts(&LanguageTag::from_str("sr-Latn").unwrap())
    );
}

// Tests from RFC 5646 2.2.4
#[test]
fn test_region() {
    fn parts(tag: &LanguageTag) -> (&str, Option<&str>, Option<&str>) {
        (tag.primary_language(), tag.script(), tag.region())
    }

    assert_eq!(
        ("de", None, Some("AT")),
        parts(&LanguageTag::from_str("de-AT").unwrap())
    );
    assert_eq!(
        ("sr", Some("Latn"), Some("RS")),
        parts(&LanguageTag::from_str("sr-Latn-RS").unwrap())
    );
    assert_eq!(
        ("es", None, Some("419")),
        parts(&LanguageTag::from_str("es-419").unwrap())
    );
}

// Tests from RFC 5646 2.2.5
#[test]
fn test_variant() {
    fn parts(tag: &LanguageTag) -> (&str, Option<&str>, Vec<&str>) {
        (
            tag.primary_language(),
            tag.variant(),
            tag.variant_subtags().collect(),
        )
    }

    assert_eq!(
        ("sl", None, vec![]),
        parts(&LanguageTag::from_str("sl").unwrap())
    );
    assert_eq!(
        ("sl", Some("nedis"), vec!["nedis"]),
        parts(&LanguageTag::from_str("sl-nedis").unwrap())
    );
    assert_eq!(
        ("de", Some("1996"), vec!["1996"]),
        parts(&LanguageTag::from_str("de-CH-1996").unwrap())
    );
    assert_eq!(
        ("art-lojban", None, vec![]),
        parts(&LanguageTag::from_str("art-lojban").unwrap())
    );
}

// Tests from RFC 5646 2.2.6
#[test]
fn test_extension() {
    fn parts(tag: &LanguageTag) -> (&str, Option<&str>, Vec<(char, &str)>) {
        (
            tag.primary_language(),
            tag.extension(),
            tag.extension_subtags().collect(),
        )
    }

    assert_eq!(
        ("en", None, vec![]),
        parts(&LanguageTag::from_str("en").unwrap())
    );
    assert_eq!(
        ("en", Some("a-bbb"), vec![('a', "bbb")]),
        parts(&LanguageTag::from_str("en-a-bbb-x-a-ccc").unwrap())
    );
    assert_eq!(
        ("fr", Some("a-latn"), vec![('a', "latn")]),
        parts(&LanguageTag::from_str("fr-a-Latn").unwrap())
    );
    assert_eq!(
        (
            "en",
            Some("r-extended-sequence"),
            vec![('r', "extended"), ('r', "sequence")]
        ),
        parts(&LanguageTag::from_str("en-Latn-GB-boont-r-extended-sequence-x-private").unwrap())
    );
    assert_eq!(
        ("i-tsu", None, vec![]),
        parts(&LanguageTag::from_str("i-tsu").unwrap())
    );
}

// Tests from RFC 5646 2.2.7
#[test]
fn test_privateuse() {
    fn parts(tag: &LanguageTag) -> (&str, Option<&str>, Vec<&str>) {
        (
            tag.primary_language(),
            tag.private_use(),
            tag.private_use_subtags().collect(),
        )
    }

    assert_eq!(
        ("en", None, vec![]),
        parts(&LanguageTag::from_str("en").unwrap())
    );
    assert_eq!(
        ("en", Some("x-us"), vec!["us"]),
        parts(&LanguageTag::from_str("en-x-US").unwrap())
    );
    assert_eq!(
        ("el", Some("x-koine"), vec!["koine"]),
        parts(&LanguageTag::from_str("el-x-koine").unwrap())
    );
    assert_eq!(
        ("x-fr-ch", Some("x-fr-ch"), vec!["fr", "ch"]),
        parts(&LanguageTag::from_str("x-fr-ch").unwrap())
    );
}

// Tests from RFC 5646 2.2.9
#[test]
fn test_is_valid() {
    assert!(LanguageTag::from_str("sr-Latn-RS").unwrap().is_valid());
    assert!(!LanguageTag::from_str("de-DE-1901-aaaaa-1901")
        .unwrap()
        .is_valid());
    assert!(!LanguageTag::from_str("en-a-bbb-a-ccc").unwrap().is_valid());
    assert!(!LanguageTag::from_str("ab-c-abc-r-toto-c-abc")
        .unwrap()
        .is_valid());
    assert!(LanguageTag::from_str("en-a-bbb-x-a-ccc")
        .unwrap()
        .is_valid());
}

// http://www.langtag.net/test-suites/well-formed-tags.txt
#[test]
fn test_wellformed_tags() {
    let tags = vec![
        "fr",
        "fr-Latn",
        "fr-fra", // Extended tag
        "fr-Latn-FR",
        "fr-Latn-419",
        "fr-FR",
        "ax-TZ",     // Not in the registry, but well-formed
        "fr-shadok", // Variant
        "fr-y-myext-myext2",
        "fra-Latn", // ISO 639 can be 3-letters
        "fra",
        "fra-FX",
        "i-klingon", // grandfathered with singleton
        "I-kLINgon", // tags are case-insensitive...
        "no-bok",    // grandfathered without singleton
        "fr-Lat",    // Extended",
        "mn-Cyrl-MN",
        "mN-cYrL-Mn",
        "fr-Latn-CA",
        "en-US",
        "fr-Latn-CA",
        "i-enochian", // Grand fathered
        "x-fr-CH",
        "sr-Latn-CS",
        "es-419",
        "sl-nedis",
        "de-CH-1996",
        "de-Latg-1996",
        "sl-IT-nedis",
        "en-a-bbb-x-a-ccc",
        "de-a-value",
        "en-Latn-GB-boont-r-extended-sequence-x-private",
        "en-x-US",
        "az-Arab-x-AZE-derbend",
        "es-Latn-CO-x-private",
        "en-US-boont",
        "ab-x-abc-x-abc",     // anything goes after x
        "ab-x-abc-a-a",       // ditto",
        "i-default",          // grandfathered",
        "i-klingon",          // grandfathered",
        "abcd-Latn",          // Language of 4 chars reserved for future use
        "AaBbCcDd-x-y-any-x", // Language of 5-8 chars, registered
        "en",
        "de-AT",
        "es-419",
        "de-CH-1901",
        "sr-Cyrl",
        "sr-Cyrl-CS",
        "sl-Latn-IT-rozaj",
        "en-US-x-twain",
        "zh-cmn",
        "zh-cmn-Hant",
        "zh-cmn-Hant-HK",
        "zh-gan",
        "zh-yue-Hant-HK",
        "xr-lxs-qut", // extlangS
        "xr-lqt-qu",  // extlang + region
        "xr-p-lze",   // Extension
    ];
    for tag in tags {
        let result = LanguageTag::from_str(tag);
        assert!(
            result.is_ok(),
            "{} should be considered well-formed but returned error {}",
            tag,
            result.err().unwrap()
        );
    }
}

// http://www.langtag.net/test-suites/broken-tags.txt
#[test]
fn test_broken_tags() {
    let tags = vec![
        "f",
        "f-Latn",
        "fr-Latn-F",
        "a-value",
        "tlh-a-b-foo",
        "i-notexist", // grandfathered but not registered: always invalid
        "abcdefghi-012345678",
        "ab-abc-abc-abc-abc",
        "ab-abcd-abc",
        "ab-ab-abc",
        "ab-123-abc",
        "a-Hant-ZH",
        "a1-Hant-ZH",
        "ab-abcde-abc",
        "ab-1abc-abc",
        "ab-ab-abcd",
        "ab-123-abcd",
        "ab-abcde-abcd",
        "ab-1abc-abcd",
        "ab-a-b",
        "ab-a-x",
        "ab--ab",
        "ab-abc-",
        "-ab-abc",
        "abcd-efg",
        "aabbccddE",
    ];
    for tag in tags {
        let result = LanguageTag::from_str(tag);
        assert!(
            result.is_err(),
            "{} should be considered not well-formed but returned result {:?}",
            tag,
            result.ok().unwrap()
        );
    }
}

// http://www.langtag.net/test-suites/valid-tags.txt
#[test]
fn test_valid_tags() {
    let tags = vec![
        "fr",
        "fr-Latn",
        "fr-fra", // Extended tag
        "fr-Latn-FR",
        "fr-Latn-419",
        "fr-FR",
        "fr-y-myext-myext2",
        "apa-Latn", // ISO 639 can be 3-letters
        "apa",
        "apa-CA",
        "i-klingon", // grandfathered with singleton
        "no-bok",    // grandfathered without singleton
        "fr-Lat",    // Extended
        "mn-Cyrl-MN",
        "mN-cYrL-Mn",
        "fr-Latn-CA",
        "en-US",
        "fr-Latn-CA",
        "i-enochian", // Grand fathered
        "x-fr-CH",
        "sr-Latn-CS",
        "es-419",
        "sl-nedis",
        "de-CH-1996",
        "de-Latg-1996",
        "sl-IT-nedis",
        "en-a-bbb-x-a-ccc",
        "de-a-value",
        "en-x-US",
        "az-Arab-x-AZE-derbend",
        "es-Latn-CO-x-private",
        "ab-x-abc-x-abc", // anything goes after x
        "ab-x-abc-a-a",   // ditto
        "i-default",      // grandfathered
        "i-klingon",      // grandfathered
        "en",
        "de-AT",
        "es-419",
        "de-CH-1901",
        "sr-Cyrl",
        "sr-Cyrl-CS",
        "sl-Latn-IT-rozaj",
        "en-US-x-twain",
        "zh-cmn",
        "zh-cmn-Hant",
        "zh-cmn-Hant-HK",
        "zh-gan",
        "zh-yue-Hant-HK",
        "en-Latn-GB-boont-r-extended-sequence-x-private",
        "en-US-boont",
    ];
    for tag in tags {
        let result = LanguageTag::from_str(tag);
        assert!(
            result.is_ok(),
            "{} should be considered well-formed but returned error {}",
            tag,
            result.err().unwrap()
        );
        let validation = result.unwrap().validate();
        assert!(
            validation.is_ok(),
            "{} should be considered valid but returned error {}",
            tag,
            validation.err().unwrap()
        );
    }
}

// http://www.langtag.net/test-suites/invalid-tags.txt
#[test]
fn test_invalid_tags() {
    let tags = vec![
        "en-a-bbb-a-ccc", // 'a' appears twice, moved from broken_tags
        "ab-c-abc-r-toto-c-abc", // 'c' appears twice ", moved from broken_tags
                          //TODO "ax-TZ",    // Not in the registry, but well-formed
                          //TODO "fra-Latn", // ISO 639 can be 3-letters
                          //TODO "fra",
                          //TODO "fra-FX",
                          //TODO "abcd-Latn",          // Language of 4 chars reserved for future use
                          //TODO "AaBbCcDd-x-y-any-x", // Language of 5-8 chars, registered
                          //TODO "zh-Latm-CN",         // Typo
                          //TODO "de-DE-1902",         // Wrong variant
                          //TODO "fr-shadok",          // Variant
    ];
    for tag in tags {
        let result = LanguageTag::from_str(tag);
        assert!(
            result.is_ok(),
            "{} should be considered well-formed but returned error {}",
            tag,
            result.err().unwrap()
        );
        let validation = result.unwrap().validate();
        assert!(validation.is_err(), "{} should be considered invalid", tag);
    }
}
