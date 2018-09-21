use std::borrow::Cow;
use std::char;
use std::str::Chars;
use utils::StaticSliceMap;

pub fn unescape_unicode_codepoints(input: &str) -> Cow<str> {
    if needs_unescape_unicode_codepoints(input) {
        UnescapeUnicodeCharIterator::new(input).collect()
    } else {
        input.into()
    }
}

fn needs_unescape_unicode_codepoints(input: &str) -> bool {
    let bytes = input.as_bytes();
    for i in 1..bytes.len() {
        if (bytes[i] == b'u' || bytes[i] == b'U') && bytes[i - 1] == b'\\' {
            return true;
        }
    }
    false
}

struct UnescapeUnicodeCharIterator<'a> {
    iter: Chars<'a>,
    buffer: String,
}

impl<'a> UnescapeUnicodeCharIterator<'a> {
    fn new(string: &'a str) -> Self {
        Self {
            iter: string.chars(),
            buffer: String::with_capacity(9),
        }
    }
}

impl<'a> Iterator for UnescapeUnicodeCharIterator<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        if !self.buffer.is_empty() {
            return Some(self.buffer.remove(0));
        }
        match self.iter.next()? {
            '\\' => match self.iter.next() {
                Some('u') => {
                    self.buffer.push('u');
                    for _ in 0..4 {
                        if let Some(c) = self.iter.next() {
                            self.buffer.push(c);
                        } else {
                            return Some('\\');
                        }
                    }
                    if let Some(c) = u32::from_str_radix(&self.buffer[1..5], 16)
                        .ok()
                        .and_then(char::from_u32)
                    {
                        self.buffer.clear();
                        Some(c)
                    } else {
                        Some('\\')
                    }
                }
                Some('U') => {
                    self.buffer.push('U');
                    for _ in 0..8 {
                        if let Some(c) = self.iter.next() {
                            self.buffer.push(c);
                        } else {
                            return Some('\\');
                        }
                    }
                    if let Some(c) = u32::from_str_radix(&self.buffer[1..9], 16)
                        .ok()
                        .and_then(char::from_u32)
                    {
                        self.buffer.clear();
                        Some(c)
                    } else {
                        Some('\\')
                    }
                }
                Some(c) => {
                    self.buffer.push(c);
                    Some('\\')
                }
                None => Some('\\'),
            },
            c => Some(c),
        }
    }
}

pub fn unescape_characters<'a>(
    input: &'a str,
    characters: &'static [u8],
    replacement: &'static StaticSliceMap<char, char>,
) -> Cow<'a, str> {
    if needs_unescape_characters(input, characters) {
        UnescapeCharsIterator::new(input, replacement).collect()
    } else {
        input.into()
    }
}

fn needs_unescape_characters(input: &str, characters: &[u8]) -> bool {
    let bytes = input.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i - 1] == b'\\' && characters.contains(&bytes[i]) {
            return true;
        }
    }
    false
}

struct UnescapeCharsIterator<'a> {
    iter: Chars<'a>,
    buffer: Option<char>,
    replacement: &'static StaticSliceMap<char, char>,
}

impl<'a> UnescapeCharsIterator<'a> {
    fn new(string: &'a str, replacement: &'static StaticSliceMap<char, char>) -> Self {
        Self {
            iter: string.chars(),
            buffer: None,
            replacement,
        }
    }
}

impl<'a> Iterator for UnescapeCharsIterator<'a> {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        if let Some(ch) = self.buffer {
            self.buffer = None;
            return Some(ch);
        }
        match self.iter.next()? {
            '\\' => match self.iter.next() {
                Some(ch) => match self.replacement.get(ch) {
                    Some(replace) => Some(replace),
                    None => {
                        self.buffer = Some(ch);
                        Some('\\')
                    }
                },
                None => Some('\\'),
            },
            c => Some(c),
        }
    }
}
