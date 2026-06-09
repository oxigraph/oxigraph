#![expect(unsafe_code)]

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
use std::alloc::{Layout, alloc, dealloc};
use std::borrow::{Borrow, Cow};
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::mem::transmute;
use std::ops::Deref;
use std::process::abort;
use std::ptr::NonNull;
use std::sync::atomic::{AtomicUsize, Ordering, fence};

const MAX_REF_COUNTER: usize = isize::MAX as usize;
const KIND_SHIFT: u32 = usize::BITS - 1;
const OWNED_FLAG: usize = (OxStrKind::Owned as usize) << KIND_SHIFT;

/// Owned variant of [`OxStr`]: A compact string type for reference-counted owned data or static slices.
///
/// `OxString` is conceptually a fusion of [`Arc<str>`](std::sync::Arc) and [`Cow<'static, str>`](std::borrow::Cow):
/// it allows storing a static string slice ([`&'static str`](std::str))
/// or a reference-counted fixed-sized string ([`Arc<str>`](std::sync::Arc)), enabling cheap clones of owned data.
///
/// When owned, cloning is cheap and increments an atomic reference count.
///
/// See [`OxStr`] for implementation details.
///
/// ```
/// use oxrdf::OxString;
///
/// let value = OxString::new("hello");
/// assert_eq!(value.as_str(), "hello");
/// ```
pub type OxString = OxStr<'static>;

/// A compact string type that can be either borrowed or reference-counted owned data.
///
/// `OxStr` is conceptually a fusion of [`Arc<str>`](std::sync::Arc) and [`Cow<'a, str>`](std::borrow::Cow):
/// it allows storing a string slice ([`&str`](std::str))
/// or a reference-counted fixed-sized string ([`Arc<str>`](std::sync::Arc)), enabling cheap clones of owned data.
///
/// It is not relying on an enum but uses an optimized layout, storing only a pointer and a `usize` length.
/// It relies on a magic bit in the length to know if the value is borrowed or owned.
/// If borrowed, the pointer directly targets the string bytes.
/// If owned, the pointer points to a memory allocation with first the reference counter, then the string bytes.
///
/// When owned, cloning is cheap and increments an atomic reference count.
///
/// ```
/// use oxrdf::OxStr;
///
/// let borrowed = OxStr::new("hello");
/// let owned = OxStr::new_owned("hello");
///
/// assert_eq!(borrowed.as_str(), "hello");
/// assert_eq!(owned.as_str(), "hello");
/// ```
pub struct OxStr<'a> {
    len: usize,
    data: NonNull<u8>,
    _marker: PhantomData<&'a ()>,
}

impl<'a> OxStr<'a> {
    /// Creates an `OxStr` borrowing from `value`.
    ///
    /// This does not allocate and keeps the lifetime of `value`.
    ///
    /// ```
    /// use oxrdf::OxStr;
    ///
    /// let value = OxStr::new("abc");
    /// assert_eq!(value.as_str(), "abc");
    /// ```
    #[inline]
    pub const fn new(value: &'a str) -> Self {
        Self {
            len: value.len(),
            // SAFETY: &str pointers are not null
            data: unsafe { NonNull::new_unchecked(value.as_ptr().cast_mut().cast()) },
            _marker: PhantomData,
        }
    }

    /// Creates an owned `OxStr` by copying `value`.
    ///
    /// Panics if allocation fails. Use [`try_new_owned`](Self::try_new_owned) for a fallible variant.
    /// ```
    /// use oxrdf::OxStr;
    ///
    /// let value = OxStr::new_owned("abc");
    /// assert_eq!(value.as_str(), "abc");
    /// ```
    #[inline]
    #[expect(clippy::expect_used)]
    pub fn new_owned(value: &str) -> Self {
        Self::try_new_owned(value).expect("failed to allocate the owned string")
    }

    /// Creates an owned `OxStr` by copying `value`.
    ///
    /// Returns `None` if allocation fails or if the final length exceeds the internal
    /// representable size.
    /// ```
    /// use oxrdf::OxStr;
    ///
    /// let value = OxStr::try_new_owned("abc").unwrap();
    /// assert_eq!(value.as_str(), "abc");
    /// ```
    #[inline]
    pub fn try_new_owned(value: &str) -> Option<Self> {
        Self::try_concat([value])
    }

