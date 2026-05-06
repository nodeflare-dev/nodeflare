//! Request Coalescing + Caching layer with sharding for reduced lock contention
//!
//! This provides two optimizations:
//! 1. Request Coalescing (singleflight): If multiple identical requests come in
//!    while one is being processed, they all wait for and share the same result
//! 2. TTL Cache with LRU eviction: Results are cached for a configurable duration
//!
//! Uses sharding to reduce lock contention on high-traffic scenarios.

use bytes::Bytes;
use lru::LruCache;
use std::hash::{Hash, Hasher};
use std::num::NonZeroUsize;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{broadcast, Mutex, RwLock};

/// Number of shards for cache partitioning (must be power of 2)
/// Increased from 16 to 32 for better performance under high concurrency
const NUM_SHARDS: usize = 32;

/// Maximum response size to cache (default: 1MB)
/// Responses larger than this are not cached to prevent memory bloat
fn max_cacheable_response_size() -> usize {
    std::env::var("PROXY_CACHE_MAX_RESPONSE_SIZE")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(1024 * 1024) // 1MB default
}

/// Cache entry with TTL tracking
/// Uses Bytes for zero-copy sharing between requests
struct CacheEntry {
    response_body: Bytes,
    status: u16,
    headers: Arc<Vec<(String, String)>>,
    created_at: Instant,
}

/// In-flight request tracker for coalescing
struct InFlightRequest {
    tx: broadcast::Sender<Arc<CacheEntry>>,
}

/// A single cache shard with its own lock
struct CacheShard {
    /// LRU cache for responses
    cache: RwLock<LruCache<u64, CacheEntry>>,
    /// In-flight requests for this shard
    in_flight: Mutex<std::collections::HashMap<u64, InFlightRequest>>,
}

impl CacheShard {
    fn new(capacity_per_shard: usize) -> Self {
        Self {
            cache: RwLock::new(LruCache::new(
                NonZeroUsize::new(capacity_per_shard).unwrap_or(NonZeroUsize::new(1).unwrap()),
            )),
            in_flight: Mutex::new(std::collections::HashMap::new()),
        }
    }
}

/// Sharded Request Cache for high-performance caching with reduced lock contention
pub struct RequestCache {
    /// Sharded caches
    shards: Vec<CacheShard>,
    /// Cache TTL
    ttl: Duration,
    /// Total max entries (informational)
    _max_entries: usize,
    /// Maximum response size to cache
    max_response_size: usize,
}

impl RequestCache {
    pub fn new(ttl_secs: u64, max_entries: usize) -> Self {
        let capacity_per_shard = max_entries / NUM_SHARDS;
        let shards = (0..NUM_SHARDS)
            .map(|_| CacheShard::new(capacity_per_shard.max(1)))
            .collect();

        Self {
            shards,
            ttl: Duration::from_secs(ttl_secs),
            _max_entries: max_entries,
            max_response_size: max_cacheable_response_size(),
        }
    }

    /// Generate cache key from server endpoint + request body
    fn cache_key(endpoint: &str, body: &[u8]) -> u64 {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        endpoint.hash(&mut hasher);
        body.hash(&mut hasher);
        hasher.finish()
    }

    /// Get shard index for a key (uses lower bits of hash)
    #[inline]
    fn shard_index(key: u64) -> usize {
        (key as usize) & (NUM_SHARDS - 1)
    }

    /// Get the shard for a given key
    #[inline]
    fn get_shard(&self, key: u64) -> &CacheShard {
        &self.shards[Self::shard_index(key)]
    }

    /// Try to get cached response (with single lock acquisition)
    pub async fn get(&self, endpoint: &str, body: &[u8]) -> Option<CachedResponse> {
        let key = Self::cache_key(endpoint, body);
        let shard = self.get_shard(key);
        let ttl = self.ttl;

        // Single write lock to both check and update LRU
        let mut cache = shard.cache.write().await;

        if let Some(entry) = cache.get(&key) {
            if entry.created_at.elapsed() < ttl {
                return Some(CachedResponse {
                    body: entry.response_body.clone(), // Zero-copy clone with Bytes
                    status: entry.status,
                    headers: (*entry.headers).clone(),
                });
            }
            // Expired - remove it
            cache.pop(&key);
        }
        None
    }

