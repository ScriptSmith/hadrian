use std::{
    collections::HashSet,
    hint,
    sync::{
        Arc,
        atomic::{AtomicI64, Ordering},
    },
    time::{Duration, Instant},
};

/// Maximum number of CAS retries before returning an error.
/// This prevents infinite spinning under extreme contention.
const MAX_CAS_RETRIES: usize = 100;

use async_trait::async_trait;
use dashmap::DashMap;

use super::{
    error::CacheResult,
    traits::{
        BatchLimitResult, BudgetCheckParams, BudgetReservation, Cache, RateLimitCheckParams,
        RateLimitResult,
    },
};
use crate::config::MemoryCacheConfig;

struct CacheEntry {
    data: Vec<u8>,
    expires_at: Option<Instant>,
    last_accessed: Instant,
}

impl CacheEntry {
    fn new(data: Vec<u8>, expires_at: Option<Instant>) -> Self {
        Self {
            data,
            expires_at,
            last_accessed: Instant::now(),
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| Instant::now() > exp)
    }

    fn touch(&mut self) {
        self.last_accessed = Instant::now();
    }
}

/// Entry for set storage with expiration
struct SetEntry {
    members: HashSet<String>,
    expires_at: Option<Instant>,
}

impl SetEntry {
    fn new(expires_at: Option<Instant>) -> Self {
        Self {
            members: HashSet::new(),
            expires_at,
        }
    }

    fn is_expired(&self) -> bool {
        self.expires_at.is_some_and(|exp| Instant::now() > exp)
    }
}

/// In-memory cache implementation using DashMap for concurrent access.
///
/// # Multi-Node Deployments
///
/// **WARNING**: This cache is NOT suitable for multi-node deployments.
///
/// Each node maintains its own independent cache. This means:
/// - Cache invalidation (e.g., API key revocation) only affects the local node
/// - Revoked API keys may remain valid on other nodes until TTL expires
/// - Rate limiting and budget enforcement are per-node, not global
///
/// For multi-node deployments, use Redis cache which provides:
/// - Shared state across all nodes
/// - Immediate cache invalidation propagation
/// - Accurate rate limiting and budget enforcement
///
/// See [`CacheConfig::Redis`](crate::config::CacheConfig::Redis) in the configuration.
pub struct MemoryCache {
    data: Arc<DashMap<String, CacheEntry>>,
    counters: Arc<DashMap<String, Arc<AtomicI64>>>,
    sets: Arc<DashMap<String, SetEntry>>,
    max_entries: usize,
    eviction_batch_size: usize,
}

impl MemoryCache {
    pub fn new(config: &MemoryCacheConfig) -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            counters: Arc::new(DashMap::new()),
            sets: Arc::new(DashMap::new()),
            max_entries: config.max_entries,
            eviction_batch_size: config.eviction_batch_size.max(1),
        }
    }

    fn evict_if_needed(&self) {
        if self.data.len() < self.max_entries {
            return;
        }

        // First pass: remove all expired entries
        self.data.retain(|_, entry| !entry.is_expired());

        // If still at or above capacity, evict least recently used entries
        let current_len = self.data.len();
        if current_len < self.max_entries {
            return;
        }

        // Calculate how many entries to evict
        let target_size = self.max_entries.saturating_sub(self.eviction_batch_size);
        let to_evict = current_len.saturating_sub(target_size);

        if to_evict == 0 {
            return;
        }

        // Collect entries sorted by last_accessed (oldest first)
        let mut entries: Vec<_> = self
            .data
            .iter()
            .map(|entry| (entry.key().clone(), entry.last_accessed))
            .collect();
        entries.sort_by_key(|(_, last_accessed)| *last_accessed);

        // Remove the oldest entries
        for (key, _) in entries.into_iter().take(to_evict) {
            self.data.remove(&key);
        }
    }
}

#[async_trait]
impl Cache for MemoryCache {
    async fn get_bytes(&self, key: &str) -> CacheResult<Option<Vec<u8>>> {
        if let Some(mut entry) = self.data.get_mut(key) {
            if entry.is_expired() {
                drop(entry);
                self.data.remove(key);
                return Ok(None);
            }

            // Update last accessed time for LRU tracking
            entry.touch();
            Ok(Some(entry.data.clone()))
        } else {
            Ok(None)
        }
    }

