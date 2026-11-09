//! Collection types that work both with `std` and, through `hashbrown`, without it.

#[cfg(not(feature = "std"))]
pub(crate) use alloc::collections::{
    BTreeMap, BTreeSet,
    btree_set::{Iter as BTreeSetIter, Range as BTreeSetRange},
};
#[cfg(not(feature = "std"))]
pub(crate) use hashbrown::{DefaultHashBuilder, HashMap, HashSet, hash_map::Entry};

#[cfg(feature = "std")]
pub(crate) use std::collections::{
    BTreeMap, BTreeSet, HashMap, HashSet,
    btree_set::{Iter as BTreeSetIter, Range as BTreeSetRange},
    hash_map::{Entry, RandomState as DefaultHashBuilder},
};

/// Hashes a string into a `u64`, using the platform default hasher.
#[inline]
pub(crate) fn hash_str(value: &str) -> u64 {
    #[cfg(feature = "std")]
    {
        use core::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        value.hash(&mut hasher);
        hasher.finish()
    }
    #[cfg(not(feature = "std"))]
    {
        use core::hash::BuildHasher;
        DefaultHashBuilder::default().hash_one(value)
    }
}