    /// Concatenates all `values` into a new owned `OxStr`.
    ///
    /// Panics if allocation fails. Use [`try_concat`](Self::try_concat) for a fallible variant.
    ///
    /// ```
    /// use oxrdf::OxStr;
    ///
    /// let value = OxStr::concat(["ab", "cd", "ef"]);
    /// assert_eq!(value.as_str(), "abcdef");
    /// ```
    #[inline]
    #[expect(clippy::expect_used)]
    pub fn concat<T: AsRef<str>>(values: impl AsRef<[T]>) -> Self {
        Self::try_concat(values).expect("failed to allocate the owned string")
    }

    /// Concatenates all `values` into a new owned `OxStr`.
    ///
    /// Returns `None` if allocation fails or if the final length exceeds the internal
    /// representable size.
    ///
    /// ```
    /// use oxrdf::OxStr;
    ///
    /// let value = OxStr::try_concat(&["ab", "cd", "ef"]).unwrap();
    /// assert_eq!(value.as_str(), "abcdef");
    /// ```
    #[inline]
    pub fn try_concat<T: AsRef<str>>(values: impl AsRef<[T]>) -> Option<Self> {
        let values = values.as_ref();
        let len = values.iter().map(|s| s.as_ref().len()).sum();
        if len >> KIND_SHIFT != 0 {
            return None; // The length is so long that it prevents using the "owned" flag, we fail to create the string
        }

        // SAFETY: we carefully choose the layout. Then we can allocate, check that allocation works and write to the allocation
        unsafe {
            let data = NonNull::new(alloc(Self::owned_layout_for_len(len)))?;
            data.cast::<AtomicUsize>().write(AtomicUsize::new(1));
            let mut write_ptr = data.cast::<AtomicUsize>().add(1).cast::<u8>();
            for value in values {
                let value = value.as_ref();
                write_ptr
                    .copy_from_nonoverlapping(NonNull::from(value.as_bytes()).cast(), value.len());
                write_ptr = write_ptr.add(value.len());
            }
            Some(Self {
                len: len | OWNED_FLAG,
                data,
                _marker: PhantomData,
            })
        }
    }

    #[inline]
    fn owned_layout_for_len(len: usize) -> Layout {
        Layout::new::<AtomicUsize>()
            .extend(Layout::array::<u8>(len).unwrap())
            .unwrap()
            .0
            .pad_to_align()
    }