    async fn set_bytes(&self, key: &str, value: &[u8], ttl: Duration) -> CacheResult<()> {
        self.evict_if_needed();

        let expires_at = if !ttl.is_zero() {
            Some(Instant::now() + ttl)
        } else {
            None
        };

        self.data
            .insert(key.to_string(), CacheEntry::new(value.to_vec(), expires_at));

        Ok(())
    }

    async fn set_nx(&self, key: &str, value: &[u8], ttl: Duration) -> CacheResult<bool> {
        // Try to insert only if key doesn't exist
        if self.data.contains_key(key)
            && let Some(entry) = self.data.get(key)
            && !entry.is_expired()
        {
            return Ok(false); // Key exists and is not expired
        }

        // Key doesn't exist or is expired, try to insert
        self.evict_if_needed();

        let expires_at = if !ttl.is_zero() {
            Some(Instant::now() + ttl)
        } else {
            None
        };

        // Use entry API for atomic check-and-insert
        use dashmap::mapref::entry::Entry;
        match self.data.entry(key.to_string()) {
            Entry::Occupied(mut e) => {
                // Entry exists - check if expired
                if e.get().is_expired() {
                    e.insert(CacheEntry::new(value.to_vec(), expires_at));
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            Entry::Vacant(e) => {
                e.insert(CacheEntry::new(value.to_vec(), expires_at));
                Ok(true)
            }
        }
    }

    async fn delete(&self, key: &str) -> CacheResult<()> {
        self.data.remove(key);
        self.counters.remove(key);
        self.sets.remove(key);
        Ok(())
    }

    async fn incr(&self, key: &str, ttl: Duration) -> CacheResult<i64> {
        self.incr_by(key, 1, ttl).await
    }

    async fn incr_by(&self, key: &str, delta: i64, _ttl: Duration) -> CacheResult<i64> {
        let counter = self
            .counters
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(AtomicI64::new(0)))
            .clone();

        Ok(counter.fetch_add(delta, Ordering::SeqCst) + delta)
    }

    async fn incr_by_float(&self, key: &str, delta: i64, ttl: Duration) -> CacheResult<i64> {
        // For in-memory, we just use integer cents
        self.incr_by(key, delta, ttl).await
    }

    async fn check_and_reserve_budget(
        &self,
        key: &str,
        estimated_cost: i64,
        limit: i64,
        _ttl: Duration,
    ) -> CacheResult<BudgetReservation> {
        // For in-memory cache, use compare-and-swap loop for atomicity
        let counter = self
            .counters
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(AtomicI64::new(0)))
            .clone();

        for _ in 0..MAX_CAS_RETRIES {
            let current = counter.load(Ordering::SeqCst);
            if current + estimated_cost > limit {
                // Over budget - don't reserve
                return Ok(BudgetReservation {
                    allowed: false,
                    current_spend: current,
                    limit,
                });
            }

            // Try to atomically add the cost
            match counter.compare_exchange(
                current,
                current + estimated_cost,
                Ordering::SeqCst,
                Ordering::SeqCst,
            ) {
                Ok(_) => {
                    return Ok(BudgetReservation {
                        allowed: true,
                        current_spend: current + estimated_cost,
                        limit,
                    });
                }
                Err(_) => {
                    // Another thread modified the value, hint to yield CPU and retry
                    hint::spin_loop();
                }
            }
        }

        Err(super::error::CacheError::Internal(
            "budget reservation failed: CAS retries exhausted under contention".to_string(),
        ))
    }

