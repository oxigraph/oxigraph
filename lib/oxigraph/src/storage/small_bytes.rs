//! Fixed-capacity inline binary buffer.
//!
//! [`SmallBytes`] is the binary analogue of [`crate::storage::small_string::SmallString`]:
//! up to `N - 1` bytes inline, with the trailing byte holding the populated
//! length. Unlike `SmallString` the contents are not constrained to UTF-8,
//! which makes this type the right fit for inline storage of binary
//! payloads (WKB-encoded geometries, packed coordinates, and similar).
//!
//! The type is `Copy` and `N` bytes wide regardless of how many bytes the
//! caller wrote, which keeps it cheap to embed in `EncodedTerm`. Equality,
//! ordering and hashing operate over the populated prefix only.

use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

/// Fixed-capacity inline binary buffer holding up to `N - 1` bytes.
///
/// The last byte (`inner[N - 1]`) stores the populated length. Callers
/// must treat the unused tail as opaque padding.
#[derive(Clone, Copy)]
pub struct SmallBytes<const N: usize> {
    inner: [u8; N],
}

impl<const N: usize> SmallBytes<N> {
    /// Maximum number of payload bytes that can be stored inline.
    pub const CAPACITY: usize = N - 1;

    /// Construct an empty buffer.
    #[inline]
    pub const fn new() -> Self {
        Self { inner: [0; N] }
    }

    /// Number of payload bytes currently stored.
    #[inline]
    pub fn len(&self) -> usize {
        self.inner[N - 1].into()
    }

    /// Whether the buffer is empty.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Borrow the populated payload as a byte slice.
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        &self.inner[..self.len()]
    }

    /// Return the underlying fixed-size array, including the length byte
    /// in the trailing slot. Useful for callers that need to write the
    /// buffer directly into a fixed-width on-disk encoding.
    #[inline]
    pub fn to_be_bytes(self) -> [u8; N] {
        self.inner
    }

    /// Reconstruct a buffer from a previously emitted fixed-size array.
    ///
    /// Returns an error if the trailing length byte exceeds [`Self::CAPACITY`].
    #[inline]
    pub fn from_be_bytes(bytes: [u8; N]) -> Result<Self, BadSmallBytesError> {
        let len: usize = bytes[N - 1].into();
        if len > Self::CAPACITY {
            return Err(BadSmallBytesError::TooLong(len));
        }
        Ok(Self { inner: bytes })
    }
}

impl<const N: usize> Default for SmallBytes<N> {
    #[inline]
    fn default() -> Self {
        Self::new()
    }
}

impl<const N: usize> AsRef<[u8]> for SmallBytes<N> {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl<const N: usize> std::fmt::Debug for SmallBytes<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SmallBytes")
            .field("len", &self.len())
            .field("bytes", &self.as_slice())
            .finish()
    }
}

impl<const N: usize> PartialEq for SmallBytes<N> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<const N: usize> Eq for SmallBytes<N> {}

impl<const N: usize> PartialOrd for SmallBytes<N> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<const N: usize> Ord for SmallBytes<N> {
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl<const N: usize> Hash for SmallBytes<N> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state)
    }
}

impl<'a, const N: usize> TryFrom<&'a [u8]> for SmallBytes<N> {
    type Error = BadSmallBytesError;

    #[inline]
    fn try_from(value: &'a [u8]) -> Result<Self, Self::Error> {
        if value.len() > Self::CAPACITY {
            return Err(BadSmallBytesError::TooLong(value.len()));
        }
        let mut inner = [0_u8; N];
        inner[..value.len()].copy_from_slice(value);
        // Capacity check above guarantees the cast fits in u8 (N - 1 <= 255).
        inner[N - 1] = u8::try_from(value.len()).map_err(|_| {
            BadSmallBytesError::TooLong(value.len())
        })?;
        Ok(Self { inner })
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum BadSmallBytesError {
    #[error("small bytes payload too large for inline capacity, found {0} bytes")]
    TooLong(usize),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_has_zero_len() {
        let buf = SmallBytes::<32>::new();
        assert!(buf.is_empty());
        assert_eq!(buf.len(), 0);
        assert_eq!(buf.as_slice(), &[] as &[u8]);
    }

    #[test]
    fn round_trips_through_be_bytes() {
        let payload: &[u8] = &[0x01, 0xAA, 0xBB, 0xCC, 0x00, 0xFF];
        let buf: SmallBytes<32> = payload.try_into().expect("fits");
        let bytes = buf.to_be_bytes();
        let restored = SmallBytes::<32>::from_be_bytes(bytes).expect("valid length byte");
        assert_eq!(restored.as_slice(), payload);
    }

    #[test]
    fn rejects_payload_larger_than_capacity() {
        let too_big = vec![0u8; SmallBytes::<32>::CAPACITY + 1];
        let result: Result<SmallBytes<32>, _> = too_big.as_slice().try_into();
        assert!(matches!(result, Err(BadSmallBytesError::TooLong(_))));
    }

    #[test]
    fn equality_ignores_unused_tail() {
        let payload: &[u8] = &[1, 2, 3];
        let a: SmallBytes<32> = payload.try_into().unwrap();
        let mut bytes = a.to_be_bytes();
        // Scribble over a non-payload byte; equality must still hold.
        bytes[10] = 0xEE;
        let b = SmallBytes::<32>::from_be_bytes(bytes).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn from_be_bytes_rejects_corrupt_length() {
        let mut bytes = [0u8; 32];
        bytes[31] = 250; // length byte exceeds CAPACITY = 31
        assert!(SmallBytes::<32>::from_be_bytes(bytes).is_err());
    }

    #[test]
    fn capacity_constant_matches_n_minus_one() {
        assert_eq!(SmallBytes::<32>::CAPACITY, 31);
        assert_eq!(SmallBytes::<8>::CAPACITY, 7);
    }
}
