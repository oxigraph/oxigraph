use std::num::NonZeroUsize;
use lru::LruCache;
use crate::JellySyntaxError;

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