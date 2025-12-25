# Caching Pattern

**Speed up queries with intelligent result caching and smart invalidation**

The Caching Pattern adds a layer between your application and Oxigraph Store to cache query results, dramatically improving performance for read-heavy workloads. This guide covers single-tier, multi-tier, and distributed caching strategies.

## When to Use

**Use Caching when:**
- Same queries execute repeatedly
- Read-heavy workloads (90%+ reads)
- Query results don't change frequently
- Need to reduce database load
- Latency requirements are strict
- Scaling read capacity is needed

**Skip caching when:**
- Write-heavy workloads (frequent updates)
- Data changes constantly (real-time systems)
- Cache invalidation is too complex
- Memory constraints are tight
- Query patterns are unpredictable

## Benefits

✅ **Dramatic Performance** - 10-100x faster for cached queries
✅ **Reduced Load** - Less stress on Oxigraph Store
✅ **Better Scalability** - Handle more concurrent users
✅ **Lower Latency** - Sub-millisecond response times
✅ **Cost Efficiency** - Smaller database instances needed

## Architecture

### Single-Tier Cache

```
Application
    ↓
In-Memory Cache (LRU)
    ↓ (cache miss)
Oxigraph Store
```

### Multi-Tier Cache

```
Application
    ↓
L1 Cache (in-process, fast)
    ↓ (miss)
L2 Cache (Redis, shared)
    ↓ (miss)
Oxigraph Store
```

### Write-Through Pattern

```
Write → Update Store → Invalidate/Update Cache
Read → Check Cache → Return or Query Store
```

---

## Implementation Examples

### Rust Implementation

#### In-Memory LRU Cache

```rust
// src/cache/query_cache.rs
use lru::LruCache;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
struct CacheEntry<T> {
    value: T,
    inserted_at: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            inserted_at: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.inserted_at.elapsed() > ttl
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QueryKey {
    query: String,
    params: Vec<(String, String)>, // Sorted parameters
}

impl QueryKey {
    pub fn new(query: String, params: Vec<(String, String)>) -> Self {
        let mut sorted_params = params;
        sorted_params.sort();
        Self {
            query,
            params: sorted_params,
        }
    }
}

pub struct QueryCache<T> {
    cache: Arc<RwLock<LruCache<QueryKey, CacheEntry<T>>>>,
    ttl: Duration,
    max_size: usize,
    hits: Arc<RwLock<u64>>,
    misses: Arc<RwLock<u64>>,
}

impl<T: Clone> QueryCache<T> {
    pub fn new(max_size: usize, ttl: Duration) -> Self {
        Self {
            cache: Arc::new(RwLock::new(
                LruCache::new(NonZeroUsize::new(max_size).unwrap()),
            )),
            ttl,
            max_size,
            hits: Arc::new(RwLock::new(0)),
            misses: Arc::new(RwLock::new(0)),
        }
    }

    pub fn get(&self, key: &QueryKey) -> Option<T> {
        let mut cache = self.cache.write().unwrap();

        if let Some(entry) = cache.get(key) {
            if entry.is_expired(self.ttl) {
                cache.pop(key);
                *self.misses.write().unwrap() += 1;
                return None;
            }

            *self.hits.write().unwrap() += 1;
            return Some(entry.value.clone());
        }

        *self.misses.write().unwrap() += 1;
        None
    }

    pub fn put(&self, key: QueryKey, value: T) {
        let mut cache = self.cache.write().unwrap();
        cache.put(key, CacheEntry::new(value));
    }

    pub fn invalidate(&self, key: &QueryKey) {
        let mut cache = self.cache.write().unwrap();
        cache.pop(key);
    }

    pub fn invalidate_all(&self) {
        let mut cache = self.cache.write().unwrap();
        cache.clear();
    }

    pub fn invalidate_matching<F>(&self, predicate: F)
    where
        F: Fn(&QueryKey) -> bool,
    {
        let mut cache = self.cache.write().unwrap();
        let keys_to_remove: Vec<_> = cache
            .iter()
            .filter(|(k, _)| predicate(k))
            .map(|(k, _)| k.clone())
            .collect();

        for key in keys_to_remove {
            cache.pop(&key);
        }
    }

    pub fn stats(&self) -> CacheStats {
        let hits = *self.hits.read().unwrap();
        let misses = *self.misses.read().unwrap();
        let size = self.cache.read().unwrap().len();

        CacheStats {
            hits,
            misses,
            size,
            max_size: self.max_size,
            hit_rate: if hits + misses > 0 {
                hits as f64 / (hits + misses) as f64
            } else {
                0.0
            },
        }
    }
}

#[derive(Debug)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub size: usize,
    pub max_size: usize,
    pub hit_rate: f64,
}
```

