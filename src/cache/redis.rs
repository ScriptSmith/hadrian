use std::time::Duration;

use async_trait::async_trait;
use redis::{
    ConnectionInfo, IntoConnectionInfo, Value, aio::MultiplexedConnection, cluster::ClusterClient,
    cluster_async::ClusterConnection,
};

use super::{
    error::CacheResult,
    traits::{
        BatchLimitResult, BudgetCheckParams, BudgetReservation, Cache, RateLimitCheckParams,
        RateLimitResult,
    },
};
use crate::config::RedisCacheConfig;

/// A wrapper enum for either a standalone or cluster Redis connection.
/// Both connection types implement the `AsyncCommands` trait, so we can use
/// the same command syntax for both.
enum RedisConn {
    Standalone(MultiplexedConnection),
    Cluster(ClusterConnection),
}

/// Macro to execute a Redis command on either connection type.
/// This avoids code duplication when dispatching commands to standalone vs cluster.
macro_rules! redis_cmd {
    ($conn:expr, $cmd:expr) => {
        match $conn {
            RedisConn::Standalone(ref mut c) => $cmd.query_async(c).await,
            RedisConn::Cluster(ref mut c) => $cmd.query_async(c).await,
        }
    };
}

/// Macro to execute a Redis script on either connection type.
macro_rules! redis_script {
    ($conn:expr, $script:expr) => {
        match $conn {
            RedisConn::Standalone(ref mut c) => $script.invoke_async(c).await,
            RedisConn::Cluster(ref mut c) => $script.invoke_async(c).await,
        }
    };
}