    /// Converts to an owned [`OxStr<'static>`](Self).
    ///
    /// If `self` is already owned, this only increments the internal atomic refcount.
    /// If `self` is borrowed, data is copied into a new allocation.
    ///
    /// ```
    /// use oxrdf::OxStr;
    ///
    /// let borrowed = OxStr::new("hello");
    /// let owned = borrowed.to_owned();
    /// assert_eq!(owned.as_str(), "hello");
    /// ```
    #[inline]
    pub fn to_owned(&self) -> OxStr<'static> {
        match self.kind() {
            OxStrKind::Owned => {
                // SAFETY: we just checked it's owned, the pointer points to the reference counter, and we can increment it to do a clone
                unsafe {
                    let count = self.owned_counter().fetch_add(1, Ordering::Relaxed); // Arc is also using relaxed, I guess it's fine

                    // We guard against massive ref count in case of forgetting strings
                    if count > MAX_REF_COUNTER {
                        abort();
                    }

                    OxStr {
                        len: self.len,
                        data: self.data,
                        _marker: PhantomData,
                    }
                }
            }
            OxStrKind::Borrowed => OxStr::new_owned(self.as_str()),
        }
    }

    /// Returns the inner string as a slice.
    #[inline]
    pub const fn as_str(&self) -> &str {
        match self.kind() {
            OxStrKind::Borrowed => {
                // SAFETY: We know we are in the borrowed case
                unsafe { self.borrowed_str() }
            }
            OxStrKind::Owned => {
                // SAFETY: We know we are in the borrowed case
                unsafe { self.owned_str() }
            }
        }
    }

    /// Returns a mutable view of the string contents if this value is owned and uniquely held.
    ///
    /// Returns `None` for borrowed values and for shared owned values (refcount > 1).
    ///
    /// ```
    /// use oxrdf::OxStr;
    ///
    /// let mut borrowed = OxStr::new("abc");
    /// assert_eq!(borrowed.get_mut(), None);
    ///
    /// let mut owned = OxStr::new_owned("abc");
    /// assert_eq!(owned.get_mut().as_deref(), Some("abc"));
    ///
    /// let shared = owned.clone();
    /// assert_eq!(borrowed.get_mut(), None);
    /// drop(shared);
    /// assert_eq!(owned.get_mut().as_deref(), Some("abc"));
    /// ```
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut str> {
        // SAFETY: if this is an owned buffer and there is a single reference to it, and we have exclusive access via &mut,
        // we can mutate the buffer, there is no possible other access to it
        unsafe { self.is_owned_and_unique().then(|| self.owned_str_mut()) }
    }

    /// Ensures unique ownership and returns a mutable view of the string contents.
    ///
    /// If `self` is borrowed or shared, this performs a clone of the string data first
    /// (copy-on-write). If already uniquely owned, no allocation is performed.
    ///
    /// ```
    /// use oxrdf::OxStr;
    ///
    /// let value = OxStr::new("abc");
    /// let mut copy = value.clone();
    /// copy.make_mut().make_ascii_uppercase();
    ///
    /// assert_eq!(value.as_str(), "abc");
    /// assert_eq!(copy.as_str(), "ABC");
    /// ```
    #[inline]
    pub fn make_mut(&mut self) -> &mut str {
        if !self.is_owned_and_unique() {
            let value = OxString::new_owned(self.as_str());
            *self = value;
        }
        // SAFETY: We made sure self is an owned string with a single reference
        unsafe { self.owned_str_mut() }
    }

    #[inline]
    const fn kind(&self) -> OxStrKind {
        // SAFETY: we have repr(usize) on OxStrKind ensure the variant numbers are the same
        unsafe { transmute(self.len >> KIND_SHIFT) }
    }

    #[inline]
    fn is_owned_and_unique(&self) -> bool {
        // SAFETY: the caller ensured the pointer targets the reference counter
        self.kind() == OxStrKind::Owned
            && unsafe { self.owned_counter().load(Ordering::Acquire) == 1 }
    }

    #[inline]
    unsafe fn owned_counter(&self) -> &AtomicUsize {
        // SAFETY: the caller ensured the pointer targets the reference counter
        unsafe { self.data.cast().as_ref() }
    }

    #[inline]
    const unsafe fn owned_str(&self) -> &str {
        // SAFETY: the caller ensured the pointer targets the reference counter + string
        unsafe {
            str::from_utf8_unchecked(
                NonNull::slice_from_raw_parts(
                    self.data.cast::<AtomicUsize>().add(1).cast(),
                    self.owned_len(),
                )
                .as_ref(),
            )
        }
    }

    #[inline]
    const unsafe fn owned_str_mut(&mut self) -> &mut str {
        // SAFETY: the caller ensured the pointer references the reference counter + string and that reference count is 1 (single access)
        unsafe {
            str::from_utf8_unchecked_mut(
                NonNull::slice_from_raw_parts(
                    self.data.cast::<AtomicUsize>().add(1).cast(),
                    self.owned_len(),
                )
                .as_mut(),
            )
        }
    }

    #[inline]
    const fn owned_len(&self) -> usize {
        self.len ^ OWNED_FLAG
    }

    #[inline]
    const unsafe fn borrowed_str(&self) -> &'a str {
        // SAFETY: the caller ensured the pointer references a borrowed string and 'a is the borrowed string lifetime
        unsafe {
            str::from_utf8_unchecked(NonNull::slice_from_raw_parts(self.data, self.len).as_ref())
        }
    }
}

// SAFETY: We are using atomic operations for reference conting and using uniqueness checks for mutable access
unsafe impl Send for OxStr<'_> {}
// SAFETY: We are using atomic operations for reference conting and using uniqueness checks for mutable access
unsafe impl Sync for OxStr<'_> {}