#### Cached Repository

```rust
// src/repositories/cached_person_repository.rs
use crate::cache::{QueryCache, QueryKey};
use crate::domain::Person;
use crate::repositories::{OxigraphPersonRepository, PersonRepository};
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

pub struct CachedPersonRepository {
    inner: OxigraphPersonRepository,
    cache: Arc<QueryCache<Vec<Person>>>,
}

impl CachedPersonRepository {
    pub fn new(inner: OxigraphPersonRepository) -> Self {
        let cache = Arc::new(QueryCache::new(
            1000,                      // Max 1000 cached queries
            Duration::from_secs(300),  // 5 minute TTL
        ));

        Self { inner, cache }
    }

    pub fn with_cache_config(
        inner: OxigraphPersonRepository,
        max_size: usize,
        ttl: Duration,
    ) -> Self {
        let cache = Arc::new(QueryCache::new(max_size, ttl));
        Self { inner, cache }
    }

    fn invalidate_user_caches(&self, user_id: &str) {
        // Invalidate all caches that might include this user
        self.cache.invalidate_matching(|key| {
            key.query.contains("find_all") || key.query.contains(user_id)
        });
    }

    pub fn cache_stats(&self) -> crate::cache::CacheStats {
        self.cache.stats()
    }
}

impl PersonRepository for CachedPersonRepository {
    fn find_by_id(&self, id: &str) -> Result<Option<Person>, Box<dyn Error>> {
        let cache_key = QueryKey::new(
            "find_by_id".to_string(),
            vec![("id".to_string(), id.to_string())],
        );

        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached.into_iter().next());
        }

        let result = self.inner.find_by_id(id)?;

        if let Some(person) = &result {
            self.cache.put(cache_key, vec![person.clone()]);
        }

        Ok(result)
    }

    fn find_by_email(&self, email: &str) -> Result<Option<Person>, Box<dyn Error>> {
        let cache_key = QueryKey::new(
            "find_by_email".to_string(),
            vec![("email".to_string(), email.to_string())],
        );

        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached.into_iter().next());
        }

        let result = self.inner.find_by_email(email)?;

        if let Some(person) = &result {
            self.cache.put(cache_key, vec![person.clone()]);
        }

        Ok(result)
    }

    fn find_all(&self) -> Result<Vec<Person>, Box<dyn Error>> {
        let cache_key = QueryKey::new("find_all".to_string(), vec![]);

        if let Some(cached) = self.cache.get(&cache_key) {
            return Ok(cached);
        }

        let result = self.inner.find_all()?;
        self.cache.put(cache_key, result.clone());

        Ok(result)
    }

    fn save(&self, person: &Person) -> Result<(), Box<dyn Error>> {
        self.inner.save(person)?;
        self.invalidate_user_caches(&person.id);
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<bool, Box<dyn Error>> {
        let result = self.inner.delete(id)?;
        self.invalidate_user_caches(id);
        Ok(result)
    }

    fn count(&self) -> Result<usize, Box<dyn Error>> {
        // Don't cache count - it changes frequently
        self.inner.count()
    }
}
```

#### Redis-Based Distributed Cache

