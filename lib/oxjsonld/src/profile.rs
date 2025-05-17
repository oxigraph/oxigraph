use std::ops::{BitOr, BitOrAssign};

/// JSON-Ld profile.
///
/// This enumeration is non exhaustive. New profiles might be added in the future.
///
/// See [JSON-LD specification](https://www.w3.org/TR/json-ld11/#iana-considerations) for a list of profiles.
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash)]
#[non_exhaustive]
pub enum JsonLdProfile {
    /// [expanded JSON-LD document form](https://www.w3.org/TR/json-ld11/#dfn-expanded-document-form)
    Expanded,
    /// [compacted JSON-LD document form](https://www.w3.org/TR/json-ld11/#dfn-compacted-document-form)
    Compacted,
    /// [JSON-LD context document](https://www.w3.org/TR/json-ld11/#dfn-context-document)
    Context,
    /// [flattened JSON-LD document form](https://www.w3.org/TR/json-ld11/#dfn-flattened-document-form)
    Flattened,
    /// [JSON-LD frame document](https://www.w3.org/TR/json-ld11-framing/#dfn-frame)
    Frame,
    /// [framed JSON-LD document form](https://www.w3.org/TR/json-ld11/#dfn-framed-document-form)
    Framed,
    /// [streaming JSON-LD document form](https://www.w3.org/TR/json-ld11-streaming/#dfn-streaming-document)
    Streaming,
}

impl JsonLdProfile {
    /// The profile canonical IRI.
    ///
    /// ```
    /// use oxjsonld::JsonLdProfile;
    ///
    /// assert_eq!(
    ///     JsonLdProfile::Expanded.iri(),
    ///     "http://www.w3.org/ns/json-ld#expanded"
    /// )
    /// ```
    #[inline]
    pub const fn iri(self) -> &'static str {
        match self {
            Self::Expanded => "http://www.w3.org/ns/json-ld#expanded",
            Self::Compacted => "http://www.w3.org/ns/json-ld#compacted",
            Self::Context => "http://www.w3.org/ns/json-ld#context",
            Self::Flattened => "http://www.w3.org/ns/json-ld#flattened",
            Self::Frame => "http://www.w3.org/ns/json-ld#frame",
            Self::Framed => "http://www.w3.org/ns/json-ld#framed",
            Self::Streaming => "http://www.w3.org/ns/json-ld#streaming",
        }
    }

    /// Looks for a known profile from an IRI.
    ///
    /// Example:
    /// ```
    /// use oxjsonld::JsonLdProfile;
    ///
    /// assert_eq!(
    ///     JsonLdProfile::from_iri("http://www.w3.org/ns/json-ld#expanded"),
    ///     Some(JsonLdProfile::Expanded)
    /// )
    /// ```
    #[inline]
    pub fn from_iri(iri: &str) -> Option<Self> {
        match iri {
            "http://www.w3.org/ns/json-ld#expanded" => Some(Self::Expanded),
            "http://www.w3.org/ns/json-ld#compacted" => Some(Self::Compacted),
            "http://www.w3.org/ns/json-ld#context" => Some(Self::Context),
            "http://www.w3.org/ns/json-ld#flattened" => Some(Self::Flattened),
            "http://www.w3.org/ns/json-ld#frame" => Some(Self::Frame),
            "http://www.w3.org/ns/json-ld#framed" => Some(Self::Framed),
            "http://www.w3.org/ns/json-ld#streaming" => Some(Self::Streaming),
            _ => None,
        }
    }

    #[inline]
    const fn to_bit(self) -> u64 {
        match self {
            JsonLdProfile::Expanded => 1,
            JsonLdProfile::Compacted => 2,
            JsonLdProfile::Context => 4,
            JsonLdProfile::Flattened => 8,
            JsonLdProfile::Frame => 16,
            JsonLdProfile::Framed => 32,
            JsonLdProfile::Streaming => 64,
        }
    }
}

/// Set of JSON-Ld profiles.
///
/// ```
/// use oxjsonld::{JsonLdProfile, JsonLdProfileSet};
///
/// let mut profile_set = JsonLdProfileSet::empty();
/// profile_set |= JsonLdProfile::Expanded;
/// profile_set |= JsonLdProfile::Streaming;
/// assert!(profile_set.contains(JsonLdProfile::Streaming));
/// assert_eq!(
///     profile_set.into_iter().collect::<Vec<_>>(),
///     vec![JsonLdProfile::Expanded, JsonLdProfile::Streaming]
/// );
/// ```
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Default)]
pub struct JsonLdProfileSet {
    value: u64,
}

impl JsonLdProfileSet {
    #[inline]
    pub const fn empty() -> Self {
        Self { value: 0 }
    }

