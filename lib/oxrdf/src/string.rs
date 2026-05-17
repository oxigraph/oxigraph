// TODO: add safety guards with max refcount and max len

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Error, Serialize, Serializer, de};
use std::alloc::{Layout, alloc, dealloc};
use std::borrow::Borrow;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::atomic::{AtomicUsize, Ordering, fence};
use std::{fmt, ptr, slice};

/// Owned type alias of [`OxStr`]
pub type OxString = OxStr<'static>;

pub struct OxStr<'a> {
    length: usize,
    data: OxStringPointer,
    _marker: PhantomData<&'a ()>,
}

#[repr(C)]
#[derive(Clone, Copy)]
union OxStringPointer {
    owned: *mut OxStringOwnedValue,
    borrowed: *const u8, // TODO: NonNull?
}

#[repr(C)]
struct OxStringOwnedValue {
    counter: AtomicUsize,
    value: str,
}

enum OxStringState<'a> {
    Owned(&'a OxStringOwnedValue),
    Borrowed(&'a str),
}

const OWNED_FLAG: usize = 1 << (usize::BITS - 1);

impl<'a> OxStr<'a> {
    #[inline]
    pub const fn new(value: &'a str) -> Self {
        Self {
            length: value.len(),
            data: OxStringPointer {
                borrowed: value.as_ptr(),
            },
            _marker: PhantomData,
        }
    }

    #[inline]
    #[expect(clippy::cast_ptr_alignment)]
    pub fn new_owned(value: &str) -> Self {
        // TODO: fallible API?
        assert_eq!(value.len() & OWNED_FLAG, 0, "Too large value");

        let (layout, value_offset) = Layout::new::<AtomicUsize>()
            .extend(Layout::array::<u8>(value.len()).unwrap())
            .unwrap();
        let layout = layout.pad_to_align();
        unsafe {
            let ptr = alloc(layout);
            assert!(!ptr.is_null());
            ptr.cast::<AtomicUsize>().write(AtomicUsize::new(1));
            ptr.add(value_offset)
                .copy_from_nonoverlapping(value.as_ptr(), value.len());
            Self {
                length: value.len() | OWNED_FLAG,
                data: OxStringPointer {
                    owned: ptr::slice_from_raw_parts_mut(ptr, value.len())
                        as *mut OxStringOwnedValue,
                },
                _marker: PhantomData,
            }
        }
    }

    #[inline]
    pub fn to_owned(&self) -> OxStr<'static> {
        match self.state() {
            OxStringState::Owned(state) => {
                state.counter.fetch_add(1, Ordering::Relaxed); // Arc is also using relaxed, I guess it's fine
                OxStr {
                    length: self.length,
                    data: self.data,
                    _marker: PhantomData,
                }
            }
            OxStringState::Borrowed(value) => OxStr::new_owned(value),
        }
    }

    #[inline]
    pub const fn as_str(&self) -> &'a str {
        match self.state() {
            OxStringState::Owned(state) => &state.value,
            OxStringState::Borrowed(value) => value,
        }
    }

    #[inline]
    const fn state(&self) -> OxStringState<'a> {
        unsafe {
            if (self.length & OWNED_FLAG) == 0 {
                // borrowed
                OxStringState::Borrowed(str::from_utf8_unchecked(slice::from_raw_parts(
                    self.data.borrowed,
                    self.length,
                )))
            } else {
                OxStringState::Owned(&*self.data.owned)
            }
        }
    }
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut str> {
        unsafe {
            if self.is_owned_and_unique() {
                // we have a unique reference, and the &mut ensure that there won't be more
                Some(&mut (*self.data.owned).value)
            } else {
                None
            }
        }
    }

    #[inline]
    pub fn make_mut(&mut self) -> &mut str {
        if !self.is_owned_and_unique() {
            let value = OxString::new_owned(self.as_str());
            *self = value;
        }
        unsafe { &mut (*self.data.owned).value }
    }

    #[inline]
    fn is_owned_and_unique(&self) -> bool {
        unsafe {
            (self.length & OWNED_FLAG) != 0
                && (*self.data.owned).counter.load(Ordering::Acquire) == 1
        }
    }
}

