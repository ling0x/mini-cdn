use bytes::Bytes;
use dashmap::DashMap;
use std::time::{Duration, Instant};

/// A single cached HTTP response.
#[derive(Clone)]
pub struct CachedResponse {
    pub body:         Bytes,
    pub content_type: String,
    pub etag:         Option<String>,
    pub expires_at:   Instant,
}

impl CachedResponse {
    pub fn is_expired(&self) -> bool {
        Instant::now() >= self.expires_at
    }
}

/// Thread-safe, TTL-based in-memory cache backed by DashMap.
pub struct EdgeCache {
    store: DashMap<String, CachedResponse>,
    ttl:   Duration,
    max:   usize,
}

impl EdgeCache {
    pub fn new(max_items: usize, ttl_secs: u64) -> Self {
        Self {
            store: DashMap::new(),
            ttl:   Duration::from_secs(ttl_secs),
            max:   max_items,
        }
    }

    /// Returns a live (non-expired) entry, or `None` on miss / stale.
    pub fn get(&self, key: &str) -> Option<CachedResponse> {
        let entry = self.store.get(key)?;
        if entry.is_expired() {
            drop(entry);
            self.store.remove(key);
            return None;
        }
        Some(entry.clone())
    }

    /// Inserts an entry. Evicts one arbitrary stale (or any) item if at capacity.
    pub fn insert(&self, key: String, resp: CachedResponse) {
        if self.store.len() >= self.max {
            // First pass: try to evict an already-expired entry
            let stale = self
                .store
                .iter()
                .find(|e| e.is_expired())
                .map(|e| e.key().clone());

            let victim = stale.or_else(|| {
                self.store.iter().next().map(|e| e.key().clone())
            });

            if let Some(k) = victim {
                self.store.remove(&k);
            }
        }
        self.store.insert(key, resp);
    }

    /// Explicitly remove a key (e.g. on receiving an invalidation signal).
    pub fn invalidate(&self, key: &str) {
        self.store.remove(key);
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }
}
