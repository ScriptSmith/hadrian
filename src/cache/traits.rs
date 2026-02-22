use std::time::Duration;

use async_trait::async_trait;

use super::error::CacheResult;

/// Result of an atomic budget reservation
#[derive(Debug, Clone)]
pub struct BudgetReservation {
    /// Whether the reservation was successful (under budget)
    pub allowed: bool,
    /// Current spend after this operation (in cents)
    pub current_spend: i64,
    /// The limit that was checked against
    pub limit: i64,
}

/// Result of an atomic rate limit check
#[derive(Debug, Clone)]
pub struct RateLimitResult {
    /// Whether the request is allowed
    pub allowed: bool,
    /// Current count after this request
    pub current: i64,
    /// The limit
    pub limit: u32,
    /// Seconds until reset
    pub reset_secs: u64,
}

/// Parameters for a single budget check in a batch operation
#[derive(Debug, Clone)]
pub struct BudgetCheckParams {
    /// Cache key
    pub key: String,
    /// Estimated cost to reserve
    pub estimated_cost: i64,
    /// Budget limit
    pub limit: i64,
    /// TTL for the cache entry
    pub ttl: Duration,
}

/// Parameters for a single rate limit check in a batch operation
#[derive(Debug, Clone)]
pub struct RateLimitCheckParams {
    /// Cache key
    pub key: String,
    /// Rate limit
    pub limit: u32,
    /// Window duration in seconds
    pub window_secs: u64,
}

/// Result of a batch limit check operation
#[derive(Debug, Clone)]
pub struct BatchLimitResult {
    /// Results for budget checks (in same order as input)
    pub budget_results: Vec<BudgetReservation>,
    /// Results for rate limit checks (in same order as input)
    pub rate_limit_results: Vec<RateLimitResult>,
}

#[async_trait]
pub trait Cache: Send + Sync {
    /// Get raw bytes from cache
    async fn get_bytes(&self, key: &str) -> CacheResult<Option<Vec<u8>>>;

    /// Set raw bytes in cache with TTL
    async fn set_bytes(&self, key: &str, value: &[u8], ttl: Duration) -> CacheResult<()>;

    /// Set raw bytes only if key doesn't exist (atomic set-if-not-exists).
    /// Returns true if the value was set, false if key already exists.
    async fn set_nx(&self, key: &str, value: &[u8], ttl: Duration) -> CacheResult<bool>;

    /// Delete a value from cache
    async fn delete(&self, key: &str) -> CacheResult<()>;

    /// Increment a counter, returning the new value
    async fn incr(&self, key: &str, ttl: Duration) -> CacheResult<i64>;

    /// Increment a counter by delta, returning the new value
    async fn incr_by(&self, key: &str, delta: i64, ttl: Duration) -> CacheResult<i64>;

    /// Increment a float counter by delta (for spend tracking in cents)
    async fn incr_by_float(&self, key: &str, delta: i64, ttl: Duration) -> CacheResult<i64>;

    /// Atomically check budget and reserve estimated cost.
    ///
    /// This performs an atomic check-and-increment operation:
    /// 1. If current_spend + estimated_cost <= limit: increment and return allowed=true
    /// 2. Otherwise: don't increment and return allowed=false
    ///
    /// This prevents race conditions where multiple concurrent requests could
    /// all pass the budget check before any of them update the spend.
    async fn check_and_reserve_budget(
        &self,
        key: &str,
        estimated_cost: i64,
        limit: i64,
        ttl: Duration,
    ) -> CacheResult<BudgetReservation>;

    /// Atomically check rate limit and increment counter.
    ///
    /// This performs an atomic check-and-increment:
    /// 1. If current_count < limit: increment and return allowed=true
    /// 2. Otherwise: don't increment and return allowed=false
    async fn check_and_incr_rate_limit(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> CacheResult<RateLimitResult>;

    /// Perform multiple budget and rate limit checks in a single operation.
    ///
    /// For Redis, this uses pipelining to reduce network round trips.
    /// For in-memory cache, this just runs checks sequentially (already fast).
    ///
    /// All checks are performed independently. If any check fails (over limit),
    /// the caller is responsible for rolling back successful reservations if needed.
    ///
    /// Returns results in the same order as the input parameters.
    async fn check_limits_batch(
        &self,
        budget_checks: &[BudgetCheckParams],
        rate_limit_checks: &[RateLimitCheckParams],
    ) -> CacheResult<BatchLimitResult>;

    // ─────────────────────────────────────────────────────────────────────────────
    // SET Operations (for user-sessions index)
    // ─────────────────────────────────────────────────────────────────────────────

    /// Add a member to a set. Returns true if the member was newly added.
    /// If TTL is provided and the key is new or has no expiry, sets the expiration.
    async fn set_add(&self, key: &str, member: &str, ttl: Option<Duration>) -> CacheResult<bool>;

    /// Remove a member from a set. Returns true if the member was removed.
    async fn set_remove(&self, key: &str, member: &str) -> CacheResult<bool>;

    /// Get all members of a set.
    async fn set_members(&self, key: &str) -> CacheResult<Vec<String>>;

    /// Get the number of members in a set.
    async fn set_cardinality(&self, key: &str) -> CacheResult<usize>;

    /// Check if a member exists in a set.
    async fn set_is_member(&self, key: &str, member: &str) -> CacheResult<bool>;

    /// Set or update the TTL (expiration) of a key.
    /// Returns true if the TTL was set, false if the key doesn't exist.
    async fn set_expire(&self, key: &str, ttl: Duration) -> CacheResult<bool>;

    // ─────────────────────────────────────────────────────────────────────────────
    // Downcasting
    // ─────────────────────────────────────────────────────────────────────────────

    /// Get a reference to the underlying RedisCache if this is a Redis-backed cache.
    /// Returns None for memory-backed caches.
    #[cfg(feature = "redis")]
    fn as_redis(&self) -> Option<&super::RedisCache> {
        None
    }
}

// Helper extension trait for working with JSON
pub trait CacheExt: Cache {
    async fn get_json<T: serde::de::DeserializeOwned>(&self, key: &str) -> CacheResult<Option<T>> {
        use super::error::CacheError;
        match self.get_bytes(key).await? {
            Some(bytes) => {
                let value = serde_json::from_slice(&bytes)
                    .map_err(|e| CacheError::Deserialization(e.to_string()))?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    async fn set_json<T: serde::Serialize>(
        &self,
        key: &str,
        value: &T,
        ttl: Duration,
    ) -> CacheResult<()> {
        use super::error::CacheError;
        let bytes =
            serde_json::to_vec(value).map_err(|e| CacheError::Serialization(e.to_string()))?;
        self.set_bytes(key, &bytes, ttl).await
    }
}

// Blanket implementation for all Cache types
impl<T: Cache + ?Sized> CacheExt for T {}