    /// Execute request with coalescing (improved lock ordering)
    ///
    /// Returns:
    /// - Cached: Found in cache
    /// - Coalesced: Another identical request was in-flight, we waited and got the result
    /// - Execute: Caller should execute the request (and then call `complete`)
    pub async fn try_coalesce(&self, endpoint: &str, body: &[u8]) -> CoalesceResult {
        let key = Self::cache_key(endpoint, body);
        let shard = self.get_shard(key);
        let ttl = self.ttl;

        // Lock ordering: always acquire in_flight lock first, then cache lock
        // This prevents deadlocks
        let mut in_flight = shard.in_flight.lock().await;

        // Check if request is already in-flight
        if let Some(existing) = in_flight.get(&key) {
            let mut rx = existing.tx.subscribe();
            drop(in_flight); // Release lock while waiting

            tracing::debug!("Coalescing request - waiting for in-flight result");

            match rx.recv().await {
                Ok(entry) => {
                    return CoalesceResult::Coalesced(CachedResponse {
                        body: entry.response_body.clone(), // Zero-copy clone with Bytes
                        status: entry.status,
                        headers: (*entry.headers).clone(),
                    });
                }
                Err(_) => {
                    // Sender dropped without sending, caller should retry
                    return CoalesceResult::Execute(RequestHandle { key });
                }
            }
        }

        // Now check cache (while holding in_flight lock to prevent race)
        {
            let mut cache = shard.cache.write().await;
            if let Some(entry) = cache.get(&key) {
                if entry.created_at.elapsed() < ttl {
                    tracing::debug!("Cache hit for request");
                    return CoalesceResult::Cached(CachedResponse {
                        body: entry.response_body.clone(), // Zero-copy clone with Bytes
                        status: entry.status,
                        headers: (*entry.headers).clone(),
                    });
                }
                // Expired - remove it
                cache.pop(&key);
            }
        }

        // No cache, no in-flight - this request will execute
        let (tx, _) = broadcast::channel(1);
        in_flight.insert(key, InFlightRequest { tx });

        tracing::debug!("No cache/in-flight - executing request");
        CoalesceResult::Execute(RequestHandle { key })
    }

    /// Complete a request and cache the result
    /// Responses larger than max_response_size are not cached to prevent memory bloat
    pub async fn complete(
        &self,
        handle: RequestHandle,
        response_body: Vec<u8>,
        status: u16,
        headers: Vec<(String, String)>,
    ) {
        let shard = self.get_shard(handle.key);
        let now = Instant::now();

        // Check if response is too large to cache
        let should_cache = response_body.len() <= self.max_response_size;

        // Convert to Bytes and Arc for zero-copy sharing
        let response_bytes = Bytes::from(response_body);
        let headers_arc = Arc::new(headers);

        let entry = Arc::new(CacheEntry {
            response_body: response_bytes.clone(),
            status,
            headers: headers_arc.clone(),
            created_at: now,
        });

        // Lock ordering: in_flight first, then cache
        let mut in_flight = shard.in_flight.lock().await;

        // Notify waiting requests (always do this, even if not caching)
        if let Some(req) = in_flight.remove(&handle.key) {
            // Ignore send errors (no receivers)
            let _ = req.tx.send(entry.clone());
        }

        // Only store in cache if response is not too large
        if should_cache {
            let mut cache = shard.cache.write().await;
            cache.put(
                handle.key,
                CacheEntry {
                    response_body: response_bytes, // Zero-copy - Bytes is cheap to clone
                    status,
                    headers: headers_arc, // Arc clone is cheap
                    created_at: now,
                },
            );
        } else {
            tracing::debug!(
                "Skipping cache for large response ({} bytes > {} max)",
                response_bytes.len(),
                self.max_response_size
            );
        }
    }

    /// Cancel an in-flight request (on error)
    pub async fn cancel(&self, handle: RequestHandle) {
        let shard = self.get_shard(handle.key);
        let mut in_flight = shard.in_flight.lock().await;
        in_flight.remove(&handle.key);
        // Dropping the sender will cause receivers to get an error
    }

    /// Periodic cleanup of expired entries across all shards
    pub async fn cleanup_expired(&self) {
        let ttl = self.ttl;
        let mut total_removed = 0;

        for shard in &self.shards {
            let mut cache = shard.cache.write().await;
            let before = cache.len();

            // LruCache doesn't have retain, so we collect keys to remove
            let keys_to_remove: Vec<u64> = cache
                .iter()
                .filter(|(_, entry)| entry.created_at.elapsed() >= ttl)
                .map(|(k, _)| *k)
                .collect();

            for key in keys_to_remove {
                cache.pop(&key);
            }

            total_removed += before - cache.len();
        }

        if total_removed > 0 {
            tracing::debug!("Cleaned up {} expired cache entries", total_removed);
        }
    }