```rust
// src/cache/redis_cache.rs
use redis::{Client, Commands, Connection};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::time::Duration;

pub struct RedisCache {
    client: Client,
    ttl: Duration,
    prefix: String,
}

impl RedisCache {
    pub fn new(redis_url: &str, ttl: Duration, prefix: String) -> Result<Self, Box<dyn Error>> {
        let client = Client::open(redis_url)?;
        Ok(Self { client, ttl, prefix })
    }

    fn get_connection(&self) -> Result<Connection, Box<dyn Error>> {
        Ok(self.client.get_connection()?)
    }

    fn make_key(&self, key: &str) -> String {
        format!("{}:{}", self.prefix, key)
    }

    pub fn get<T: for<'de> Deserialize<'de>>(&self, key: &str) -> Result<Option<T>, Box<dyn Error>> {
        let mut conn = self.get_connection()?;
        let redis_key = self.make_key(key);

        let value: Option<String> = conn.get(&redis_key)?;

        match value {
            Some(json) => Ok(Some(serde_json::from_str(&json)?)),
            None => Ok(None),
        }
    }

    pub fn set<T: Serialize>(&self, key: &str, value: &T) -> Result<(), Box<dyn Error>> {
        let mut conn = self.get_connection()?;
        let redis_key = self.make_key(key);
        let json = serde_json::to_string(value)?;

        conn.set_ex(&redis_key, json, self.ttl.as_secs() as usize)?;

        Ok(())
    }

    pub fn delete(&self, key: &str) -> Result<(), Box<dyn Error>> {
        let mut conn = self.get_connection()?;
        let redis_key = self.make_key(key);
        conn.del(&redis_key)?;
        Ok(())
    }

    pub fn delete_pattern(&self, pattern: &str) -> Result<(), Box<dyn Error>> {
        let mut conn = self.get_connection()?;
        let search_pattern = self.make_key(pattern);

        let keys: Vec<String> = conn.keys(&search_pattern)?;

        if !keys.is_empty() {
            conn.del(keys)?;
        }

        Ok(())
    }

    pub fn flush_all(&self) -> Result<(), Box<dyn Error>> {
        let mut conn = self.get_connection()?;
        let pattern = self.make_key("*");
        let keys: Vec<String> = conn.keys(&pattern)?;

        if !keys.is_empty() {
            conn.del(keys)?;
        }

        Ok(())
    }
}
```

#### Multi-Tier Cache

```rust
// src/cache/multi_tier_cache.rs
use crate::cache::{QueryCache, RedisCache};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::sync::Arc;

pub struct MultiTierCache<T> {
    l1: Arc<QueryCache<T>>,
    l2: Option<Arc<RedisCache>>,
}

impl<T: Clone + Serialize + for<'de> Deserialize<'de>> MultiTierCache<T> {
    pub fn new(l1: Arc<QueryCache<T>>, l2: Option<Arc<RedisCache>>) -> Self {
        Self { l1, l2 }
    }

    pub fn get(&self, key: &str) -> Result<Option<T>, Box<dyn Error>> {
        // Try L1 cache first
        let query_key = crate::cache::QueryKey::new(key.to_string(), vec![]);
        if let Some(value) = self.l1.get(&query_key) {
            return Ok(Some(value));
        }

        // Try L2 cache (Redis)
        if let Some(l2) = &self.l2 {
            if let Some(value) = l2.get::<T>(key)? {
                // Populate L1 cache
                self.l1.put(query_key, value.clone());
                return Ok(Some(value));
            }
        }

        Ok(None)
    }

    pub fn put(&self, key: &str, value: T) -> Result<(), Box<dyn Error>> {
        let query_key = crate::cache::QueryKey::new(key.to_string(), vec![]);

        // Put in L1
        self.l1.put(query_key, value.clone());

        // Put in L2
        if let Some(l2) = &self.l2 {
            l2.set(key, &value)?;
        }

        Ok(())
    }

    pub fn invalidate(&self, key: &str) -> Result<(), Box<dyn Error>> {
        let query_key = crate::cache::QueryKey::new(key.to_string(), vec![]);

        // Invalidate L1
        self.l1.invalidate(&query_key);

        // Invalidate L2
        if let Some(l2) = &self.l2 {
            l2.delete(key)?;
        }

        Ok(())
    }
}
```

---

### Python Implementation

#### In-Memory LRU Cache