    #[inline]
    pub const fn from_profile(profile: JsonLdProfile) -> Self {
        Self {
            value: profile.to_bit(),
        }
    }

    /// Checks if this profile list contains the given profile.
    #[inline]
    pub const fn contains(self, profile: JsonLdProfile) -> bool {
        self.value & profile.to_bit() != 0
    }
}

impl From<JsonLdProfile> for JsonLdProfileSet {
    #[inline]
    fn from(profile: JsonLdProfile) -> Self {
        Self {
            value: profile.to_bit(),
        }
    }
}

impl IntoIterator for JsonLdProfileSet {
    type Item = JsonLdProfile;
    type IntoIter = JsonLdProfileBagIter;

    #[inline]
    fn into_iter(self) -> JsonLdProfileBagIter {
        JsonLdProfileBagIter {
            set: self,
            possible_values: [
                JsonLdProfile::Expanded,
                JsonLdProfile::Compacted,
                JsonLdProfile::Context,
                JsonLdProfile::Flattened,
                JsonLdProfile::Frame,
                JsonLdProfile::Framed,
                JsonLdProfile::Streaming,
            ]
            .into_iter(),
        }
    }
}

/// Iterator output of [`JsonLdProfileSet::into_iter`].
pub struct JsonLdProfileBagIter {
    set: JsonLdProfileSet,
    possible_values: std::array::IntoIter<JsonLdProfile, 7>,
}

impl Iterator for JsonLdProfileBagIter {
    type Item = JsonLdProfile;

    #[inline]
    fn next(&mut self) -> Option<JsonLdProfile> {
        loop {
            let possible_value = self.possible_values.next()?;
            if self.set.contains(possible_value) {
                return Some(possible_value);
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.set.value.count_ones().try_into().unwrap();
        (size, Some(size))
    }
}

impl BitOr for JsonLdProfileSet {
    type Output = Self;

    #[inline]
    fn bitor(self, rhs: Self) -> Self {
        Self {
            value: self.value | rhs.value,
        }
    }
}

impl BitOr<JsonLdProfile> for JsonLdProfileSet {
    type Output = JsonLdProfileSet;

    #[inline]
    fn bitor(self, rhs: JsonLdProfile) -> JsonLdProfileSet {
        self | JsonLdProfileSet::from(rhs)
    }
}

impl BitOr<JsonLdProfileSet> for JsonLdProfile {
    type Output = JsonLdProfileSet;

    #[inline]
    fn bitor(self, rhs: JsonLdProfileSet) -> JsonLdProfileSet {
        JsonLdProfileSet::from(self) | rhs
    }
}

impl BitOr for JsonLdProfile {
    type Output = JsonLdProfileSet;

    #[inline]
    fn bitor(self, rhs: Self) -> JsonLdProfileSet {
        JsonLdProfileSet::from(self) | JsonLdProfileSet::from(rhs)
    }
}

impl BitOrAssign for JsonLdProfileSet {
    #[inline]
    fn bitor_assign(&mut self, rhs: Self) {
        self.value |= rhs.value;
    }
}

impl BitOrAssign<JsonLdProfile> for JsonLdProfileSet {
    #[inline]
    fn bitor_assign(&mut self, rhs: JsonLdProfile) {
        *self |= JsonLdProfileSet::from(rhs);
    }
}

/// JSON-LD [processing mode](https://www.w3.org/TR/json-ld11/#dfn-processing-mode)
#[derive(Eq, PartialEq, Debug, Clone, Copy, Hash, Default)]
pub enum JsonLdProcessingMode {
    #[default]
    JsonLd1_0,
    JsonLd1_1, // TODO: Move to 1.1 when implemented
}

impl JsonLdProcessingMode {
    /// The string identifier.
    ///
    /// ```
    /// use oxjsonld::JsonLdProcessingMode;
    ///
    /// assert_eq!(JsonLdProcessingMode::JsonLd1_0.as_str(), "json-ld-1.0");
    /// ```
    #[inline]
    pub const fn as_str(self) -> &'static str {
        match self {
            JsonLdProcessingMode::JsonLd1_0 => "json-ld-1.0",
            JsonLdProcessingMode::JsonLd1_1 => "json-ld-1.1",
        }
    }

    /// From a string identifier.
    ///
    /// ```
    /// use oxjsonld::JsonLdProcessingMode;
    ///
    /// assert_eq!(
    ///     JsonLdProcessingMode::from_id("json-ld-1.1"),
    ///     Some(JsonLdProcessingMode::JsonLd1_1)
    /// );
    /// ```
    #[inline]
    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            "json-ld-1.0" => Some(JsonLdProcessingMode::JsonLd1_0),
            "json-ld-1.1" => Some(JsonLdProcessingMode::JsonLd1_1),
            _ => None,
        }
    }
}