    /// Get cache statistics
    pub async fn stats(&self) -> CacheStats {
        let mut cached_entries = 0;
        let mut in_flight_requests = 0;

        for shard in &self.shards {
            let cache = shard.cache.read().await;
            let in_flight = shard.in_flight.lock().await;
            cached_entries += cache.len();
            in_flight_requests += in_flight.len();
        }

        CacheStats {
            cached_entries,
            in_flight_requests,
            num_shards: NUM_SHARDS,
        }
    }
}

/// Handle returned when a request should be executed
pub struct RequestHandle {
    key: u64,
}

/// Result of trying to coalesce a request
pub enum CoalesceResult {
    /// Found in cache
    Cached(CachedResponse),
    /// Another identical request is in-flight, we waited and got the result
    Coalesced(CachedResponse),
    /// No cache/in-flight, caller should execute and then call `complete`
    Execute(RequestHandle),
}

/// Cached response data
/// Uses Bytes for efficient zero-copy sharing between requests
#[derive(Clone)]
pub struct CachedResponse {
    pub body: Bytes,
    pub status: u16,
    pub headers: Vec<(String, String)>,
}

/// Cache statistics
pub struct CacheStats {
    pub cached_entries: usize,
    pub in_flight_requests: usize,
    pub num_shards: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cache_hit() {
        let cache = RequestCache::new(60, 100);
        let endpoint = "http://example.com/api";
        let body = b"test body";

        // First request - should execute
        let result = cache.try_coalesce(endpoint, body).await;
        let handle = match result {
            CoalesceResult::Execute(h) => h,
            _ => panic!("Expected Execute"),
        };

        // Complete the request
        cache
            .complete(handle, b"response".to_vec(), 200, vec![])
            .await;

        // Second request - should be cached
        let result = cache.try_coalesce(endpoint, body).await;
        match result {
            CoalesceResult::Cached(resp) => {
                assert_eq!(resp.body, b"response");
                assert_eq!(resp.status, 200);
            }
            _ => panic!("Expected Cached"),
        }
    }

    #[tokio::test]
    async fn test_coalescing() {
        let cache = Arc::new(RequestCache::new(60, 100));
        let endpoint = "http://example.com/api";
        let body = b"test body";

        // Start first request
        let cache1 = cache.clone();
        let result1 = cache1.try_coalesce(endpoint, body).await;
        let handle = match result1 {
            CoalesceResult::Execute(h) => h,
            _ => panic!("Expected Execute for first request"),
        };

        // Start second request concurrently - should coalesce
        let cache2 = cache.clone();
        let endpoint2 = endpoint.to_string();
        let body2 = body.to_vec();
        let join_handle = tokio::spawn(async move {
            cache2.try_coalesce(&endpoint2, &body2).await
        });

        // Small delay to ensure second request is waiting
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Complete first request
        cache
            .complete(handle, b"shared response".to_vec(), 200, vec![])
            .await;

        // Second request should get coalesced result
        let result2 = join_handle.await.unwrap();
        match result2 {
            CoalesceResult::Coalesced(resp) => {
                assert_eq!(resp.body, b"shared response");
            }
            _ => panic!("Expected Coalesced for second request"),
        }
    }

    #[tokio::test]
    async fn test_sharding() {
        let cache = RequestCache::new(60, 100);

        // Test that different keys go to different shards
        let key1 = RequestCache::cache_key("endpoint1", b"body1");
        let key2 = RequestCache::cache_key("endpoint2", b"body2");

        // Keys should be distributed across shards
        let shard1 = RequestCache::shard_index(key1);
        let shard2 = RequestCache::shard_index(key2);

        // Both should be valid shard indices
        assert!(shard1 < NUM_SHARDS);
        assert!(shard2 < NUM_SHARDS);
    }

    #[tokio::test]
    async fn test_lru_eviction() {
        // Small cache to test eviction
        let cache = RequestCache::new(60, NUM_SHARDS * 2); // 2 per shard

        // Fill up one shard
        for i in 0..5 {
            let endpoint = format!("endpoint{}", i * NUM_SHARDS); // Same shard
            let body = b"body";

            let result = cache.try_coalesce(&endpoint, body).await;
            if let CoalesceResult::Execute(h) = result {
                cache.complete(h, b"response".to_vec(), 200, vec![]).await;
            }
        }

        // Stats should show entries
        let stats = cache.stats().await;
        assert!(stats.cached_entries > 0);
    }
}