unsafe impl Send for OxStr<'_> {}
unsafe impl Sync for OxStr<'_> {}

impl Drop for OxStr<'_> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            if (self.length & OWNED_FLAG) != 0 {
                let state = self.data.owned;
                // Load and fence from Arc implementation
                if (*state).counter.fetch_sub(1, Ordering::Release) != 1 {
                    return;
                }
                fence(Ordering::Acquire);
                dealloc(state.cast(), Layout::for_value(&*state));
            }
        }
    }
}

impl Clone for OxStr<'_> {
    #[inline]
    fn clone(&self) -> Self {
        if let OxStringState::Owned(state) = self.state() {
            state.counter.fetch_add(1, Ordering::Relaxed); // Arc is also using relaxed, I guess it's fine
        }
        Self {
            length: self.length,
            data: self.data,
            _marker: PhantomData,
        }
    }
}

impl AsRef<str> for OxStr<'_> {
    #[inline]
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl Deref for OxStr<'_> {
    type Target = str;

    #[inline]
    fn deref(&self) -> &str {
        self.as_str()
    }
}

impl Borrow<str> for OxStr<'_> {
    #[inline]
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq for OxStr<'_> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        self.as_str().eq(other.as_str())
    }
}

impl Eq for OxStr<'_> {}

impl PartialOrd for OxStr<'_> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OxStr<'_> {
    #[inline]
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl Hash for OxStr<'_> {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state)
    }
}

impl fmt::Debug for OxStr<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl fmt::Display for OxStr<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.as_str().fmt(f)
    }
}

impl<'a> From<&'a str> for OxStr<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self::new_owned(value)
    }
}

// TODO: remove
impl From<String> for OxStr<'_> {
    #[inline]
    fn from(value: String) -> Self {
        Self::new_owned(&value)
    }
}

#[cfg(feature = "serde")]
impl Serialize for OxStr<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.as_str().serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de> Deserialize<'de> for OxStr<'_> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct StrVisitor;

        impl de::Visitor<'_> for StrVisitor {
            type Value = OxStr<'static>;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a string")
            }

            fn visit_str<E: Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(OxStr::new_owned(v))
            }

            fn visit_bytes<E: Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                let str = str::from_utf8(v)
                    .map_err(|_| Error::invalid_value(Unexpected::Bytes(v), &self))?;
                self.visit_str(str)
            }
        }

        deserializer.deserialize_str(StrVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_pointer_width = "32")]
    use std::hint::black_box;

    #[test]
    fn owned_clone() {
        let str = OxStr::new_owned("a");
        let copy = str.clone();
        drop(str);
        assert_eq!(copy.as_str(), "a");
    }

    #[test]
    fn owned_ref_clone() {
        let str = OxStr::new_owned("a");
        let copy = str.to_owned();
        drop(str);
        assert_eq!(copy.as_str(), "a");
    }

    #[test]
    fn as_mut() {
        assert_eq!(OxStr::new("a").get_mut(), None);
        let mut v = OxStr::new_owned("a");
        assert_eq!(v.get_mut().as_deref(), Some("a"));
        assert_eq!(v.clone().get_mut(), None);
    }

    #[test]
    fn make_mut() {
        let slice = OxStr::new("a");
        let mut slice_clone = slice.clone();
        slice_clone.make_mut().make_ascii_uppercase();
        assert_eq!(slice.as_str(), "a");
        assert_eq!(slice_clone.as_str(), "A");

        let mut owned = OxStr::new_owned("a");
        owned.make_mut().make_ascii_uppercase();
        assert_eq!(owned.as_str(), "A");

        let mut owned_clone = owned.clone();
        owned_clone.make_mut().make_ascii_lowercase();
        assert_eq!(owned.as_str(), "A");
        assert_eq!(owned_clone.as_str(), "a");
    }
}