```python
# cache/query_cache.py
from collections import OrderedDict
from dataclasses import dataclass
from datetime import datetime, timedelta
from typing import Any, Dict, Optional, Callable
import hashlib
import json

@dataclass
class CacheEntry:
    value: Any
    inserted_at: datetime

    def is_expired(self, ttl: timedelta) -> bool:
        return datetime.now() - self.inserted_at > ttl


class QueryCache:
    def __init__(self, max_size: int, ttl: timedelta):
        self.cache: OrderedDict[str, CacheEntry] = OrderedDict()
        self.max_size = max_size
        self.ttl = ttl
        self.hits = 0
        self.misses = 0

    def _make_key(self, query: str, params: Dict[str, Any] = None) -> str:
        """Create cache key from query and parameters."""
        params = params or {}
        key_data = {"query": query, "params": sorted(params.items())}
        key_json = json.dumps(key_data, sort_keys=True)
        return hashlib.sha256(key_json.encode()).hexdigest()

    def get(self, query: str, params: Dict[str, Any] = None) -> Optional[Any]:
        """Get value from cache."""
        key = self._make_key(query, params)

        if key in self.cache:
            entry = self.cache[key]

            if entry.is_expired(self.ttl):
                del self.cache[key]
                self.misses += 1
                return None

            # Move to end (most recently used)
            self.cache.move_to_end(key)
            self.hits += 1
            return entry.value

        self.misses += 1
        return None

    def put(self, query: str, value: Any, params: Dict[str, Any] = None) -> None:
        """Put value in cache."""
        key = self._make_key(query, params)

        # Evict oldest if at max size
        if key not in self.cache and len(self.cache) >= self.max_size:
            self.cache.popitem(last=False)

        self.cache[key] = CacheEntry(value=value, inserted_at=datetime.now())

    def invalidate(self, query: str, params: Dict[str, Any] = None) -> None:
        """Invalidate specific cache entry."""
        key = self._make_key(query, params)
        self.cache.pop(key, None)

    def invalidate_all(self) -> None:
        """Clear all cache entries."""
        self.cache.clear()

    def invalidate_matching(self, predicate: Callable[[str], bool]) -> None:
        """Invalidate entries matching predicate."""
        keys_to_remove = [
            key for key in self.cache.keys() if predicate(key)
        ]
        for key in keys_to_remove:
            del self.cache[key]

    def stats(self) -> Dict[str, Any]:
        """Get cache statistics."""
        total = self.hits + self.misses
        hit_rate = self.hits / total if total > 0 else 0.0

        return {
            'hits': self.hits,
            'misses': self.misses,
            'size': len(self.cache),
            'max_size': self.max_size,
            'hit_rate': hit_rate,
        }
```

#### Cached Repository

```python
# repositories/cached_person_repository.py
from datetime import timedelta
from typing import List, Optional
from cache.query_cache import QueryCache
from domain.person import Person
from repositories.person_repository import PersonRepository
from repositories.oxigraph_person_repository import OxigraphPersonRepository

class CachedPersonRepository(PersonRepository):
    def __init__(
        self,
        inner: OxigraphPersonRepository,
        max_cache_size: int = 1000,
        ttl: timedelta = timedelta(minutes=5),
    ):
        self.inner = inner
        self.cache = QueryCache(max_cache_size, ttl)

    def _invalidate_user_caches(self, user_id: str) -> None:
        """Invalidate all caches that might include this user."""
        # Invalidate specific user queries
        self.cache.invalidate('find_by_id', {'id': user_id})

        # Invalidate queries that return multiple users
        self.cache.invalidate('find_all')

    def find_by_id(self, id: str) -> Optional[Person]:
        cached = self.cache.get('find_by_id', {'id': id})
        if cached is not None:
            return cached

        result = self.inner.find_by_id(id)

        if result is not None:
            self.cache.put('find_by_id', result, {'id': id})

        return result

    def find_by_email(self, email: str) -> Optional[Person]:
        cached = self.cache.get('find_by_email', {'email': email})
        if cached is not None:
            return cached

        result = self.inner.find_by_email(email)

        if result is not None:
            self.cache.put('find_by_email', result, {'email': email})

        return result

    def find_all(self) -> List[Person]:
        cached = self.cache.get('find_all')
        if cached is not None:
            return cached

        result = self.inner.find_all()
        self.cache.put('find_all', result)

        return result

    def save(self, person: Person) -> None:
        self.inner.save(person)
        self._invalidate_user_caches(person.id)

    def delete(self, id: str) -> bool:
        result = self.inner.delete(id)
        self._invalidate_user_caches(id)
        return result

    def count(self) -> int:
        # Don't cache count - changes frequently
        return self.inner.count()

    def cache_stats(self) -> dict:
        """Get cache statistics."""
        return self.cache.stats()
```

#### Redis Cache