/// Macro to execute a Redis pipeline on either connection type.
macro_rules! redis_pipe {
    ($conn:expr, $pipe:expr) => {
        match $conn {
            RedisConn::Standalone(ref mut c) => $pipe.query_async(c).await,
            RedisConn::Cluster(ref mut c) => $pipe.query_async(c).await,
        }
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// Stream Types
// ─────────────────────────────────────────────────────────────────────────────

/// An entry read from a Redis Stream.
#[derive(Debug, Clone)]
pub struct StreamEntry {
    /// The stream entry ID (e.g., "1234567890-0")
    pub id: String,
    /// Field-value pairs in this entry
    pub fields: Vec<(String, String)>,
}

impl StreamEntry {
    /// Get a field value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

/// Lua script for atomic budget check and reservation.
/// Returns [allowed (0/1), current_spend, limit]
///
/// IMPORTANT: Only sets TTL when the key is new or has no expiry.
/// This prevents extending the rate limit window on every request,
/// which would cause counters to persist across window boundaries.
const BUDGET_CHECK_SCRIPT: &str = r#"
local key = KEYS[1]
local estimated_cost = tonumber(ARGV[1])
local limit = tonumber(ARGV[2])
local ttl = tonumber(ARGV[3])

local current = tonumber(redis.call('GET', key) or '0')

if current + estimated_cost <= limit then
    -- Under budget: reserve the cost
    local new_value = redis.call('INCRBY', key, estimated_cost)
    -- Only set TTL if key is new or has no expiry (TTL returns -1 for no expiry, -2 for missing key)
    -- After INCRBY, key always exists, so -1 means no expiry set yet
    if ttl > 0 and redis.call('TTL', key) < 0 then
        redis.call('EXPIRE', key, ttl)
    end
    return {1, new_value, limit}
else
    -- Over budget: don't reserve
    return {0, current, limit}
end
"#;

/// Lua script for atomic increment that preserves existing TTL.
/// Returns the new value after increment.
///
/// Only sets TTL when the key has no expiry (TTL < 0).
/// This prevents extending rate limit windows on every increment.
const INCR_PRESERVE_TTL_SCRIPT: &str = r#"
local key = KEYS[1]
local delta = tonumber(ARGV[1])
local ttl = tonumber(ARGV[2])

local new_value = redis.call('INCRBY', key, delta)
-- Only set TTL if key has no expiry (TTL returns -1 for no expiry after INCRBY)
if ttl > 0 and redis.call('TTL', key) < 0 then
    redis.call('EXPIRE', key, ttl)
end
return new_value
"#;

/// Lua script for atomic rate limit check and increment.
/// Returns [allowed (0/1), current_count, ttl_remaining]
///
/// NOTE: Only sets TTL when the key has no expiry (TTL < 0).
/// This ensures fixed time windows are maintained and counters
/// expire correctly at the end of each window.
const RATE_LIMIT_SCRIPT: &str = r#"
local key = KEYS[1]
local limit = tonumber(ARGV[1])
local window_secs = tonumber(ARGV[2])

local current = tonumber(redis.call('GET', key) or '0')
local ttl = redis.call('TTL', key)

-- Get current TTL for return value (for reset_secs calculation)
-- TTL returns -2 if key doesn't exist, -1 if no expiry set
if ttl < 0 then
    ttl = window_secs
end

if current < limit then
    -- Under limit: increment
    local new_value = redis.call('INCR', key)
    -- Only set TTL if key has no expiry (preserves fixed time windows)
    if redis.call('TTL', key) < 0 then
        redis.call('EXPIRE', key, window_secs)
    end
    return {1, new_value, ttl}
else
    -- Over limit: don't increment
    return {0, current, ttl}
end
"#;

/// Internal enum to hold either a standalone or cluster Redis client.
enum RedisConnection {
    Standalone(redis::Client),
    Cluster(ClusterClient),
}

pub struct RedisCache {
    connection: RedisConnection,
    key_prefix: String,
}

impl RedisCache {
    pub async fn from_config(config: &RedisCacheConfig) -> CacheResult<Self> {
        let connection = if let Some(cluster_config) = &config.cluster {
            // Cluster mode: parse nodes from URL (comma-separated)
            // e.g., "redis://host1:6379,host2:6379,host3:6379"
            let nodes: Vec<ConnectionInfo> = config
                .url
                .split(',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty())
                .map(|s| {
                    // Ensure each node has redis:// prefix if not present
                    let node_url = if s.starts_with("redis://") || s.starts_with("rediss://") {
                        s.to_string()
                    } else {
                        format!("redis://{}", s)
                    };
                    node_url.into_connection_info()
                })
                .collect::<Result<Vec<_>, _>>()?;

            if nodes.is_empty() {
                return Err(super::error::CacheError::Redis(redis::RedisError::from((
                    redis::ErrorKind::InvalidClientConfig,
                    "No cluster nodes specified in URL",
                ))));
            }

            // Build cluster client with configuration from cluster settings
            let mut builder = redis::cluster::ClusterClientBuilder::new(nodes);

            // Apply cluster-specific settings
            if cluster_config.read_from_replicas {
                builder = builder.read_from_replicas();
            }

            builder = builder.retries(cluster_config.retries);
            builder = builder
                .connection_timeout(Duration::from_secs(cluster_config.connection_timeout_secs));
            builder =
                builder.response_timeout(Duration::from_secs(cluster_config.response_timeout_secs));

            let cluster_client = builder.build()?;
            RedisConnection::Cluster(cluster_client)
        } else {
            // Standalone mode: single Redis instance
            let client = redis::Client::open(config.url.as_str())?;
            RedisConnection::Standalone(client)
        };

        Ok(Self {
            connection,
            key_prefix: config.key_prefix.clone(),
        })
    }

    fn prefixed_key(&self, key: &str) -> String {
        format!("{}{}", self.key_prefix, key)
    }

    /// Get a Redis connection, either standalone or cluster.
    async fn get_connection(&self) -> CacheResult<RedisConn> {
        match &self.connection {
            RedisConnection::Standalone(client) => {
                let conn = client.get_multiplexed_async_connection().await?;
                Ok(RedisConn::Standalone(conn))
            }
            RedisConnection::Cluster(client) => {
                let conn = client.get_async_connection().await?;
                Ok(RedisConn::Cluster(conn))
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Redis Streams Operations
    // ─────────────────────────────────────────────────────────────────────────────

    /// Add an entry to a Redis Stream.
    ///
    /// Uses XADD with optional MAXLEN ~ for approximate trimming.
    /// Returns the auto-generated entry ID.
    pub async fn stream_add(
        &self,
        key: &str,
        fields: &[(&str, &str)],
        max_len: Option<u64>,
    ) -> CacheResult<String> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let mut cmd = redis::cmd("XADD");
        cmd.arg(&full_key);

        // Optional approximate trimming
        if let Some(max) = max_len {
            cmd.arg("MAXLEN").arg("~").arg(max);
        }

        // Auto-generate ID
        cmd.arg("*");

        // Add field-value pairs
        for (field, value) in fields {
            cmd.arg(*field).arg(*value);
        }

        let id: String = redis_cmd!(conn, cmd)?;
        Ok(id)
    }

    /// Create a consumer group for a stream.
    ///
    /// Uses XGROUP CREATE with MKSTREAM to create the stream if it doesn't exist.
    /// Returns true if the group was created, false if it already exists.
    pub async fn stream_create_group(
        &self,
        key: &str,
        group: &str,
        start_id: &str,
    ) -> CacheResult<bool> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let result: Result<(), redis::RedisError> = redis_cmd!(
            conn,
            redis::cmd("XGROUP")
                .arg("CREATE")
                .arg(&full_key)
                .arg(group)
                .arg(start_id)
                .arg("MKSTREAM")
        );

        match result {
            Ok(()) => Ok(true),
            Err(e) => {
                // BUSYGROUP means the group already exists - that's fine
                if e.to_string().contains("BUSYGROUP") {
                    Ok(false)
                } else {
                    Err(e.into())
                }
            }
        }
    }

    /// Read entries from a stream using a consumer group.
    ///
    /// Uses XREADGROUP to read undelivered entries (">") or pending entries.
    /// Returns entries that should be acknowledged after processing.
    pub async fn stream_read_group(
        &self,
        key: &str,
        group: &str,
        consumer: &str,
        count: usize,
        block_ms: Option<u64>,
    ) -> CacheResult<Vec<StreamEntry>> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let mut cmd = redis::cmd("XREADGROUP");
        cmd.arg("GROUP").arg(group).arg(consumer);

        if let Some(ms) = block_ms {
            cmd.arg("BLOCK").arg(ms);
        }

        cmd.arg("COUNT").arg(count);
        cmd.arg("STREAMS").arg(&full_key).arg(">");

        let value: Value = redis_cmd!(conn, cmd)?;

        // Parse XREADGROUP response: [[stream_name, [[id, [field, value, ...]]]]] or Nil
        Ok(Self::parse_xreadgroup_response(value))
    }

    /// Acknowledge entries in a consumer group.
    ///
    /// Uses XACK to mark entries as processed. Returns the number of entries acknowledged.
    pub async fn stream_ack(&self, key: &str, group: &str, ids: &[&str]) -> CacheResult<u64> {
        if ids.is_empty() {
            return Ok(0);
        }

        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let mut cmd = redis::cmd("XACK");
        cmd.arg(&full_key).arg(group);
        for id in ids {
            cmd.arg(*id);
        }

        let count: u64 = redis_cmd!(conn, cmd)?;
        Ok(count)
    }

    /// Get the count of pending entries for a consumer group.
    ///
    /// Uses XPENDING to get summary info. Returns the total pending count.
    pub async fn stream_pending_count(&self, key: &str, group: &str) -> CacheResult<u64> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let value: Value = redis_cmd!(conn, redis::cmd("XPENDING").arg(&full_key).arg(group))?;

        // XPENDING returns [count, min-id, max-id, [[consumer, count], ...]]
        // First element is the total pending count
        if let Value::Array(arr) = value
            && !arr.is_empty()
        {
            match &arr[0] {
                Value::Int(n) => return Ok(*n as u64),
                Value::BulkString(bytes) => {
                    if let Ok(s) = std::str::from_utf8(bytes)
                        && let Ok(n) = s.parse::<u64>()
                    {
                        return Ok(n);
                    }
                }
                _ => {}
            }
        }

        Ok(0)
    }

    /// Get the length of a stream.
    pub async fn stream_len(&self, key: &str) -> CacheResult<u64> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let len: u64 = redis_cmd!(conn, redis::cmd("XLEN").arg(&full_key))?;

        Ok(len)
    }

    /// Parse XREADGROUP response into stream entries.
    fn parse_xreadgroup_response(value: Value) -> Vec<StreamEntry> {
        let mut entries = Vec::new();

        // Response is Nil if no entries, or [[stream_name, [[id, [field, value, ...]]]]]
        if let Value::Array(streams) = value {
            for stream in streams {
                if let Value::Array(stream_data) = stream
                    && stream_data.len() >= 2
                    && let Value::Array(stream_entries) = &stream_data[1]
                {
                    for entry_value in stream_entries {
                        if let Value::Array(entry) = entry_value
                            && entry.len() >= 2
                        {
                            // First element is the ID
                            let id = match &entry[0] {
                                Value::BulkString(bytes) => {
                                    String::from_utf8_lossy(bytes).to_string()
                                }
                                _ => continue,
                            };

                            // Second element is the fields array
                            let mut fields = Vec::new();
                            if let Value::Array(field_values) = &entry[1] {
                                let mut iter = field_values.iter();
                                while let (Some(key), Some(val)) = (iter.next(), iter.next()) {
                                    if let (Value::BulkString(k), Value::BulkString(v)) = (key, val)
                                    {
                                        fields.push((
                                            String::from_utf8_lossy(k).to_string(),
                                            String::from_utf8_lossy(v).to_string(),
                                        ));
                                    }
                                }
                            }

                            entries.push(StreamEntry { id, fields });
                        }
                    }
                }
            }
        }

        entries
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // Redis Sorted Set Operations
    // ─────────────────────────────────────────────────────────────────────────────

    /// Add a member to a sorted set with a score.
    ///
    /// Returns true if the member was newly added, false if it was updated.
    /// If TTL is provided and the key is new or has no expiry, sets the expiration.
    pub async fn zset_add(
        &self,
        key: &str,
        score: f64,
        member: &str,
        ttl: Option<Duration>,
    ) -> CacheResult<bool> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        // ZADD returns the number of elements added (0 if updated, 1 if new)
        let added: i64 = redis_cmd!(
            conn,
            redis::cmd("ZADD").arg(&full_key).arg(score).arg(member)
        )?;

        // Set TTL if provided and key has no existing expiry
        if let Some(ttl) = ttl
            && ttl.as_secs() > 0
        {
            let current_ttl: i64 = redis_cmd!(conn, redis::cmd("TTL").arg(&full_key))?;
            if current_ttl < 0 {
                let _: () =
                    redis_cmd!(conn, redis::cmd("EXPIRE").arg(&full_key).arg(ttl.as_secs()))?;
            }
        }

        Ok(added > 0)
    }

    /// Get members from a sorted set within a score range.
    ///
    /// Returns members with their scores, ordered by score ascending.
    pub async fn zset_range_by_score(
        &self,
        key: &str,
        min: f64,
        max: f64,
        limit: Option<usize>,
    ) -> CacheResult<Vec<(String, f64)>> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let mut cmd = redis::cmd("ZRANGEBYSCORE");
        cmd.arg(&full_key).arg(min).arg(max).arg("WITHSCORES");

        if let Some(limit) = limit {
            cmd.arg("LIMIT").arg(0).arg(limit);
        }

        let value: Value = redis_cmd!(conn, cmd)?;

        // Parse response: [member1, score1, member2, score2, ...]
        let mut results = Vec::new();
        if let Value::Array(arr) = value {
            let mut iter = arr.into_iter();
            while let (Some(member_val), Some(score_val)) = (iter.next(), iter.next()) {
                let member = match member_val {
                    Value::BulkString(bytes) => String::from_utf8_lossy(&bytes).to_string(),
                    _ => continue,
                };
                let score = match score_val {
                    Value::BulkString(bytes) => {
                        String::from_utf8_lossy(&bytes).parse().unwrap_or(0.0)
                    }
                    Value::Double(f) => f,
                    _ => continue,
                };
                results.push((member, score));
            }
        }

        Ok(results)
    }

    /// Remove members from a sorted set within a score range.
    ///
    /// Returns the number of members removed.
    pub async fn zset_remove_by_score(&self, key: &str, min: f64, max: f64) -> CacheResult<u64> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let removed: u64 = redis_cmd!(
            conn,
            redis::cmd("ZREMRANGEBYSCORE")
                .arg(&full_key)
                .arg(min)
                .arg(max)
        )?;

        Ok(removed)
    }
}

