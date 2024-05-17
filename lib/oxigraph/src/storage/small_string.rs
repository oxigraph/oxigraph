use std::borrow::Borrow;
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};
use std::ops::Deref;
use std::str::{FromStr, Utf8Error};
use std::{fmt, str};

/// A small inline string
#[derive(Clone, Copy, Default)]
pub struct SmallString {
    inner: [u8; 16],
}

impl SmallString {
    #[inline]
    pub const fn new() -> Self {
        Self { inner: [0; 16] }
    }

    #[inline]
    pub fn from_utf8(bytes: &[u8]) -> Result<Self, BadSmallStringError> {
        Self::from_str(str::from_utf8(bytes).map_err(BadSmallStringError::BadUtf8)?)
    }

    #[inline]
    pub fn from_be_bytes(bytes: [u8; 16]) -> Result<Self, BadSmallStringError> {
        // We check that it is valid UTF-8
        str::from_utf8(&bytes.as_ref()[..bytes[15].into()])
            .map_err(BadSmallStringError::BadUtf8)?;
        Ok(Self { inner: bytes })
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.inner[15].into()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    #[inline]
    #[allow(unsafe_code)]
    pub fn as_str(&self) -> &str {
        // SAFETY: safe because we ensured it in constructors
        unsafe { str::from_utf8_unchecked(self.as_bytes()) }
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.inner[..self.len()]
    }

    #[inline]
    pub fn to_be_bytes(self) -> [u8; 16] {
        self.inner
    }
}

impl Deref for SmallString {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl AsRef<str> for SmallString {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for SmallString {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Debug for SmallString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl fmt::Display for SmallString {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl PartialEq for SmallString {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for SmallString {}

impl PartialOrd for SmallString {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SmallString {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for SmallString {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl From<SmallString> for String {
    #[inline]
    fn from(value: SmallString) -> Self {
        value.as_str().into()
    }
}

impl<'a> From<&'a SmallString> for &'a str {
    #[inline]
    fn from(value: &'a SmallString) -> Self {
        value.as_str()
    }
}

impl FromStr for SmallString {
    type Err = BadSmallStringError;

    #[inline]
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.len() <= 15 {
            let mut inner = [0; 16];
            inner[..value.len()].copy_from_slice(value.as_bytes());
            inner[15] = value
                .len()
                .try_into()
                .map_err(|_| Self::Err::TooLong(value.len()))?;
            Ok(Self { inner })
        } else {
            Err(Self::Err::TooLong(value.len()))
        }
    }
}

impl<'a> TryFrom<&'a str> for SmallString {
    type Error = BadSmallStringError;

    #[inline]
    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        Self::from_str(value)
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum BadSmallStringError {
    #[error("small strings could only contain at most 15 characters, found {0}")]
    TooLong(usize),
    #[error(transparent)]
    BadUtf8(#[from] Utf8Error),
}
