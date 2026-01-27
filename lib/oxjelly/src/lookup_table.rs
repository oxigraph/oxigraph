use std::num::NonZeroUsize;
use lru::LruCache;
use crate::JellySyntaxError;

pub(crate) enum LookupResult {
    MRUHit,
    CacheHit(u32),
    CacheMiss(u32),
}

impl From<LookupResult> for u32 {
    fn from(value: LookupResult) -> Self {
        match value {
            LookupResult::MRUHit => 0u32,
            LookupResult::CacheHit(id) => id,
            LookupResult::CacheMiss(id) => id,
        }
    }
}

pub(crate) struct InverseLookupTable {
    cache: LruCache<String, u32>,
    next_id: u32,
}

impl Default for InverseLookupTable {
    fn default() -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::new(8).unwrap()),
            next_id: 1,
        }
    }
}

impl InverseLookupTable {
    pub(crate) fn get_or_push(&mut self, key: String) -> LookupResult {
        if let Some((mru_key, _)) = self.cache.peek_mru() {
            if mru_key == &key { return LookupResult::MRUHit; }
        }

        if let Some(value) = self.cache.get(&key) {
            LookupResult::CacheHit(*value)
        } else {
            let new_id = self.next_id;
            if let Some((_, evicted_value)) = self.cache.push(key, new_id) {
                self.next_id = evicted_value;
            } else {
                self.next_id += 1;
            }
            LookupResult::CacheMiss(new_id)
        }
    }

    pub(crate) fn resize(&mut self, capacity: u32) {
        self.cache.resize(NonZeroUsize::new(capacity as usize).unwrap_or(NonZeroUsize::MIN))
    }

    pub(crate) fn capacity(&self) -> u32 {
        self.cache.cap().get() as u32
    }
}

pub(crate) struct LookupTable {
    cache: LruCache<u32, String>,
}

impl LookupTable {
    pub(crate) fn new() -> Self {
        Self {
            cache: LruCache::new(NonZeroUsize::MIN),
        }
    }

    pub(crate) fn lookup(&mut self, id: &u32) -> Option<&String> {
        if *id == 0 {
            self.cache.peek_mru().map(|(_, v)| v)
        } else {
            self.cache.get(id)
        }
    }

    pub(crate) fn push(&mut self, id: u32, value: String) -> Result<(), JellySyntaxError> {
        if id > self.capacity() {
            return Err(
                JellySyntaxError::IdOutOfBounds(id, self.capacity()).into()
            );
        }

        let id = if id == 0 {
            self.cache.peek_mru().map(|(k, _)| k + 1).unwrap_or(1)
        } else {
            id
        };

        self.cache.push(id, value);
        Ok(())
    }

    pub(crate) fn resize(&mut self, capacity: u32) {
        self.cache.resize(NonZeroUsize::new(capacity as usize).unwrap_or(NonZeroUsize::MIN))
    }

    pub(crate) fn capacity(&self) -> u32 {
        self.cache.cap().get() as u32
    }
}