#[async_trait]
impl Cache for RedisCache {
    async fn get_bytes(&self, key: &str) -> CacheResult<Option<Vec<u8>>> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let data: Option<Vec<u8>> = redis_cmd!(conn, redis::cmd("GET").arg(&full_key))?;

        Ok(data)
    }

    async fn set_bytes(&self, key: &str, value: &[u8], ttl: Duration) -> CacheResult<()> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        if ttl.as_secs() > 0 {
            let _: () = redis_cmd!(
                conn,
                redis::cmd("SETEX")
                    .arg(&full_key)
                    .arg(ttl.as_secs())
                    .arg(value)
            )?;
        } else {
            let _: () = redis_cmd!(conn, redis::cmd("SET").arg(&full_key).arg(value))?;
        }

        Ok(())
    }

    async fn set_nx(&self, key: &str, value: &[u8], ttl: Duration) -> CacheResult<bool> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        // Use SET with NX (only set if not exists) and EX (expire)
        let result: Option<String> = if ttl.as_secs() > 0 {
            redis_cmd!(
                conn,
                redis::cmd("SET")
                    .arg(&full_key)
                    .arg(value)
                    .arg("NX")
                    .arg("EX")
                    .arg(ttl.as_secs())
            )?
        } else {
            redis_cmd!(conn, redis::cmd("SET").arg(&full_key).arg(value).arg("NX"))?
        };

        // SET ... NX returns "OK" if set, nil if key exists
        Ok(result.is_some())
    }

    async fn delete(&self, key: &str) -> CacheResult<()> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let _: () = redis_cmd!(conn, redis::cmd("DEL").arg(&full_key))?;
        Ok(())
    }

    async fn incr(&self, key: &str, ttl: Duration) -> CacheResult<i64> {
        self.incr_by(key, 1, ttl).await
    }

    async fn incr_by(&self, key: &str, delta: i64, ttl: Duration) -> CacheResult<i64> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        // Use Lua script to atomically increment and set TTL only if not already set.
        // This prevents extending rate limit windows on every increment.
        if ttl.as_secs() > 0 {
            let result: i64 = redis_script!(
                conn,
                redis::Script::new(INCR_PRESERVE_TTL_SCRIPT)
                    .key(&full_key)
                    .arg(delta)
                    .arg(ttl.as_secs() as i64)
            )?;
            Ok(result)
        } else {
            let result: i64 = redis_cmd!(conn, redis::cmd("INCRBY").arg(&full_key).arg(delta))?;
            Ok(result)
        }
    }

    async fn incr_by_float(&self, key: &str, delta: i64, ttl: Duration) -> CacheResult<i64> {
        // Use INCRBY for integer cents
        self.incr_by(key, delta, ttl).await
    }

    async fn check_and_reserve_budget(
        &self,
        key: &str,
        estimated_cost: i64,
        limit: i64,
        ttl: Duration,
    ) -> CacheResult<BudgetReservation> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let result: Vec<i64> = redis_script!(
            conn,
            redis::Script::new(BUDGET_CHECK_SCRIPT)
                .key(&full_key)
                .arg(estimated_cost)
                .arg(limit)
                .arg(ttl.as_secs() as i64)
        )?;

        Ok(BudgetReservation {
            allowed: result.first().copied().unwrap_or(0) == 1,
            current_spend: result.get(1).copied().unwrap_or(0),
            limit: result.get(2).copied().unwrap_or(limit),
        })
    }

    async fn check_and_incr_rate_limit(
        &self,
        key: &str,
        limit: u32,
        window_secs: u64,
    ) -> CacheResult<RateLimitResult> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let result: Vec<i64> = redis_script!(
            conn,
            redis::Script::new(RATE_LIMIT_SCRIPT)
                .key(&full_key)
                .arg(limit)
                .arg(window_secs as i64)
        )?;

        Ok(RateLimitResult {
            allowed: result.first().copied().unwrap_or(0) == 1,
            current: result.get(1).copied().unwrap_or(0),
            limit,
            reset_secs: result.get(2).copied().unwrap_or(window_secs as i64) as u64,
        })
    }

    async fn check_limits_batch(
        &self,
        budget_checks: &[BudgetCheckParams],
        rate_limit_checks: &[RateLimitCheckParams],
    ) -> CacheResult<BatchLimitResult> {
        // Empty checks = empty results
        if budget_checks.is_empty() && rate_limit_checks.is_empty() {
            return Ok(BatchLimitResult {
                budget_results: vec![],
                rate_limit_results: vec![],
            });
        }

        // All keys for the same API key use hash tags (e.g., gw:ratelimit:{api_key_id}:minute)
        // which ensures they hash to the same cluster slot, enabling pipelining in cluster mode.
        let mut conn = self.get_connection().await?;

        // Build a pipeline with all script invocations using EVAL
        // Note: redis-rs Script type doesn't support pipelining, so we use raw EVAL commands
        let mut pipe = redis::pipe();

        // Add budget check scripts to pipeline
        for check in budget_checks {
            let full_key = self.prefixed_key(&check.key);
            pipe.cmd("EVAL")
                .arg(BUDGET_CHECK_SCRIPT)
                .arg(1) // Number of keys
                .arg(&full_key)
                .arg(check.estimated_cost)
                .arg(check.limit)
                .arg(check.ttl.as_secs() as i64);
        }

        // Add rate limit check scripts to pipeline
        for check in rate_limit_checks {
            let full_key = self.prefixed_key(&check.key);
            pipe.cmd("EVAL")
                .arg(RATE_LIMIT_SCRIPT)
                .arg(1) // Number of keys
                .arg(&full_key)
                .arg(check.limit)
                .arg(check.window_secs as i64);
        }

        // Execute all scripts in a single round trip
        let results: Vec<Vec<i64>> = redis_pipe!(conn, pipe)?;

        // Parse budget results (first N results)
        let budget_results: Vec<BudgetReservation> = results
            .iter()
            .take(budget_checks.len())
            .zip(budget_checks.iter())
            .map(|(result, check)| BudgetReservation {
                allowed: result.first().copied().unwrap_or(0) == 1,
                current_spend: result.get(1).copied().unwrap_or(0),
                limit: result.get(2).copied().unwrap_or(check.limit),
            })
            .collect();

        // Parse rate limit results (remaining results)
        let rate_limit_results: Vec<RateLimitResult> = results
            .iter()
            .skip(budget_checks.len())
            .zip(rate_limit_checks.iter())
            .map(|(result, check)| RateLimitResult {
                allowed: result.first().copied().unwrap_or(0) == 1,
                current: result.get(1).copied().unwrap_or(0),
                limit: check.limit,
                reset_secs: result.get(2).copied().unwrap_or(check.window_secs as i64) as u64,
            })
            .collect();

        Ok(BatchLimitResult {
            budget_results,
            rate_limit_results,
        })
    }

    // ─────────────────────────────────────────────────────────────────────────────
    // SET Operations
    // ─────────────────────────────────────────────────────────────────────────────

    async fn set_add(&self, key: &str, member: &str, ttl: Option<Duration>) -> CacheResult<bool> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        // SADD returns the number of elements added (0 if already exists, 1 if new)
        let added: i64 = redis_cmd!(conn, redis::cmd("SADD").arg(&full_key).arg(member))?;

        // Only set TTL if provided and key has no existing expiry
        if let Some(ttl) = ttl
            && ttl.as_secs() > 0
        {
            // TTL returns -1 if key exists but has no expiry, -2 if key doesn't exist
            // After SADD, key always exists, so -1 means no expiry set
            let current_ttl: i64 = redis_cmd!(conn, redis::cmd("TTL").arg(&full_key))?;
            if current_ttl < 0 {
                let _: () =
                    redis_cmd!(conn, redis::cmd("EXPIRE").arg(&full_key).arg(ttl.as_secs()))?;
            }
        }

        Ok(added > 0)
    }

    async fn set_remove(&self, key: &str, member: &str) -> CacheResult<bool> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        // SREM returns the number of elements removed
        let removed: i64 = redis_cmd!(conn, redis::cmd("SREM").arg(&full_key).arg(member))?;

        Ok(removed > 0)
    }

    async fn set_members(&self, key: &str) -> CacheResult<Vec<String>> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let members: Vec<String> = redis_cmd!(conn, redis::cmd("SMEMBERS").arg(&full_key))?;

        Ok(members)
    }

    async fn set_cardinality(&self, key: &str) -> CacheResult<usize> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let count: i64 = redis_cmd!(conn, redis::cmd("SCARD").arg(&full_key))?;

        Ok(count as usize)
    }

    async fn set_is_member(&self, key: &str, member: &str) -> CacheResult<bool> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        let is_member: i64 = redis_cmd!(conn, redis::cmd("SISMEMBER").arg(&full_key).arg(member))?;

        Ok(is_member == 1)
    }

    async fn set_expire(&self, key: &str, ttl: Duration) -> CacheResult<bool> {
        let mut conn = self.get_connection().await?;
        let full_key = self.prefixed_key(key);

        // EXPIRE returns 1 if the timeout was set, 0 if key doesn't exist
        let result: i64 = redis_cmd!(conn, redis::cmd("EXPIRE").arg(&full_key).arg(ttl.as_secs()))?;

        Ok(result == 1)
    }

    fn as_redis(&self) -> Option<&RedisCache> {
        Some(self)
    }
}