```python
# cache/redis_cache.py
import json
import redis
from datetime import timedelta
from typing import Any, Optional, Type, TypeVar

T = TypeVar('T')

class RedisCache:
    def __init__(self, redis_url: str, ttl: timedelta, prefix: str = 'oxigraph'):
        self.client = redis.from_url(redis_url)
        self.ttl = int(ttl.total_seconds())
        self.prefix = prefix

    def _make_key(self, key: str) -> str:
        return f"{self.prefix}:{key}"

    def get(self, key: str, type_hint: Type[T] = None) -> Optional[T]:
        """Get value from Redis."""
        redis_key = self._make_key(key)
        value = self.client.get(redis_key)

        if value is None:
            return None

        return json.loads(value)

    def set(self, key: str, value: Any) -> None:
        """Set value in Redis with TTL."""
        redis_key = self._make_key(key)
        json_value = json.dumps(value)
        self.client.setex(redis_key, self.ttl, json_value)

    def delete(self, key: str) -> None:
        """Delete key from Redis."""
        redis_key = self._make_key(key)
        self.client.delete(redis_key)

    def delete_pattern(self, pattern: str) -> None:
        """Delete all keys matching pattern."""
        search_pattern = self._make_key(pattern)
        keys = self.client.keys(search_pattern)

        if keys:
            self.client.delete(*keys)

    def flush_all(self) -> None:
        """Delete all keys with prefix."""
        self.delete_pattern('*')
```

---

### JavaScript Implementation

#### In-Memory LRU Cache

```javascript
// cache/QueryCache.js
import crypto from 'crypto';

class CacheEntry {
    constructor(value) {
        this.value = value;
        this.insertedAt = Date.now();
    }

    isExpired(ttlMs) {
        return Date.now() - this.insertedAt > ttlMs;
    }
}

export class QueryCache {
    constructor(maxSize, ttlMs) {
        this.cache = new Map();
        this.maxSize = maxSize;
        this.ttlMs = ttlMs;
        this.hits = 0;
        this.misses = 0;
    }

    makeKey(query, params = {}) {
        const keyData = JSON.stringify({ query, params }, Object.keys(params).sort());
        return crypto.createHash('sha256').update(keyData).digest('hex');
    }

    get(query, params = {}) {
        const key = this.makeKey(query, params);

        if (this.cache.has(key)) {
            const entry = this.cache.get(key);

            if (entry.isExpired(this.ttlMs)) {
                this.cache.delete(key);
                this.misses++;
                return null;
            }

            // Move to end (LRU)
            this.cache.delete(key);
            this.cache.set(key, entry);

            this.hits++;
            return entry.value;
        }

        this.misses++;
        return null;
    }

    put(query, value, params = {}) {
        const key = this.makeKey(query, params);

        // Evict oldest if at max size
        if (!this.cache.has(key) && this.cache.size >= this.maxSize) {
            const firstKey = this.cache.keys().next().value;
            this.cache.delete(firstKey);
        }

        this.cache.set(key, new CacheEntry(value));
    }

    invalidate(query, params = {}) {
        const key = this.makeKey(query, params);
        this.cache.delete(key);
    }

    invalidateAll() {
        this.cache.clear();
    }

    invalidateMatching(predicate) {
        for (const key of this.cache.keys()) {
            if (predicate(key)) {
                this.cache.delete(key);
            }
        }
    }

    stats() {
        const total = this.hits + this.misses;
        const hitRate = total > 0 ? this.hits / total : 0;

        return {
            hits: this.hits,
            misses: this.misses,
            size: this.cache.size,
            maxSize: this.maxSize,
            hitRate,
        };
    }
}
```

#### Cached Repository

```javascript
// repositories/CachedPersonRepository.js
import { PersonRepository } from './PersonRepository.js';
import { QueryCache } from '../cache/QueryCache.js';

export class CachedPersonRepository extends PersonRepository {
    constructor(inner, maxSize = 1000, ttlMs = 300000) {
        super();
        this.inner = inner;
        this.cache = new QueryCache(maxSize, ttlMs);
    }

    invalidateUserCaches(userId) {
        this.cache.invalidate('findById', { id: userId });
        this.cache.invalidate('findAll');
    }

    async findById(id) {
        const cached = this.cache.get('findById', { id });
        if (cached !== null) {
            return cached;
        }

        const result = await this.inner.findById(id);

        if (result !== null) {
            this.cache.put('findById', result, { id });
        }

        return result;
    }

    async findByEmail(email) {
        const cached = this.cache.get('findByEmail', { email });
        if (cached !== null) {
            return cached;
        }

        const result = await this.inner.findByEmail(email);

        if (result !== null) {
            this.cache.put('findByEmail', result, { email });
        }

        return result;
    }

    async findAll() {
        const cached = this.cache.get('findAll');
        if (cached !== null) {
            return cached;
        }

        const result = await this.inner.findAll();
        this.cache.put('findAll', result);

        return result;
    }

    async save(person) {
        await this.inner.save(person);
        this.invalidateUserCaches(person.id);
    }

    async delete(id) {
        const result = await this.inner.delete(id);
        this.invalidateUserCaches(id);
        return result;
    }

    async count() {
        // Don't cache count
        return await this.inner.count();
    }

    cacheStats() {
        return this.cache.stats();
    }
}
```