impl Drop for OxStr<'_> {
    #[inline]
    fn drop(&mut self) {
        if self.kind() == OxStrKind::Owned {
            // SAFETY: we just checked it's the owned variant, we can call owned_counter
            // and then after doing proper ordering checks taken from Arc we can allocate
            // using the same layout as alloc
            unsafe {
                // Load and fence from Arc implementation
                if self.owned_counter().fetch_sub(1, Ordering::Release) != 1 {
                    return;
                }
                fence(Ordering::Acquire);
                dealloc(
                    self.data.as_mut(),
                    Self::owned_layout_for_len(self.owned_len()),
                );
            }
        }
    }
}

impl Clone for OxStr<'_> {
    #[inline]
    fn clone(&self) -> Self {
        if self.kind() == OxStrKind::Owned {
            // SAFETY: we just checked it's the owned variant, we can call owned_counter
            let count = unsafe {
                self.owned_counter().fetch_add(1, Ordering::Relaxed) // Arc is also using relaxed, I guess it's fine
            };

            // We guard against massive ref count in case of forgetting strings
            if count > MAX_REF_COUNTER {
                abort();
            }
        }
        Self {
            len: self.len,
            data: self.data,
            _marker: PhantomData,
        }
    }
}

impl Default for OxStr<'_> {
    #[inline]
    fn default() -> Self {
        Self {
            len: 0,
            data: NonNull::dangling(),
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
        self.as_str().eq(other)
    }
}

impl PartialEq<str> for OxStr<'_> {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_str().eq(other)
    }
}

impl PartialEq<&str> for OxStr<'_> {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self.as_str().eq(*other)
    }
}

impl PartialEq<String> for OxStr<'_> {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_str().eq(other)
    }
}

impl PartialEq<Cow<'_, str>> for OxStr<'_> {
    #[inline]
    fn eq(&self, other: &Cow<'_, str>) -> bool {
        self.as_str().eq(other)
    }
}

impl PartialEq<OxStr<'_>> for str {
    #[inline]
    fn eq(&self, other: &OxStr<'_>) -> bool {
        self.eq(other.as_str())
    }
}

impl PartialEq<OxStr<'_>> for &str {
    #[inline]
    fn eq(&self, other: &OxStr<'_>) -> bool {
        (*self).eq(other.as_str())
    }
}

impl PartialEq<OxStr<'_>> for String {
    #[inline]
    fn eq(&self, other: &OxStr<'_>) -> bool {
        self.eq(other.as_str())
    }
}

impl PartialEq<OxStr<'_>> for Cow<'_, str> {
    #[inline]
    fn eq(&self, other: &OxStr<'_>) -> bool {
        self.eq(other.as_str())
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
        fmt::Debug::fmt(self.as_str(), f)
    }
}

impl fmt::Display for OxStr<'_> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_str(), f)
    }
}

impl<'a> From<&'a str> for OxStr<'a> {
    #[inline]
    fn from(value: &'a str) -> Self {
        Self::new(value)
    }
}

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

            fn visit_str<E: de::Error>(self, v: &str) -> Result<Self::Value, E> {
                Ok(OxStr::new_owned(v))
            }

            fn visit_bytes<E: de::Error>(self, v: &[u8]) -> Result<Self::Value, E> {
                let str = str::from_utf8(v)
                    .map_err(|_| de::Error::invalid_value(de::Unexpected::Bytes(v), &self))?;
                self.visit_str(str)
            }
        }

        deserializer.deserialize_str(StrVisitor)
    }
}

#[derive(Eq, PartialEq)]
#[repr(usize)]
enum OxStrKind {
    #[expect(unused)]
    Borrowed = 0,
    Owned = 1,
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

    #[test]
    fn size() {
        assert_eq!(
            size_of::<OxStr<'_>>(),
            size_of::<usize>() + size_of::<*const u8>()
        );
    }

    #[test]
    fn niche() {
        assert_eq!(size_of::<Option<OxStr<'_>>>(), size_of::<OxStr<'_>>());
    }

    #[test]
    fn default() {
        assert_eq!(OxStr::default(), "");
    }
}