    async fn check_and_incr_rate_limit(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> CacheResult<RateLimitResult> {
        // For in-memory cache, use compare-and-swap loop for atomicity
        let counter = self
            .counters
            .entry(key.to_string())
            .or_insert_with(|| Arc::new(AtomicI64::new(0)))
            .clone();

        for _ in 0..MAX_CAS_RETRIES {
            let current = counter.load(Ordering::SeqCst);
            if current >= limit as i64 {
                // Over limit - don't increment
                return Ok(RateLimitResult {
                    allowed: false,
                    current,
                    limit,
                    reset_secs: window_secs,
                });
            }

            // Try to atomically increment
            match counter.compare_exchange(current, current + 1, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => {
                    return Ok(RateLimitResult {
                        allowed: true,
                        current: current + 1,
                        limit,
                        reset_secs: window_secs,
                    });
                }
                Err(_) => {
                    // Another thread modified the value, hint to yield CPU and retry
                    hint::spin_loop();
                }
            }
        }

        Err(super::error::CacheError::Internal(
            "rate limit check failed: CAS retries exhausted under contention".to_string(),
        ))
    }

    async fn check_limits_batch(
        &self,
        budget_checks: &[BudgetCheckParams],
        rate_limit_checks: &[RateLimitCheckParams],
    ) -> CacheResult<BatchLimitResult> {
        // For in-memory cache, just call individual methods sequentially
        // No network overhead means pipelining provides no benefit
        let mut budget_results = Vec::with_capacity(budget_checks.len());
        for check in budget_checks {
            let result = self
                .check_and_reserve_budget(&check.key, check.estimated_cost, check.limit, check.ttl)
                .await?;
            budget_results.push(result);
        }

        let mut rate_limit_results = Vec::with_capacity(rate_limit_checks.len());
        for check in rate_limit_checks {
            let result = self
                .check_and_incr_rate_limit(&check.key, check.limit, check.window_secs)
                .await?;
            rate_limit_results.push(result);
        }

        Ok(BatchLimitResult {
            budget_results,
            rate_limit_results,
        })
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // SET Operations
    // ─────────────────────────────────────────────────────────────────────────────

    async fn set_add(&self, key: &str, member: &str, ttl: Option<Duration>) -> CacheResult<bool> {
        use dashmap::mapref::entry::Entry;

        match self.sets.entry(key.to_string()) {
            Entry::Occupied(mut e) => {
                let entry = e.get_mut();
                // Check if expired
                if entry.is_expired() {
                    // Replace with a new entry
                    let expires_at = ttl.map(|t| Instant::now() + t);
                    let mut new_entry = SetEntry::new(expires_at);
                    new_entry.members.insert(member.to_string());
                    *entry = new_entry;
                    Ok(true)
                } else {
                    // Add to existing set
                    Ok(entry.members.insert(member.to_string()))
                }
            }
            Entry::Vacant(e) => {
                // Create new set with the member
                let expires_at = ttl.map(|t| Instant::now() + t);
                let mut entry = SetEntry::new(expires_at);
                entry.members.insert(member.to_string());
                e.insert(entry);
                Ok(true)
            }
        }
    }

    async fn set_remove(&self, key: &str, member: &str) -> CacheResult<bool> {
        if let Some(mut entry) = self.sets.get_mut(key) {
            // Check if expired
            if entry.is_expired() {
                drop(entry);
                self.sets.remove(key);
                return Ok(false);
            }
            Ok(entry.members.remove(member))
        } else {
            Ok(false)
        }
    }

    async fn set_members(&self, key: &str) -> CacheResult<Vec<String>> {
        if let Some(entry) = self.sets.get(key) {
            // Check if expired
            if entry.is_expired() {
                drop(entry);
                self.sets.remove(key);
                return Ok(Vec::new());
            }
            Ok(entry.members.iter().cloned().collect())
        } else {
            Ok(Vec::new())
        }
    }

    async fn set_cardinality(&self, key: &str) -> CacheResult<usize> {
        if let Some(entry) = self.sets.get(key) {
            // Check if expired
            if entry.is_expired() {
                drop(entry);
                self.sets.remove(key);
                return Ok(0);
            }
            Ok(entry.members.len())
        } else {
            Ok(0)
        }
    }

    async fn set_is_member(&self, key: &str, member: &str) -> CacheResult<bool> {
        if let Some(entry) = self.sets.get(key) {
            // Check if expired
            if entry.is_expired() {
                drop(entry);
                self.sets.remove(key);
                return Ok(false);
            }
            Ok(entry.members.contains(member))
        } else {
            Ok(false)
        }
    }

    async fn set_expire(&self, key: &str, ttl: Duration) -> CacheResult<bool> {
        // Try to update TTL on data entries
        if let Some(mut entry) = self.data.get_mut(key) {
            entry.expires_at = Some(Instant::now() + ttl);
            return Ok(true);
        }

        // Try to update TTL on set entries
        if let Some(mut entry) = self.sets.get_mut(key) {
            entry.expires_at = Some(Instant::now() + ttl);
            return Ok(true);
        }

        // Key doesn't exist
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use tokio::time::sleep;

    use super::*;

    fn test_config(max_entries: usize) -> MemoryCacheConfig {
        MemoryCacheConfig {
            max_entries,
            ..Default::default()
        }
    }

    fn test_config_with_eviction(
        max_entries: usize,
        eviction_batch_size: usize,
    ) -> MemoryCacheConfig {
        MemoryCacheConfig {
            max_entries,
            eviction_batch_size,
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn test_get_set_bytes() {
        let cache = MemoryCache::new(&test_config(100));

        // Set and get a value
        cache
            .set_bytes("key1", b"value1", Duration::from_secs(60))
            .await
            .unwrap();
        let result = cache.get_bytes("key1").await.unwrap();
        assert_eq!(result, Some(b"value1".to_vec()));

        // Get non-existent key returns None
        let result = cache.get_bytes("nonexistent").await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_delete() {
        let cache = MemoryCache::new(&test_config(100));

        cache
            .set_bytes("key1", b"value1", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(cache.get_bytes("key1").await.unwrap().is_some());

        cache.delete("key1").await.unwrap();
        assert!(cache.get_bytes("key1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_ttl_expiration() {
        let cache = MemoryCache::new(&test_config(100));

        // Set with short TTL (200ms to avoid flakiness)
        cache
            .set_bytes("expiring", b"value", Duration::from_millis(200))
            .await
            .unwrap();

        // Should exist immediately
        assert!(cache.get_bytes("expiring").await.unwrap().is_some());

        // Wait for expiration
        sleep(Duration::from_millis(300)).await;

        // Should be expired
        assert!(cache.get_bytes("expiring").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_zero_ttl_means_no_expiration() {
        let cache = MemoryCache::new(&test_config(100));

        // Set with zero TTL (no expiration)
        cache
            .set_bytes("forever", b"value", Duration::from_secs(0))
            .await
            .unwrap();

        // Should exist
        assert!(cache.get_bytes("forever").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_incr() {
        let cache = MemoryCache::new(&test_config(100));

        // First increment on non-existent key
        let val = cache
            .incr("counter", Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(val, 1);

        // Second increment
        let val = cache
            .incr("counter", Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(val, 2);

        // Third increment
        let val = cache
            .incr("counter", Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(val, 3);
    }

    #[tokio::test]
    async fn test_incr_by() {
        let cache = MemoryCache::new(&test_config(100));

        let val = cache
            .incr_by("counter", 5, Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(val, 5);

        let val = cache
            .incr_by("counter", 10, Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(val, 15);

        // Negative delta (decrement)
        let val = cache
            .incr_by("counter", -3, Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(val, 12);
    }

    #[tokio::test]
    async fn test_rate_limit_allowed() {
        let cache = MemoryCache::new(&test_config(100));

        // First request should be allowed
        let result = cache
            .check_and_incr_rate_limit("rate:key", 10, 60)
            .await
            .unwrap();
        assert!(result.allowed);
        assert_eq!(result.current, 1);
        assert_eq!(result.limit, 10);
        assert_eq!(result.reset_secs, 60);

        // Should be allowed until limit
        for i in 2..=10 {
            let result = cache
                .check_and_incr_rate_limit("rate:key", 10, 60)
                .await
                .unwrap();
            assert!(result.allowed, "Request {} should be allowed", i);
            assert_eq!(result.current, i);
        }
    }

    #[tokio::test]
    async fn test_rate_limit_exceeded() {
        let cache = MemoryCache::new(&test_config(100));

        // Fill up to the limit
        for _ in 0..10 {
            cache
                .check_and_incr_rate_limit("rate:key", 10, 60)
                .await
                .unwrap();
        }

        // Next request should be denied
        let result = cache
            .check_and_incr_rate_limit("rate:key", 10, 60)
            .await
            .unwrap();
        assert!(!result.allowed);
        assert_eq!(result.current, 10); // Should not increment past limit
    }

    #[tokio::test]
    async fn test_budget_reservation_allowed() {
        let cache = MemoryCache::new(&test_config(100));
        let limit = 10000; // $100.00 in cents

        // First reservation should be allowed
        let result = cache
            .check_and_reserve_budget("budget:key", 500, limit, Duration::from_secs(3600))
            .await
            .unwrap();
        assert!(result.allowed);
        assert_eq!(result.current_spend, 500);
        assert_eq!(result.limit, limit);

        // Second reservation should be allowed
        let result = cache
            .check_and_reserve_budget("budget:key", 500, limit, Duration::from_secs(3600))
            .await
            .unwrap();
        assert!(result.allowed);
        assert_eq!(result.current_spend, 1000);
    }

    #[tokio::test]
    async fn test_budget_reservation_denied() {
        let cache = MemoryCache::new(&test_config(100));
        let limit = 1000; // $10.00 in cents

        // Use up most of the budget
        cache
            .check_and_reserve_budget("budget:key", 900, limit, Duration::from_secs(3600))
            .await
            .unwrap();

        // This would exceed the limit
        let result = cache
            .check_and_reserve_budget("budget:key", 200, limit, Duration::from_secs(3600))
            .await
            .unwrap();
        assert!(!result.allowed);
        assert_eq!(result.current_spend, 900); // Should not have changed
    }

    #[tokio::test]
    async fn test_budget_exact_limit() {
        let cache = MemoryCache::new(&test_config(100));
        let limit = 1000;

        // Exactly at limit should be allowed
        let result = cache
            .check_and_reserve_budget("budget:key", 1000, limit, Duration::from_secs(3600))
            .await
            .unwrap();
        assert!(result.allowed);
        assert_eq!(result.current_spend, 1000);

        // Any more should be denied
        let result = cache
            .check_and_reserve_budget("budget:key", 1, limit, Duration::from_secs(3600))
            .await
            .unwrap();
        assert!(!result.allowed);
    }

    #[tokio::test]
    async fn test_eviction_on_max_entries() {
        let cache = MemoryCache::new(&test_config(3));

        // Fill with expired entries (100ms TTL to avoid flakiness)
        cache
            .set_bytes("old1", b"v", Duration::from_millis(100))
            .await
            .unwrap();
        cache
            .set_bytes("old2", b"v", Duration::from_millis(100))
            .await
            .unwrap();
        cache
            .set_bytes("old3", b"v", Duration::from_millis(100))
            .await
            .unwrap();

        // Wait for expiration
        sleep(Duration::from_millis(200)).await;

        // This should trigger eviction of expired entries
        cache
            .set_bytes("new", b"value", Duration::from_secs(60))
            .await
            .unwrap();

        // New entry should exist
        assert!(cache.get_bytes("new").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_delete_also_removes_counter() {
        let cache = MemoryCache::new(&test_config(100));

        // Create a counter
        cache
            .incr("counter:key", Duration::from_secs(60))
            .await
            .unwrap();
        cache
            .incr("counter:key", Duration::from_secs(60))
            .await
            .unwrap();

        // Delete should remove both data and counter
        cache.delete("counter:key").await.unwrap();

        // Next incr should start from 0
        let val = cache
            .incr("counter:key", Duration::from_secs(60))
            .await
            .unwrap();
        assert_eq!(val, 1);
    }

    #[tokio::test]
    async fn test_multiple_independent_keys() {
        let cache = MemoryCache::new(&test_config(100));

        cache
            .set_bytes("key1", b"value1", Duration::from_secs(60))
            .await
            .unwrap();
        cache
            .set_bytes("key2", b"value2", Duration::from_secs(60))
            .await
            .unwrap();

        assert_eq!(
            cache.get_bytes("key1").await.unwrap(),
            Some(b"value1".to_vec())
        );
        assert_eq!(
            cache.get_bytes("key2").await.unwrap(),
            Some(b"value2".to_vec())
        );

        // Deleting one doesn't affect the other
        cache.delete("key1").await.unwrap();
        assert!(cache.get_bytes("key1").await.unwrap().is_none());
        assert!(cache.get_bytes("key2").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_overwrite_value() {
        let cache = MemoryCache::new(&test_config(100));

        cache
            .set_bytes("key", b"first", Duration::from_secs(60))
            .await
            .unwrap();
        cache
            .set_bytes("key", b"second", Duration::from_secs(60))
            .await
            .unwrap();

        assert_eq!(
            cache.get_bytes("key").await.unwrap(),
            Some(b"second".to_vec())
        );
    }

    #[tokio::test]
    async fn test_concurrent_rate_limit_checks() {
        use std::sync::Arc;

        let cache = Arc::new(MemoryCache::new(&test_config(100)));
        let limit = 100u32;

        // Spawn many concurrent tasks that all try to increment
        let tasks: Vec<_> = (0..200)
            .map(|_| {
                let cache = Arc::clone(&cache);
                tokio::spawn(async move {
                    cache
                        .check_and_incr_rate_limit("concurrent:rate", limit, 60)
                        .await
                })
            })
            .collect();

        let results: Vec<_> = futures::future::join_all(tasks)
            .await
            .into_iter()
            .map(|r| r.unwrap().unwrap())
            .collect();

        // Count how many were allowed
        let allowed_count = results.iter().filter(|r| r.allowed).count();

        // Exactly 100 should be allowed
        assert_eq!(
            allowed_count, 100,
            "Expected exactly 100 allowed, got {}",
            allowed_count
        );
    }

    #[tokio::test]
    async fn test_concurrent_budget_reservations() {
        use std::sync::Arc;

        let cache = Arc::new(MemoryCache::new(&test_config(100)));
        let limit = 1000i64;
        let cost_per_request = 50i64;

        // Spawn concurrent tasks that all try to reserve budget
        let tasks: Vec<_> = (0..50)
            .map(|_| {
                let cache = Arc::clone(&cache);
                tokio::spawn(async move {
                    cache
                        .check_and_reserve_budget(
                            "concurrent:budget",
                            cost_per_request,
                            limit,
                            Duration::from_secs(3600),
                        )
                        .await
                })
            })
            .collect();

        let results: Vec<_> = futures::future::join_all(tasks)
            .await
            .into_iter()
            .map(|r| r.unwrap().unwrap())
            .collect();

        // Count how many were allowed
        let allowed_count = results.iter().filter(|r| r.allowed).count();

        // At 50 cost per request and 1000 limit, exactly 20 should be allowed
        assert_eq!(
            allowed_count, 20,
            "Expected exactly 20 allowed, got {}",
            allowed_count
        );
    }

    #[tokio::test]
    async fn test_set_nx_new_key() {
        let cache = MemoryCache::new(&test_config(100));

        // First set_nx should succeed
        let result = cache
            .set_nx("new_key", b"value", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(result, "First set_nx should succeed");

        // Verify the value was set
        let value = cache.get_bytes("new_key").await.unwrap();
        assert_eq!(value, Some(b"value".to_vec()));
    }

    #[tokio::test]
    async fn test_set_nx_existing_key() {
        let cache = MemoryCache::new(&test_config(100));

        // Set a value first
        cache
            .set_bytes("existing_key", b"original", Duration::from_secs(60))
            .await
            .unwrap();

        // Second set_nx should fail
        let result = cache
            .set_nx("existing_key", b"new_value", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(!result, "set_nx should fail for existing key");

        // Original value should be preserved
        let value = cache.get_bytes("existing_key").await.unwrap();
        assert_eq!(value, Some(b"original".to_vec()));
    }

    #[tokio::test]
    async fn test_set_nx_expired_key() {
        let cache = MemoryCache::new(&test_config(100));

        // Set a value with short TTL
        cache
            .set_bytes("expiring_key", b"original", Duration::from_millis(100))
            .await
            .unwrap();

        // Wait for expiration
        sleep(Duration::from_millis(200)).await;

        // set_nx should succeed for expired key
        let result = cache
            .set_nx("expiring_key", b"new_value", Duration::from_secs(60))
            .await
            .unwrap();
        assert!(result, "set_nx should succeed for expired key");

        // New value should be set
        let value = cache.get_bytes("expiring_key").await.unwrap();
        assert_eq!(value, Some(b"new_value".to_vec()));
    }

    #[tokio::test]
    async fn test_lru_eviction_evicts_oldest() {
        // max_entries=5, eviction_batch_size=2
        let cache = MemoryCache::new(&test_config_with_eviction(5, 2));

        // Fill cache with entries (with delays to ensure distinct access times)
        for i in 0..5 {
            cache
                .set_bytes(&format!("key{}", i), b"value", Duration::from_secs(60))
                .await
                .unwrap();
            // Small delay to ensure different access times
            sleep(Duration::from_millis(10)).await;
        }

        // Access key0 and key1 to make them "recently used"
        cache.get_bytes("key0").await.unwrap();
        sleep(Duration::from_millis(10)).await;
        cache.get_bytes("key1").await.unwrap();

        // Add a new entry, which should trigger LRU eviction
        // The oldest (least recently accessed) entries should be evicted
        // key2, key3, key4 were not accessed after insert, so key2 and key3 should be evicted
        cache
            .set_bytes("new_key", b"new_value", Duration::from_secs(60))
            .await
            .unwrap();

        // key0 and key1 should still exist (recently accessed)
        assert!(
            cache.get_bytes("key0").await.unwrap().is_some(),
            "key0 should exist (recently accessed)"
        );
        assert!(
            cache.get_bytes("key1").await.unwrap().is_some(),
            "key1 should exist (recently accessed)"
        );

        // new_key should exist
        assert!(
            cache.get_bytes("new_key").await.unwrap().is_some(),
            "new_key should exist"
        );

        // At least one of key2, key3, key4 should be evicted
        let remaining = [
            cache.get_bytes("key2").await.unwrap().is_some(),
            cache.get_bytes("key3").await.unwrap().is_some(),
            cache.get_bytes("key4").await.unwrap().is_some(),
        ]
        .iter()
        .filter(|&&x| x)
        .count();

        // After eviction, we should have fewer than 5 entries
        // eviction_batch_size=2 means target is max_entries - 2 = 3 entries after eviction
        assert!(
            remaining <= 2,
            "Expected at most 2 of key2/key3/key4 to remain, got {}",
            remaining
        );
    }

    #[tokio::test]
    async fn test_lru_eviction_prefers_expired_first() {
        let cache = MemoryCache::new(&test_config_with_eviction(4, 2));

        // Add entries: some expired, some not
        cache
            .set_bytes("expired1", b"v", Duration::from_millis(50))
            .await
            .unwrap();
        cache
            .set_bytes("expired2", b"v", Duration::from_millis(50))
            .await
            .unwrap();
        cache
            .set_bytes("valid1", b"v", Duration::from_secs(60))
            .await
            .unwrap();
        cache
            .set_bytes("valid2", b"v", Duration::from_secs(60))
            .await
            .unwrap();

        // Wait for some entries to expire
        sleep(Duration::from_millis(100)).await;

        // Add new entry, triggering eviction
        cache
            .set_bytes("new", b"new", Duration::from_secs(60))
            .await
            .unwrap();

        // Expired entries should be removed first
        assert!(
            cache.get_bytes("expired1").await.unwrap().is_none(),
            "expired1 should be evicted"
        );
        assert!(
            cache.get_bytes("expired2").await.unwrap().is_none(),
            "expired2 should be evicted"
        );

        // Valid entries should remain
        assert!(
            cache.get_bytes("valid1").await.unwrap().is_some(),
            "valid1 should exist"
        );
        assert!(
            cache.get_bytes("valid2").await.unwrap().is_some(),
            "valid2 should exist"
        );
        assert!(
            cache.get_bytes("new").await.unwrap().is_some(),
            "new should exist"
        );
    }

    #[tokio::test]
    async fn test_lru_no_eviction_below_capacity() {
        let cache = MemoryCache::new(&test_config_with_eviction(10, 2));

        // Add entries below capacity
        for i in 0..5 {
            cache
                .set_bytes(&format!("key{}", i), b"value", Duration::from_secs(60))
                .await
                .unwrap();
        }

        // All entries should exist
        for i in 0..5 {
            assert!(
                cache
                    .get_bytes(&format!("key{}", i))
                    .await
                    .unwrap()
                    .is_some(),
                "key{} should exist",
                i
            );
        }
    }

    #[tokio::test]
    async fn test_get_updates_last_accessed() {
        let cache = MemoryCache::new(&test_config_with_eviction(3, 1));

        // Add entries
        cache
            .set_bytes("key1", b"v1", Duration::from_secs(60))
            .await
            .unwrap();
        sleep(Duration::from_millis(20)).await;
        cache
            .set_bytes("key2", b"v2", Duration::from_secs(60))
            .await
            .unwrap();
        sleep(Duration::from_millis(20)).await;
        cache
            .set_bytes("key3", b"v3", Duration::from_secs(60))
            .await
            .unwrap();

        // Access key1 to make it recently used
        sleep(Duration::from_millis(20)).await;
        cache.get_bytes("key1").await.unwrap();

        // Add new entry, triggering eviction
        // key2 should be evicted (oldest last_accessed)
        cache
            .set_bytes("key4", b"v4", Duration::from_secs(60))
            .await
            .unwrap();

        // key1 should still exist (was accessed recently)
        assert!(
            cache.get_bytes("key1").await.unwrap().is_some(),
            "key1 should exist (was accessed recently)"
        );

        // key2 should be evicted (oldest)
        assert!(
            cache.get_bytes("key2").await.unwrap().is_none(),
            "key2 should be evicted (oldest)"
        );
    }
}