---

## Invalidation Strategies

### 1. Time-Based (TTL)

**Pros:** Simple, automatic cleanup
**Cons:** Stale data until expiration

```rust
// Cache with 5 minute TTL
let cache = QueryCache::new(1000, Duration::from_secs(300));
```

### 2. Event-Based

**Pros:** Always fresh data
**Cons:** Requires tracking all mutations

```rust
impl CachedRepository {
    fn save(&self, entity: &Entity) -> Result<()> {
        self.inner.save(entity)?;
        self.cache.invalidate(&entity.id); // Invalidate on write
        Ok(())
    }
}
```

### 3. Pattern-Based

**Pros:** Bulk invalidation
**Cons:** May invalidate more than needed

```rust
// Invalidate all queries containing "user-123"
cache.invalidate_matching(|key| key.query.contains("user-123"));
```

### 4. Tag-Based

**Pros:** Precise control
**Cons:** More complex implementation

```rust
// Tag cache entries with related entities
cache.put_with_tags(key, value, vec!["user:123", "org:456"]);

// Invalidate all entries tagged with "org:456"
cache.invalidate_by_tag("org:456");
```

---

## Cache Warming

Pre-populate cache for predictable queries:

```rust
pub fn warm_cache(&self) -> Result<(), Box<dyn Error>> {
    // Warm frequently accessed queries
    let _ = self.find_all()?;
    let _ = self.count()?;

    // Warm specific important entities
    for id in &["user-1", "user-2", "user-3"] {
        let _ = self.find_by_id(id)?;
    }

    println!("Cache warmed: {:?}", self.cache_stats());
    Ok(())
}
```

---

## Monitoring and Metrics

### Log Cache Performance

```rust
use tracing::info;

let stats = cache.stats();
info!(
    "Cache stats - Hit rate: {:.2}%, Hits: {}, Misses: {}, Size: {}/{}",
    stats.hit_rate * 100.0,
    stats.hits,
    stats.misses,
    stats.size,
    stats.max_size
);
```

### Prometheus Metrics

```rust
use prometheus::{Counter, Gauge, Histogram};

lazy_static! {
    static ref CACHE_HITS: Counter = Counter::new("cache_hits", "Cache hits").unwrap();
    static ref CACHE_MISSES: Counter = Counter::new("cache_misses", "Cache misses").unwrap();
    static ref CACHE_SIZE: Gauge = Gauge::new("cache_size", "Current cache size").unwrap();
    static ref CACHE_LATENCY: Histogram = Histogram::new("cache_latency", "Cache lookup latency").unwrap();
}
```

---

## Best Practices

### ✅ DO:

**Cache Immutable Data** - User profiles, static reference data
**Use TTL** - Prevent unbounded memory growth
**Monitor Hit Rates** - Target 80%+ hit rate
**Warm Critical Paths** - Pre-populate important queries
**Version Cache Keys** - Include schema version in key

### ❌ DON'T:

**Cache Everything** - Only cache hot queries
**Forget Invalidation** - Stale data causes bugs
**Cache Large Objects** - Consider memory usage
**Ignore Metrics** - Monitor hit rates and adjust

---

## Performance Tips

1. **Size Cache Appropriately** - Too small = low hit rate, too large = memory pressure
2. **Tune TTL** - Balance freshness vs hit rate
3. **Use Compression** - For large cached values (especially Redis)
4. **Batch Invalidations** - Group related invalidations
5. **Consider Read-Through** - Automatically populate cache on miss

---

## Next Steps

- Combine with [Repository Pattern](./repository-pattern.md) for clean architecture
- Add [Multi-Tenancy](./multi-tenancy.md) with tenant-aware caching
- Use [Event Sourcing](./event-sourcing.md) for cache invalidation events

---

## Summary

The Caching Pattern provides:
- **Dramatic performance improvements** (10-100x faster)
- **Reduced database load** for read-heavy workloads
- **Better scalability** with multi-tier caching
- **Flexible invalidation** strategies

Start with simple in-memory LRU cache, then add Redis for distributed caching as you scale.
